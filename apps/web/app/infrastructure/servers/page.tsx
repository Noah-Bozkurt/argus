import Link from 'next/link'
import { getServers } from '../../../lib/api'

function formatPercent(value: number | undefined): string {
  return typeof value === 'number' ? `${Math.round(value)}%` : '—'
}

function formatHeartbeat(value: string | null): string {
  return value ? new Date(value).toLocaleString() : 'Never'
}

function Utilization({ value }: { value: number | undefined }) {
  const safe = typeof value === 'number' ? Math.max(0, Math.min(100, value)) : 0
  return (
    <div className="utilization-cell">
      <div className="utilization-value"><span>{formatPercent(value)}</span></div>
      <div className="utilization-track"><div className="utilization-fill" style={{ width: `${safe}%` }} /></div>
    </div>
  )
}

export default async function ServersPage() {
  const servers = await getServers()
  const online = servers.filter((server) => server.online).length
  const unhealthy = servers.length - online

  return (
    <main>
      <div className="page-header">
        <div>
          <span className="eyebrow">Infrastructure</span>
          <h1>Servers</h1>
          <p>Host health, utilization and connected workloads reported by Argus agents.</p>
        </div>
      </div>

      <div className="stats-grid">
        <div className="stat-card"><div className="stat-label"><span>Total servers</span><span className="badge">Fleet</span></div><div className="stat-value">{servers.length}</div><div className="stat-meta">Registered with this workspace</div></div>
        <div className="stat-card"><div className="stat-label"><span>Online</span><span className="status-dot online" /></div><div className="stat-value">{online}</div><div className="stat-meta">Healthy agent heartbeat</div></div>
        <div className="stat-card"><div className="stat-label"><span>Offline</span><span className={`status-dot ${unhealthy ? 'danger' : 'online'}`} /></div><div className="stat-value">{unhealthy}</div><div className="stat-meta">Needs attention</div></div>
        <div className="stat-card"><div className="stat-label"><span>Containers</span><span className="badge info">Docker</span></div><div className="stat-value">{servers.reduce((sum, server) => sum + (server.snapshot?.docker.containers.length ?? 0), 0)}</div><div className="stat-meta">Visible across online snapshots</div></div>
      </div>

      <section className="panel">
        <div className="panel-header"><div><h2>Infrastructure fleet</h2><p>{online}/{servers.length} servers reporting online</p></div></div>
        {servers.length === 0 ? (
          <div className="empty-state"><strong>No servers registered</strong>Add a server from a project environment to start monitoring it.</div>
        ) : (
          <div className="table-wrap" style={{ border: 0, borderRadius: 0 }}>
            <table className="responsive-table">
              <thead><tr><th>Server</th><th>Status</th><th>CPU</th><th>RAM</th><th>Disk</th><th>Services</th><th>Last heartbeat</th></tr></thead>
              <tbody>
                {servers.map((server) => (
                  <tr key={server.server_id}>
                    <td>
                      <div className="row-title"><span className={`status-dot ${server.online ? 'online' : 'danger'}`} /><Link href={`/infrastructure/servers/${server.server_id}`}>{server.hostname}</Link></div>
                      <div className="row-subtitle">{server.snapshot?.os ?? 'Unknown OS'} · <code>{server.server_id.slice(0, 12)}</code></div>
                    </td>
                    <td data-label="Status"><span className={`badge ${server.online ? 'success' : 'danger'}`}>{server.online ? 'Online' : 'Offline'}</span></td>
                    <td data-label="CPU"><Utilization value={server.snapshot?.cpu_percent} /></td>
                    <td data-label="RAM"><Utilization value={server.snapshot?.ram_percent} /></td>
                    <td data-label="Disk"><Utilization value={server.snapshot?.disk_percent} /></td>
                    <td data-label="Services">{server.services.length}</td>
                    <td data-label="Heartbeat">{formatHeartbeat(server.last_heartbeat)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </section>
    </main>
  )
}
