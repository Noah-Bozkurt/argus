'use client'

import Link from 'next/link'
import { useEffect, useMemo, useState } from 'react'
import type { ServerView } from '../../../lib/api'
import LucideIcon from '../../lucide-icons'
import usePersistentChoice from '../../use-persistent-choice'

const FAVORITES_KEY = 'argus:favorites:v1'
const STATUS_FILTERS = ['all', 'online', 'offline', 'attention'] as const
const SORTS = ['name', 'heartbeat', 'cpu', 'disk'] as const

type StatusFilter = typeof STATUS_FILTERS[number]
type SortChoice = typeof SORTS[number]

function Utilization({ value }: { value: number | undefined }) {
  const safe = typeof value === 'number' ? Math.max(0, Math.min(100, value)) : 0
  return <div className="utilization-cell"><div className="utilization-value"><span>{typeof value === 'number' ? `${Math.round(value)}%` : '—'}</span></div><div className="utilization-track"><div className="utilization-fill" style={{ width: `${safe}%` }} /></div></div>
}

function relativeTime(value: string | null): string {
  if (!value) return 'Never'
  const delta = Date.now() - new Date(value).getTime()
  const seconds = Math.max(0, Math.floor(delta / 1000))
  if (seconds < 60) return `${seconds}s ago`
  const minutes = Math.floor(seconds / 60)
  if (minutes < 60) return `${minutes}m ago`
  const hours = Math.floor(minutes / 60)
  if (hours < 24) return `${hours}h ago`
  return `${Math.floor(hours / 24)}d ago`
}

function attention(server: ServerView): boolean {
  const snapshot = server.snapshot
  return !server.online || Boolean(snapshot && (
    snapshot.disk_percent >= 85 ||
    snapshot.updates.reboot_required ||
    snapshot.diagnostics.failed_units.length > 0 ||
    snapshot.security.findings.some((finding) => ['HIGH', 'CRITICAL'].includes(finding.severity.toUpperCase()))
  ))
}

function initialFavorites(): string[] {
  if (typeof window === 'undefined') return []
  try { return JSON.parse(window.localStorage.getItem(FAVORITES_KEY) ?? '[]') as string[] } catch { return [] }
}

