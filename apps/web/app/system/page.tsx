import { currentSessionToken, getWorkspaceUser } from '../../lib/auth'
import { getCommandHistory, getServer } from '../../lib/api'
import LucideIcon from '../lucide-icons'
import { startArgusUpdate } from './actions'
import UpdateSubmit from './update-submit'
import WhatsNew from './whats-new'

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
  const latestUpdate = updates[0]

  return <main className="system-page">
    <div className="page-header">
      <div>
        <span className="eyebrow">Administration</span>
        <h1>System</h1>
        <p>Manage this Argus installation, run safe updates and find recovery tools when the web interface cannot.</p>
      </div>
    </div>

    <section className="system-summary" aria-label="System summary">
      <div className="system-summary-item">
        <div className="system-summary-label"><LucideIcon name="package" className="system-icon" />Installed revision</div>
        <div className="system-summary-value">{installed.slice(0, 12)}</div>
        <div className="system-summary-meta">Pinned installation revision</div>
      </div>
      <div className="system-summary-item">
        <div className="system-summary-label"><LucideIcon name="servers" className="system-icon" />Local agent</div>
        <div className="system-summary-value">{server?.online ? 'Online' : 'Offline'}</div>
        <div className="system-summary-meta">Last heartbeat {fmt(server?.last_heartbeat ?? null)}</div>
      </div>
      <div className="system-summary-item">
        <div className="system-summary-label"><LucideIcon name="shield-check" className="system-icon" />Package updates</div>
        <div className="system-summary-value">{server?.snapshot?.updates.pending_updates ?? '—'} pending</div>
        <div className="system-summary-meta">APT packages on this host</div>
      </div>
      <div className="system-summary-item">
        <div className="system-summary-label"><LucideIcon name="history" className="system-icon" />Latest Argus update</div>
        <div className="system-summary-value">{updateOutcome ?? (latestUpdate ? latestUpdate.command.status : 'No history')}</div>
        <div className="system-summary-meta">{latestUpdate ? fmt(latestUpdate.command.created_at) : 'No browser-managed update yet'}</div>
      </div>
    </section>

    <div className="system-layout">
      <section className="system-card">
        <div className="system-card-header">
          <div className="system-heading">
            <span className="system-heading-icon"><LucideIcon name="refresh" className="system-icon" /></span>
            <div>
              <h2>Update Argus</h2>
              <p>Runs the host updater with a rollback snapshot and health verification before the new revision is accepted.</p>
            </div>
          </div>
          <span className={`system-access${owner ? ' allowed' : ''}`}>{owner ? 'Owner access' : 'Owner only'}</span>
        </div>
        <div className="system-card-body">
          {!server ? (
            <div className="system-notice"><LucideIcon name="alert-triangle" className="system-icon" /><span>The local control-plane server is not configured in the web container. Use <code>sudo argusctl update</code> on the host.</span></div>
          ) : (
            <form action={startArgusUpdate}>
              <label>
                Target revision
                <input name="version" defaultValue="main" pattern="[A-Za-z0-9_][A-Za-z0-9_.-]{0,127}" required disabled={!owner} />
              </label>
              <p className="system-form-help">Use <code>main</code> for the current channel or enter an immutable revision.</p>
              <label>
                Confirm your password
                <input name="password" type="password" autoComplete="current-password" required disabled={!owner} />
              </label>
              <div className="system-notice"><LucideIcon name="alert-triangle" className="system-icon" /><span>The web UI will be briefly unavailable while the host-side update continues independently. Failed health checks automatically trigger rollback.</span></div>
              {!updateCapable ? <div className="system-notice"><LucideIcon name="alert-triangle" className="system-icon" /><span>The local Agent/Helper does not expose <code>argus.update/v1</code> yet. Upgrade the host tools before scheduling updates here.</span></div> : null}
              <UpdateSubmit disabled={!owner || !server.online || !updateCapable} />
            </form>
          )}
        </div>
      </section>

      <div className="system-side-stack">
        <section className="system-card">
          <div className="system-card-header">
            <div className="system-heading">
              <span className="system-heading-icon"><LucideIcon name="shield-check" className="system-icon" /></span>
              <div><h2>Update readiness</h2><p>What must be available before Argus can update itself from the browser.</p></div>
            </div>
          </div>
          <div className="system-card-body">
            <div className="system-status-list">
              <div className="system-status-row"><span>Local agent</span><span>{server?.online ? 'Online' : 'Offline'}</span></div>
              <div className="system-status-row"><span>Update capability</span><span>{updateCapable ? 'Available' : 'Unavailable'}</span></div>
              <div className="system-status-row"><span>Current account</span><span>{owner ? 'Owner' : user?.role ?? 'Unknown'}</span></div>
              <div className="system-status-row"><span>Rollback safety</span><span>Host managed</span></div>
            </div>
          </div>
        </section>

        <section className="system-card">
          <div className="system-card-header">
            <div className="system-heading">
              <span className="system-heading-icon"><LucideIcon name="terminal" className="system-icon" /></span>
              <div><h2>Recovery</h2><p>Keep these host commands available if the control plane cannot return.</p></div>
            </div>
          </div>
          <div className="system-card-body">
            <div className="system-code-list">
              <div className="system-code-row"><strong>Check installation status</strong><code>sudo argusctl status</code></div>
              <div className="system-code-row"><strong>Recover an interrupted update</strong><code>sudo argusctl recover-update</code></div>
              <div className="system-code-row"><strong>Inspect update logs</strong><code>sudo argusctl logs update</code></div>
            </div>
          </div>
        </section>
      </div>
    </div>

    <WhatsNew revision={installed} />

    <section className="system-card system-activity">
      <div className="system-card-header">
        <div className="system-heading">
          <span className="system-heading-icon"><LucideIcon name="history" className="system-icon" /></span>
          <div><h2>Update activity</h2><p>Recent browser requests and the latest authoritative host updater output.</p></div>
        </div>
        {updateOutcome ? <span className={`badge ${updateOutcome === 'Succeeded' ? 'success' : updateOutcome === 'Failed' ? 'danger' : updateOutcome === 'Rolled back' ? 'warning' : 'info'}`}>{updateOutcome}</span> : null}
      </div>
      <div className="system-card-body">
        {updates.length ? (
          <ol className="timeline">
            {updates.map((item) => <li className="timeline-item" key={item.command.id}>
              <div className="timeline-title">Update to {item.command.command_type.version ?? 'main'}</div>
              <div className="timeline-meta"><span className={`badge ${item.command.status === 'SUCCEEDED' ? 'success' : item.command.status === 'FAILED' ? 'danger' : 'info'}`}>{item.command.status === 'SUCCEEDED' ? 'Scheduled on host' : item.command.status}</span> {fmt(item.command.created_at)}</div>
              {item.output ? <pre>{item.output}</pre> : null}
            </li>)}
          </ol>
        ) : <div className="empty-state"><strong>No browser updates yet</strong>Update requests will appear here after they are scheduled.</div>}
        {updateLog ? <details className="operation-log"><summary className="button">View full update log</summary><div className="operation-log-toolbar"><span>Latest host updater output. This determines whether the update succeeded or rolled back.</span><span className="badge info">Host log</span></div><pre>{updateLog}</pre></details> : null}
      </div>
    </section>
  </main>
}
