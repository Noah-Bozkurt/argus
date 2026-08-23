'use client'

import Link from 'next/link'
import { useEffect, useMemo, useState } from 'react'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import { flexRender, getCoreRowModel, useReactTable, type ColumnDef } from '@tanstack/react-table'
import type { ControlApiServerView as ServerView } from '../../../lib/control-api-contract'
import LucideIcon from '../../lucide-icons'
import Tooltip from '../../ui/tooltip'
import usePersistentChoice from '../../use-persistent-choice'

const FAVORITES_KEY = 'argus:favorites:v1'
const SERVER_QUERY_KEY = ['servers'] as const
const STATUS_FILTERS = ['all', 'online', 'offline', 'attention'] as const
const SORTS = ['name', 'heartbeat', 'cpu', 'disk'] as const

type StatusFilter = typeof STATUS_FILTERS[number]
type SortChoice = typeof SORTS[number]

function Utilization({ value }: { value: number | undefined }) {
  const safe = typeof value === 'number' ? Math.max(0, Math.min(100, value)) : 0
  return <div className="utilization-cell"><div className="utilization-value"><span>{typeof value === 'number' ? `${Math.round(value)}%` : '—'}</span></div><div className="utilization-track"><div className="utilization-fill" style={{ width: `${safe}%` }} /></div></div>
}

function relativeTime(value: string | null | undefined): string {
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

async function fetchServers(): Promise<ServerView[]> {
  const response = await fetch('/api/servers', { cache: 'no-store' })
  if (!response.ok) throw new Error(`Unable to refresh servers (${response.status})`)
  return response.json() as Promise<ServerView[]>
}

export default function ServerFleet({ initialServers }: { initialServers: ServerView[] }) {
  const [live, setLive] = useState(false)
  const [query, setQuery] = useState('')
  const [status, setStatus] = usePersistentChoice<StatusFilter>('argus:servers:status', 'all', STATUS_FILTERS)
  const [sort, setSort] = usePersistentChoice<SortChoice>('argus:servers:sort', 'name', SORTS)
  const [favorites, setFavorites] = useState<string[]>(initialFavorites)
  const [copied, setCopied] = useState<string | null>(null)
  const queryClient = useQueryClient()
  const serverQuery = useQuery({
    queryKey: SERVER_QUERY_KEY,
    queryFn: fetchServers,
    initialData: initialServers,
    staleTime: 5_000,
    refetchInterval: live ? false : 10_000,
  })
  const servers = serverQuery.data

  useEffect(() => {
    let lastMessage = 0
    const source = new EventSource('/api/servers/events')
    source.addEventListener('snapshot', (event) => {
      const next = JSON.parse((event as MessageEvent).data) as ServerView[]
      queryClient.setQueryData(SERVER_QUERY_KEY, next)
      lastMessage = Date.now()
      setLive(true)
    })
    source.onerror = () => { if (Date.now() - lastMessage > 20_000) setLive(false) }
    const staleTimer = window.setInterval(() => { if (Date.now() - lastMessage > 20_000) setLive(false) }, 5_000)
    return () => { source.close(); window.clearInterval(staleTimer) }
  }, [queryClient])

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

  const columns = useMemo<ColumnDef<ServerView>[]>(() => [
    {
      id: 'server',
      header: 'Server',
      cell: ({ row }) => {
        const server = row.original
        const pinned = favorites.includes(`server:${server.server_id}`)
        return <div className="resource-inline-actions"><Tooltip content={pinned ? `Unpin ${server.hostname}` : `Pin ${server.hostname}`}><button className={`pin-button${pinned ? ' active' : ''}`} type="button" onClick={() => toggleFavorite(server.server_id)} aria-label={pinned ? `Unpin ${server.hostname}` : `Pin ${server.hostname}`}><LucideIcon name="star" /></button></Tooltip><div><div className="row-title"><span className={`status-dot ${server.online ? 'online' : 'danger'}`} /><Link href={`/infrastructure/servers/${server.server_id}`}>{server.hostname}</Link>{attention(server) ? <span className="badge warning">Attention</span> : null}</div><div className="row-subtitle">{server.snapshot?.os ?? 'Unknown OS'} · <code>{server.server_id.slice(0, 12)}</code> <button className="copy-button" type="button" onClick={() => void copy(server.server_id, server.server_id)}>{copied === server.server_id ? 'Copied' : 'Copy ID'}</button></div></div></div>
      },
    },
    { id: 'status', header: 'Status', cell: ({ row }) => <span className={`state-label ${row.original.online ? 'success' : 'danger'}`}>{row.original.online ? 'Online' : 'Offline'}</span> },
    { id: 'cpu', header: 'CPU', cell: ({ row }) => <Utilization value={row.original.snapshot?.cpu_percent} /> },
    { id: 'memory', header: 'Memory', cell: ({ row }) => <Utilization value={row.original.snapshot?.ram_percent} /> },
    { id: 'disk', header: 'Disk', cell: ({ row }) => <Utilization value={row.original.snapshot?.disk_percent} /> },
    { id: 'services', header: 'Services', cell: ({ row }) => row.original.services.length },
    { id: 'heartbeat', header: 'Heartbeat', cell: ({ row }) => <span title={row.original.last_heartbeat ? new Date(row.original.last_heartbeat).toLocaleString() : undefined}>{relativeTime(row.original.last_heartbeat)}</span> },
  ], [copied, favorites])

  const table = useReactTable({
    data: visible,
    columns,
    getCoreRowModel: getCoreRowModel(),
  })

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
          <span className={`live-state ${live ? 'online' : 'connecting'}`} title={serverQuery.isError ? 'Live connection unavailable; polling fallback active' : undefined}><span className="status-dot" />{live ? 'Live' : serverQuery.isFetching ? 'Syncing' : 'Polling'}</span>
        </div>
      </div>

      {visible.length === 0 ? <div className="empty-state"><strong>No matching servers</strong>Change the search or status filter to show other nodes.</div> : <>
        <div className="desktop-resource-table table-wrap server-table-wrap">
          <table>
            <thead>{table.getHeaderGroups().map((headerGroup) => <tr key={headerGroup.id}>{headerGroup.headers.map((header) => <th key={header.id}>{header.isPlaceholder ? null : flexRender(header.column.columnDef.header, header.getContext())}</th>)}</tr>)}</thead>
            <tbody>{table.getRowModel().rows.map((row) => <tr key={row.id}>{row.getVisibleCells().map((cell) => <td key={cell.id}>{flexRender(cell.column.columnDef.cell, cell.getContext())}</td>)}</tr>)}</tbody>
          </table>
        </div>

        <ul className="mobile-server-list">
          {visible.map((server) => {
            const pinned = favorites.includes(`server:${server.server_id}`)
            return <li key={server.server_id}>
              <div className="mobile-server-head">
                <Link href={`/infrastructure/servers/${server.server_id}`}><span className={`status-dot ${server.online ? 'online' : 'danger'}`} /><strong>{server.hostname}</strong></Link>
                <Tooltip content={pinned ? `Unpin ${server.hostname}` : `Pin ${server.hostname}`}><button className={`pin-button${pinned ? ' active' : ''}`} type="button" onClick={() => toggleFavorite(server.server_id)} aria-label={pinned ? `Unpin ${server.hostname}` : `Pin ${server.hostname}`}><LucideIcon name="star" /></button></Tooltip>
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