export default function ServerFleet({ initialServers }: { initialServers: ServerView[] }) {
  const [servers, setServers] = useState(initialServers)
  const [live, setLive] = useState(false)
  const [query, setQuery] = useState('')
  const [status, setStatus] = usePersistentChoice<StatusFilter>('argus:servers:status', 'all', STATUS_FILTERS)
  const [sort, setSort] = usePersistentChoice<SortChoice>('argus:servers:sort', 'name', SORTS)
  const [favorites, setFavorites] = useState<string[]>(initialFavorites)
  const [copied, setCopied] = useState<string | null>(null)

  useEffect(() => {
    let lastMessage = 0
    const source = new EventSource('/api/servers/events')
    source.addEventListener('snapshot', (event) => { setServers(JSON.parse((event as MessageEvent).data)); lastMessage = Date.now(); setLive(true) })
    source.onerror = () => { if (Date.now() - lastMessage > 20_000) setLive(false) }
    const staleTimer = window.setInterval(() => { if (Date.now() - lastMessage > 20_000) setLive(false) }, 5_000)
    return () => { source.close(); window.clearInterval(staleTimer) }
  }, [])

  const online = servers.filter((server) => server.online).length
  const needsAttention = servers.filter(attention).length
  const containers = servers.reduce((sum, server) => sum + (server.snapshot?.docker.containers.length ?? 0), 0)

  const visible = useMemo(() => {
    const normalized = query.trim().toLowerCase()
    return servers
      .filter((server) => {
        if (status === 'online' && !server.online) return false
        if (status === 'offline' && server.online) return false
        if (status === 'attention' && !attention(server)) return false
        return true
      })
      .filter((server) => !normalized || [server.hostname, server.server_id, server.snapshot?.os ?? '', server.snapshot?.architecture ?? '', ...server.services.map((service) => service.name)].join(' ').toLowerCase().includes(normalized))
      .sort((left, right) => {
        const leftPinned = favorites.includes(`server:${left.server_id}`) ? 1 : 0
        const rightPinned = favorites.includes(`server:${right.server_id}`) ? 1 : 0
        if (leftPinned !== rightPinned) return rightPinned - leftPinned
        if (sort === 'heartbeat') return Date.parse(right.last_heartbeat ?? '1970-01-01') - Date.parse(left.last_heartbeat ?? '1970-01-01')
        if (sort === 'cpu') return (right.snapshot?.cpu_percent ?? -1) - (left.snapshot?.cpu_percent ?? -1)
        if (sort === 'disk') return (right.snapshot?.disk_percent ?? -1) - (left.snapshot?.disk_percent ?? -1)
        return left.hostname.localeCompare(right.hostname)
      })
  }, [favorites, query, servers, sort, status])

  function toggleFavorite(serverId: string) {
    const key = `server:${serverId}`
    setFavorites((current) => {
      const next = current.includes(key) ? current.filter((item) => item !== key) : [key, ...current]
      window.localStorage.setItem(FAVORITES_KEY, JSON.stringify(next))
      return next
    })
  }

  async function copy(value: string, key: string) {
    await navigator.clipboard.writeText(value)
    setCopied(key)
    window.setTimeout(() => setCopied((current) => current === key ? null : current), 1200)
  }

  return <>
    <div className="stats-grid fleet-summary">
      <div className="stat-card"><div className="stat-label"><span>Servers</span></div><div className="stat-value">{servers.length}</div><div className="stat-meta">registered nodes</div></div>
      <div className="stat-card"><div className="stat-label"><span>Online</span><span className="status-dot online" /></div><div className="stat-value">{online}</div><div className="stat-meta">healthy heartbeats</div></div>
      <div className="stat-card"><div className="stat-label"><span>Attention</span><span className={`status-dot ${needsAttention ? 'warning' : 'online'}`} /></div><div className="stat-value">{needsAttention}</div><div className="stat-meta">actionable nodes</div></div>
      <div className="stat-card"><div className="stat-label"><span>Containers</span></div><div className="stat-value">{containers}</div><div className="stat-meta">visible in snapshots</div></div>
    </div>

    <section className="resource-section server-fleet-section">
      <div className="section-bar resource-toolbar-header">
        <div><h2>Fleet</h2><p>{visible.length} shown · live host telemetry</p></div>
        <div className="resource-toolbar">
          <input type="search" value={query} onChange={(event) => setQuery(event.target.value)} placeholder="Search servers…" aria-label="Search servers" />
          <select value={status} onChange={(event) => setStatus(event.target.value as StatusFilter)} aria-label="Filter server status"><option value="all">All statuses</option><option value="online">Online</option><option value="offline">Offline</option><option value="attention">Needs attention</option></select>
          <select value={sort} onChange={(event) => setSort(event.target.value as SortChoice)} aria-label="Sort servers"><option value="name">Name</option><option value="heartbeat">Latest heartbeat</option><option value="cpu">CPU usage</option><option value="disk">Disk usage</option></select>
          <span className={`live-state ${live ? 'online' : 'connecting'}`}><span className="status-dot" />{live ? 'Live' : 'Connecting'}</span>
        </div>
      </div>

      {visible.length === 0 ? <div className="empty-state"><strong>No matching servers</strong>Change the search or status filter to show other nodes.</div> : <>
        <div className="desktop-resource-table table-wrap server-table-wrap">
          <table><thead><tr><th>Server</th><th>Status</th><th>CPU</th><th>Memory</th><th>Disk</th><th>Services</th><th>Heartbeat</th></tr></thead><tbody>{visible.map((server) => {
            const pinned = favorites.includes(`server:${server.server_id}`)
            return <tr key={server.server_id}><td><div className="resource-inline-actions"><button className={`pin-button${pinned ? ' active' : ''}`} type="button" onClick={() => toggleFavorite(server.server_id)} aria-label={pinned ? `Unpin ${server.hostname}` : `Pin ${server.hostname}`}><LucideIcon name="star" /></button><div><div className="row-title"><span className={`status-dot ${server.online ? 'online' : 'danger'}`} /><Link href={`/infrastructure/servers/${server.server_id}`}>{server.hostname}</Link>{attention(server) ? <span className="badge warning">Attention</span> : null}</div><div className="row-subtitle">{server.snapshot?.os ?? 'Unknown OS'} · <code>{server.server_id.slice(0, 12)}</code> <button className="copy-button" type="button" onClick={() => void copy(server.server_id, server.server_id)}>{copied === server.server_id ? 'Copied' : 'Copy ID'}</button></div></div></div></td><td><span className={`state-label ${server.online ? 'success' : 'danger'}`}>{server.online ? 'Online' : 'Offline'}</span></td><td><Utilization value={server.snapshot?.cpu_percent} /></td><td><Utilization value={server.snapshot?.ram_percent} /></td><td><Utilization value={server.snapshot?.disk_percent} /></td><td>{server.services.length}</td><td title={server.last_heartbeat ? new Date(server.last_heartbeat).toLocaleString() : undefined}>{relativeTime(server.last_heartbeat)}</td></tr>
          })}</tbody></table>
        </div>

        <ul className="mobile-server-list">
          {visible.map((server) => {
            const pinned = favorites.includes(`server:${server.server_id}`)
            return <li key={server.server_id}>
              <div className="mobile-server-head">
                <Link href={`/infrastructure/servers/${server.server_id}`}><span className={`status-dot ${server.online ? 'online' : 'danger'}`} /><strong>{server.hostname}</strong></Link>
                <button className={`pin-button${pinned ? ' active' : ''}`} type="button" onClick={() => toggleFavorite(server.server_id)} aria-label={pinned ? `Unpin ${server.hostname}` : `Pin ${server.hostname}`}><LucideIcon name="star" /></button>
              </div>
              <div className="mobile-server-meta"><span>{server.snapshot?.os ?? 'Unknown OS'}</span><span>{relativeTime(server.last_heartbeat)}</span>{attention(server) ? <span className="state-label warning">Attention</span> : null}</div>
              <div className="mobile-server-metrics"><div><span>CPU</span><strong>{server.snapshot ? `${Math.round(server.snapshot.cpu_percent)}%` : '—'}</strong></div><div><span>Memory</span><strong>{server.snapshot ? `${Math.round(server.snapshot.ram_percent)}%` : '—'}</strong></div><div><span>Disk</span><strong>{server.snapshot ? `${Math.round(server.snapshot.disk_percent)}%` : '—'}</strong></div></div>
            </li>
          })}
        </ul>
      </>}
    </section>
  </>
}
