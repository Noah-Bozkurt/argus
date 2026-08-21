import { currentSessionToken, getWorkspaceUser } from '../../lib/auth'
import { getCommandHistory, getServer } from '../../lib/api'
import { startArgusUpdate } from './actions'
import UpdateSubmit from './update-submit'

function fmt(value: string | null) { return value ? new Date(value).toLocaleString() : '—' }

export default async function SystemPage() {
  const token = currentSessionToken()
  const user = token ? await getWorkspaceUser(token) : null
  const serverId = process.env.ARGUS_SERVER_ID
  const [server, commands] = serverId ? await Promise.all([getServer(serverId), getCommandHistory(serverId)]) : [null, []]
  const updates = commands.filter((item) => item.command.command_type.kind === 'argus.update')
  const updateLog = server?.snapshot?.diagnostics.journals.find((journal) => journal.service === 'argus-update')?.output
  const updateOutcome = !updateLog ? null : updateLog.includes('update succeeded:') ? 'Succeeded' : updateLog.includes('rollback completed successfully') ? 'Rolled back' : updateLog.includes('[argus-update] error:') ? 'Failed' : 'Running or awaiting status'
  const installed = process.env.ARGUS_VERSION ?? server?.snapshot?.agent_version ?? 'unknown'
  const owner = user?.role === 'owner'
  const updateCapable = server?.capabilities?.some((capability) => capability.name === 'argus.update' && capability.version === 'v1') ?? false

  return <main>
    <div className="page-header"><div><span className="eyebrow">Administration</span><h1>Argus system</h1><p>Control-plane health, versions, recovery information and transactional updates.</p></div></div>
    <div className="stats-grid">
      <div className="stat-card"><div className="stat-label"><span>Revision</span><span className="badge info">Pinned</span></div><div className="stat-value system-revision">{installed.slice(0, 12)}</div><div className="stat-meta">Immutable installed revision</div></div>
      <div className="stat-card"><div className="stat-label"><span>Local agent</span><span className={`status-dot ${server?.online ? 'online' : 'danger'}`} /></div><div className="stat-value">{server?.online ? 'Online' : 'Offline'}</div><div className="stat-meta">Last heartbeat {fmt(server?.last_heartbeat ?? null)}</div></div>
      <div className="stat-card"><div className="stat-label"><span>Pending packages</span><span className="badge">APT</span></div><div className="stat-value">{server?.snapshot?.updates.pending_updates ?? '—'}</div><div className="stat-meta">On the control-plane host</div></div>
      <div className="stat-card"><div className="stat-label"><span>Update history</span><span className="badge">Recent</span></div><div className="stat-value">{updates.length}</div><div className="stat-meta">Browser-requested updates</div></div>
    </div>
    <div className="detail-split">
      <section className="detail-card"><div className="detail-card-header"><div><h2>Control-plane update</h2><p>Uses the host updater, verified snapshots, health checks and automatic rollback.</p></div><span className={`badge ${owner ? 'success' : 'warning'}`}>{owner ? 'Owner access' : 'Owner only'}</span></div><div className="detail-card-body">
        {!server ? <div className="callout warning">The local control-plane server is not configured. Keep using <code>sudo argusctl update</code>.</div> : <form action={startArgusUpdate}>
          <label>Release channel or immutable revision<input name="version" defaultValue="main" pattern="[A-Za-z0-9_][A-Za-z0-9_.-]{0,127}" required disabled={!owner} /></label>
          <label>Confirm your password<input name="password" type="password" autoComplete="current-password" required disabled={!owner} /></label>
          <div className="callout warning">Argus will become temporarily unavailable. A host-side unit continues the transaction and rolls back if health verification fails.</div>
          {!updateCapable ? <div className="callout warning">The local Agent/Helper must be upgraded before browser-managed updates can be scheduled.</div> : null}
          <UpdateSubmit disabled={!owner || !server.online || !updateCapable} />
        </form>}
      </div></section>
      <section className="detail-card"><div className="detail-card-header"><div><h2>Recovery</h2><p>Local commands remain available if the web interface cannot return.</p></div></div><div className="detail-card-body"><div className="callout"><strong>Check status</strong><pre>sudo argusctl status</pre></div><div className="callout"><strong>Recover an interrupted update</strong><pre>sudo argusctl recover-update</pre></div><p className="muted">Update snapshots are retained on the host and validated before the target revision starts.</p></div></section>
    </div>
    <section className="detail-card"><div className="detail-card-header"><div><h2>Update activity</h2><p>Scheduling acknowledgements and latest host-side updater output.</p></div>{updateOutcome ? <span className={`badge ${updateOutcome === 'Succeeded' ? 'success' : updateOutcome === 'Failed' ? 'danger' : updateOutcome === 'Rolled back' ? 'warning' : 'info'}`}>{updateOutcome}</span> : null}</div><div className="detail-card-body">{updates.length ? <ol className="timeline">{updates.map((item) => <li className="timeline-item" key={item.command.id}><div className="timeline-title">Argus update to {item.command.command_type.version ?? 'main'}</div><div className="timeline-meta"><span className={`badge ${item.command.status === 'SUCCEEDED' ? 'success' : item.command.status === 'FAILED' ? 'danger' : 'info'}`}>{item.command.status === 'SUCCEEDED' ? 'Scheduled on host' : item.command.status}</span> {fmt(item.command.created_at)}</div>{item.output ? <pre>{item.output}</pre> : null}</li>)}</ol> : <div className="empty-state"><strong>No browser updates yet</strong>Update operations appear here after they are requested.</div>}{updateLog ? <details className="operation-log"><summary className="button">Full update log</summary><div className="operation-log-toolbar"><span>Latest host updater output; this is authoritative for success or rollback.</span><span className="badge info">Host log</span></div><pre>{updateLog}</pre></details> : null}</div></section>
  </main>
}
