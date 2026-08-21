'use client'

import Link from 'next/link'
import { useEffect, useState } from 'react'
import type { ServerView } from '../../../lib/api'

function Utilization({ value }: { value: number | undefined }) {
  const safe = typeof value === 'number' ? Math.max(0, Math.min(100, value)) : 0
  return <div className="utilization-cell"><div className="utilization-value"><span>{typeof value === 'number' ? `${Math.round(value)}%` : '—'}</span></div><div className="utilization-track"><div className="utilization-fill" style={{ width: `${safe}%` }} /></div></div>
}

export default function ServerFleet({ initialServers }: { initialServers: ServerView[] }) {
  const [servers, setServers] = useState(initialServers)
  const [live, setLive] = useState(false)
  useEffect(() => {
    let lastMessage = 0
    const source = new EventSource('/api/servers/events')
    source.addEventListener('snapshot', (event) => { setServers(JSON.parse((event as MessageEvent).data)); lastMessage = Date.now(); setLive(true) })
    source.onerror = () => { if (Date.now() - lastMessage > 20_000) setLive(false) }
    const staleTimer = window.setInterval(() => { if (Date.now() - lastMessage > 20_000) setLive(false) }, 5_000)
    return () => { source.close(); window.clearInterval(staleTimer) }
  }, [])
  const online = servers.filter((server) => server.online).length
  const unhealthy = servers.length - online
  return <>
    <div className="stats-grid">
      <div className="stat-card"><div className="stat-label"><span>Total servers</span><span className="badge">Fleet</span></div><div className="stat-value">{servers.length}</div><div className="stat-meta">Registered with this workspace</div></div>
      <div className="stat-card"><div className="stat-label"><span>Online</span><span className="status-dot online" /></div><div className="stat-value">{online}</div><div className="stat-meta">Healthy agent heartbeat</div></div>
      <div className="stat-card"><div className="stat-label"><span>Offline</span><span className={`status-dot ${unhealthy ? 'danger' : 'online'}`} /></div><div className="stat-value">{unhealthy}</div><div className="stat-meta">Needs attention</div></div>
      <div className="stat-card"><div className="stat-label"><span>Containers</span><span className="badge info">Docker</span></div><div className="stat-value">{servers.reduce((sum, server) => sum + (server.snapshot?.docker.containers.length ?? 0), 0)}</div><div className="stat-meta">Visible across snapshots</div></div>
    </div>
    <section className="panel"><div className="panel-header"><div><h2>Infrastructure fleet</h2><p>{online}/{servers.length} servers reporting online</p></div><span className={`badge ${live ? 'success' : 'warning'}`}><span className={`status-dot ${live ? 'online' : 'warning'}`} />{live ? 'Live' : 'Connecting'}</span></div>
      {servers.length === 0 ? <div className="empty-state"><strong>No servers registered</strong>Add a server from a project environment to start monitoring it.</div> : <div className="table-wrap" style={{ border: 0, borderRadius: 0 }}><table className="responsive-table"><thead><tr><th>Server</th><th>Status</th><th>CPU</th><th>RAM</th><th>Disk</th><th>Services</th><th>Last heartbeat</th></tr></thead><tbody>{servers.map((server) => <tr key={server.server_id}><td><div className="row-title"><span className={`status-dot ${server.online ? 'online' : 'danger'}`} /><Link href={`/infrastructure/servers/${server.server_id}`}>{server.hostname}</Link></div><div className="row-subtitle">{server.snapshot?.os ?? 'Unknown OS'} · <code>{server.server_id.slice(0, 12)}</code></div></td><td data-label="Status"><span className={`badge ${server.online ? 'success' : 'danger'}`}>{server.online ? 'Online' : 'Offline'}</span></td><td data-label="CPU"><Utilization value={server.snapshot?.cpu_percent} /></td><td data-label="RAM"><Utilization value={server.snapshot?.ram_percent} /></td><td data-label="Disk"><Utilization value={server.snapshot?.disk_percent} /></td><td data-label="Services">{server.services.length}</td><td data-label="Heartbeat">{server.last_heartbeat ? new Date(server.last_heartbeat).toLocaleString() : 'Never'}</td></tr>)}</tbody></table></div>}
    </section>
  </>
}
