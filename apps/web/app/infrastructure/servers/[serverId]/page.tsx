import Link from 'next/link'
import type { CSSProperties } from 'react'
import { getCommandHistory, getDesiredState, getMaintenanceHistory, getServer } from '../../../../lib/api'
import {
  actOnContainer,
  actOnServer,
  actOnService,
  applySystemConfigRestore,
  beginMaintenance,
  createSystemConfigBackup,
  enforceDesiredFirewall,
  finishMaintenance,
  preflightSystemConfigRestore,
  saveDesiredState,
  verifySystemConfigBackup,
} from './actions'

function formatDate(value: string | null | undefined): string {
  if (!value) return 'Never'
  return `${new Date(value).toLocaleString('en-GB', {
    day: '2-digit',
    month: 'short',
    year: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
    timeZone: 'UTC',
  })} UTC`
}

function formatDuration(seconds: number | undefined): string {
  if (typeof seconds !== 'number') return '—'
  const days = Math.floor(seconds / 86400)
  const hours = Math.floor((seconds % 86400) / 3600)
  const minutes = Math.floor((seconds % 3600) / 60)
  if (days > 0) return `${days}d ${hours}h`
  if (hours > 0) return `${hours}h ${minutes}m`
  return `${minutes}m`
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 ** 2) return `${(bytes / 1024).toFixed(1)} KB`
  if (bytes < 1024 ** 3) return `${(bytes / 1024 ** 2).toFixed(1)} MB`
  return `${(bytes / 1024 ** 3).toFixed(1)} GB`
}

function osIcon(os: string | undefined): { src: string; label: string } | null {
  const value = (os ?? '').toLowerCase()
  if (value.includes('ubuntu')) return { src: 'https://cdn.simpleicons.org/ubuntu?viewbox=auto', label: 'Ubuntu' }
  if (value.includes('debian')) return { src: 'https://cdn.simpleicons.org/debian?viewbox=auto', label: 'Debian' }
  if (value.includes('alpine')) return { src: 'https://cdn.simpleicons.org/alpinelinux?viewbox=auto', label: 'Alpine Linux' }
  if (value.includes('arch')) return { src: 'https://cdn.simpleicons.org/archlinux?viewbox=auto', label: 'Arch Linux' }
  if (value.includes('fedora')) return { src: 'https://cdn.simpleicons.org/fedora?viewbox=auto', label: 'Fedora' }
  if (value.includes('centos')) return { src: 'https://cdn.simpleicons.org/centos?viewbox=auto', label: 'CentOS' }
  if (value.includes('linux')) return { src: 'https://cdn.simpleicons.org/linux/e8ebf2?viewbox=auto', label: 'Linux' }
  return null
}

function MetricGauge({ label, value }: { label: string; value: number | undefined }) {
  const safeValue = typeof value === 'number' ? Math.max(0, Math.min(100, value)) : 0
  return (
    <div className="metric-gauge">
      <div className="metric-ring" style={{ '--metric': `${safeValue}%` } as CSSProperties}>
        <span className="metric-ring-value">{typeof value === 'number' ? `${value.toFixed(1)}%` : '—'}</span>
      </div>
      <span className="metric-gauge-label">{label}</span>
    </div>
  )
}

function statusBadge(
  value: boolean | null | undefined,
  positiveWhenTrue = true,
  enabled = 'Enabled',
  disabled = 'Disabled',
) {
  if (value === null || value === undefined) return <span className="badge">Unknown</span>
  const positive = positiveWhenTrue ? value : !value
  return <span className={`badge ${positive ? 'success' : 'warning'}`}>{value ? enabled : disabled}</span>
}

function firewallStatusClass(status: string): string {
  const value = status.trim().toLowerCase()
  return value === 'active' || value.endsWith(': active') ? 'success' : 'warning'
}

export default async function ServerPage({ params }: { params: { serverId: string } }) {
  const [server, commands, maintenance, desiredState] = await Promise.all([
    getServer(params.serverId),
    getCommandHistory(params.serverId),
    getMaintenanceHistory(params.serverId),
    getDesiredState(params.serverId),
  ])
  const snapshot = server.snapshot
  const now = Date.now()
  const activeMaintenance = maintenance.find(
    (window) =>
      !window.ended_at &&
      new Date(window.starts_at).getTime() <= now &&
      new Date(window.ends_at).getTime() > now,
  )
  const firewallDrift = desiredState.drift.some((item) => item.field === 'firewall_enabled')
  const reconcileMode = desiredState.policy.mode === 'ENFORCE'
  const distro = osIcon(snapshot?.os)

  return (
    <main className="server-page">
      <div className="page-header">
        <div>
          <Link className="panel-link" href="/infrastructure/servers">← Servers</Link>
          <div className="server-header-main">
            <div className="os-logo">
              {distro ? <img src={distro.src} alt={`${distro.label} logo`} /> : <strong>OS</strong>}
            </div>
            <div className="server-title-block">
              <span className="eyebrow">Infrastructure node</span>
              <h1>{server.hostname}</h1>
              <div className="server-title-meta">
                <span className={`badge ${server.online ? 'success' : 'danger'}`}><span className={`status-dot ${server.online ? 'online' : 'danger'}`} />{server.online ? 'Online' : 'Offline'}</span>
                {activeMaintenance ? <span className="badge warning">Maintenance active</span> : null}
                <span className="badge">Agent {snapshot?.agent_version ?? 'unknown'}</span>
              </div>
            </div>
          </div>
        </div>
      </div>

      <div className="server-overview-grid">
        <section className="server-overview-card">
          <h2 className="overview-card-title">System identity</h2>
          <div className="info-grid">
            <div className="info-item"><span className="info-label">Operating system</span><span className="info-value">{snapshot?.os ?? 'Unknown'}</span></div>
            <div className="info-item"><span className="info-label">Architecture</span><span className="info-value">{snapshot?.architecture ?? 'Unknown'}</span></div>
            <div className="info-item"><span className="info-label">Kernel</span><span className="info-value">{snapshot?.kernel ?? 'Unknown'}</span></div>
            <div className="info-item"><span className="info-label">Last heartbeat</span><span className="info-value">{formatDate(server.last_heartbeat)}</span></div>
          </div>
        </section>

        <section className="server-metrics-card">
          <h2 className="overview-card-title">Live utilization</h2>
          <div className="metric-grid">
            <MetricGauge label="CPU" value={snapshot?.cpu_percent} />
            <MetricGauge label="Memory" value={snapshot?.ram_percent} />
            <MetricGauge label="Disk" value={snapshot?.disk_percent} />
          </div>
          <div className="metric-secondary-grid">
            <div className="metric-secondary"><span className="info-label">Load average</span><strong>{snapshot?.load ?? '—'}</strong></div>
            <div className="metric-secondary"><span className="info-label">Uptime</span><strong>{formatDuration(snapshot?.uptime_seconds)}</strong></div>
          </div>
        </section>
      </div>

      <section className="detail-card">
        <div className="detail-card-header">
          <div><h2>Desired state &amp; drift</h2><p>Define safe host policy and see where the current machine differs from it.</p></div>
          <span className={`badge ${reconcileMode ? 'info' : ''}`}>{desiredState.policy.mode}</span>
        </div>
        <div className="detail-card-body">
          <form action={async (formData) => { 'use server'; await saveDesiredState(server.server_id, formData) }}>
            <div className="compact-form-grid">
              <label>Mode<select name="mode" defaultValue={desiredState.policy.mode}><option value="MONITOR">Monitor only</option><option value="ENFORCE">Reconcile supported fields</option></select></label>
              <label>Firewall<select name="firewall_enabled" defaultValue={desiredState.policy.firewall_enabled === null ? 'ignore' : String(desiredState.policy.firewall_enabled)}><option value="ignore">Do not manage</option><option value="true">Must be active</option><option value="false">Must be inactive</option></select></label>
              <label>SSH password authentication<select name="ssh_password_auth" defaultValue={desiredState.policy.ssh_password_auth === null ? 'ignore' : String(desiredState.policy.ssh_password_auth)}><option value="ignore">Do not manage</option><option value="false">Must be disabled</option><option value="true">Must be enabled</option></select></label>
              <label>SSH root login<select name="ssh_root_login" defaultValue={desiredState.policy.ssh_root_login ?? 'ignore'}><option value="ignore">Do not manage</option><option value="no">Must be disabled</option><option value="prohibit-password">Keys only</option><option value="yes">Allowed</option></select></label>
              <label>Automatic security updates<select name="automatic_security_updates" defaultValue={desiredState.policy.automatic_security_updates === null ? 'ignore' : String(desiredState.policy.automatic_security_updates)}><option value="ignore">Do not manage</option><option value="true">Must be enabled</option><option value="false">Must be disabled</option></select></label>
            </div>
            <button className="primary" type="submit">Save desired state</button>
          </form>

          <div className="callout">ENFORCE currently supports the safe V1 policy shape where the firewall must be active and SSH/update fields remain unmanaged. Reconciliation runs every 60 seconds and mutates firewall state only inside a maintenance window.</div>

          {desiredState.policy.firewall_enabled === true ? (
            <div className="action-row">
              {!reconcileMode ? <form action={async () => { 'use server'; await enforceDesiredFirewall(server.server_id) }}><button type="submit" disabled={!activeMaintenance || !firewallDrift || !snapshot?.security.available}>Enable firewall safely</button></form> : null}
              {!activeMaintenance && firewallDrift ? <span className="badge warning">Maintenance required for drift repair</span> : null}
              {!firewallDrift && snapshot?.security.available ? <span className="badge success">Firewall matches desired state</span> : null}
            </div>
          ) : null}

          <h3>Detected drift</h3>
          {desiredState.drift.length === 0 ? (
            <div className="callout success">No drift detected for configured policy fields.</div>
          ) : (
            <div className="resource-list">
              {desiredState.drift.map((item) => (
                <div className="resource-card" key={item.field}>
                  <div className="resource-card-head"><strong>{item.field.replaceAll('_', ' ')}</strong><span className={`badge ${item.severity === 'CRITICAL' ? 'danger' : item.severity === 'WARNING' ? 'warning' : 'info'}`}>{item.severity}</span></div>
                  <div className="resource-meta">Desired <code>{String(item.desired)}</code> · actual <code>{String(item.actual)}</code></div>
                </div>
              ))}
            </div>
          )}
        </div>
      </section>

      <div className="detail-split">
        <section className="detail-card">
          <div className="detail-card-header"><div><h2>Security posture</h2><p>Baseline hardening reported by the agent.</p></div></div>
          <div className="detail-card-body">
            {!snapshot?.security.available ? <div className="empty-state"><strong>Security inspection unavailable</strong>The agent did not report security data.</div> : (
              <>
                <div className="status-grid">
                  <div className="status-item"><span className="info-label">Firewall</span><span className="info-value"><span className={`badge ${firewallStatusClass(snapshot.security.firewall_status)}`}>{snapshot.security.firewall_status}</span></span></div>
                  <div className="status-item"><span className="info-label">SSH password</span><span className="info-value">{statusBadge(snapshot.security.ssh_password_auth, false)}</span></div>
                  <div className="status-item"><span className="info-label">SSH root login</span><span className="info-value"><span className={`badge ${snapshot.security.ssh_root_login === 'no' ? 'success' : 'warning'}`}>{snapshot.security.ssh_root_login}</span></span></div>
                  <div className="status-item"><span className="info-label">Security updates</span><span className="info-value">{statusBadge(snapshot.security.automatic_security_updates)}</span></div>
                </div>

                <h3>Findings</h3>
                {snapshot.security.findings.length === 0 ? <div className="callout success">No baseline security findings.</div> : (
                  <div className="resource-list">{snapshot.security.findings.map((finding) => <div className="resource-card" key={finding.code}><div className="resource-card-head"><strong>{finding.code}</strong><span className={`badge ${finding.severity === 'CRITICAL' ? 'danger' : finding.severity === 'WARNING' ? 'warning' : 'info'}`}>{finding.severity}</span></div><div className="resource-meta">{finding.message}</div></div>)}</div>
                )}

                <details className="log-details">
                  <summary>UFW rules · {snapshot.security.firewall_rules.length}</summary>
                  <pre>{snapshot.security.firewall_rules.length ? snapshot.security.firewall_rules.join('\n') : 'No UFW rules reported.'}</pre>
                </details>
              </>
            )}
          </div>
        </section>

        <section className="detail-card">
          <div className="detail-card-header"><div><h2>Diagnostics</h2><p>Service health, listening ports and recent journals.</p></div></div>
          <div className="detail-card-body">
            <div className="info-grid">
              <div className="info-item"><span className="info-label">Failed units</span><span className="info-value">{snapshot?.diagnostics.failed_units.length ?? 0}</span></div>
              <div className="info-item"><span className="info-label">Listening TCP ports</span><span className="info-value">{snapshot?.diagnostics.listening_tcp_ports.length ?? 0}</span></div>
            </div>

            {snapshot?.diagnostics.failed_units.length ? <ul className="chip-list">{snapshot.diagnostics.failed_units.map((unit) => <li className="chip" key={unit}>{unit}</li>)}</ul> : <div className="callout success">No failed systemd units.</div>}
            {snapshot?.diagnostics.listening_tcp_ports.length ? <ul className="chip-list">{snapshot.diagnostics.listening_tcp_ports.map((port) => <li className="chip" key={port}>{port}</li>)}</ul> : null}

            <h3>Recent service logs</h3>
            {snapshot?.diagnostics.journals.length ? snapshot.diagnostics.journals.map((journal) => (
              <details className="log-details" key={journal.service}>
                <summary>{journal.service}</summary>
                <pre>{journal.output || 'No recent journal entries.'}</pre>
              </details>
            )) : <div className="empty-state"><strong>No journal snapshots</strong>Recent service output will appear when reported by the agent.</div>}
          </div>
        </section>
      </div>

      <section className="detail-card">
        <div className="detail-card-header"><div><h2>Backups &amp; recovery</h2><p>System-security configuration snapshots with integrity verification and guarded restore.</p></div>{snapshot?.backups.available ? <span className="badge success">Target available</span> : <span className="badge warning">Unavailable</span>}</div>
        <div className="detail-card-body">
          {!snapshot?.backups.available ? <div className="empty-state"><strong>Backup target unavailable</strong>This server is not currently reporting a usable backup target.</div> : (
            <>
              <div className="info-grid">
                <div className="info-item"><span className="info-label">Target</span><span className="info-value">{snapshot.backups.target}</span></div>
                <div className="info-item"><span className="info-label">Profile</span><span className="info-value">System security configuration</span></div>
              </div>
              <div className="action-row"><form action={async () => { 'use server'; await createSystemConfigBackup(server.server_id) }}><button className="primary" type="submit">Create backup</button></form></div>
              {snapshot.backups.artifacts.length === 0 ? <div className="empty-state"><strong>No backups found</strong>Create the first system configuration snapshot.</div> : (
                <div className="resource-list">
                  {snapshot.backups.artifacts.map((backup) => {
                    const restoreAllowed = Boolean(activeMaintenance && backup.verified)
                    return (
                      <article className="resource-card" key={backup.name}>
                        <div className="resource-card-head"><div><h3>{backup.name}</h3><div className="resource-meta">{backup.profile} · {formatBytes(backup.size_bytes)} · SHA-256 {backup.sha256 || 'missing'}</div></div><span className={`badge ${backup.verified ? 'success' : 'warning'}`}>{backup.verified ? 'Verified' : 'Unverified'}</span></div>
                        <div className="action-row">
                          <form action={async () => { 'use server'; await verifySystemConfigBackup(server.server_id, backup.name) }}><button type="submit">Verify integrity</button></form>
                          <form action={async () => { 'use server'; await preflightSystemConfigRestore(server.server_id, backup.name) }}><button type="submit">Validate restore</button></form>
                        </div>
                        <form action={async (formData) => { 'use server'; await applySystemConfigRestore(server.server_id, backup.name, formData) }}>
                          <label>Type <code>{backup.name}</code> to confirm<input name="confirmation" required autoComplete="off" disabled={!restoreAllowed} /></label>
                          <button className="danger" type="submit" disabled={!restoreAllowed}>Restore configuration</button>
                        </form>
                        {!activeMaintenance ? <div className="callout warning">Live restore requires an active maintenance window.</div> : null}
                        {!backup.verified ? <div className="callout warning">Verify this backup before live restore.</div> : null}
                      </article>
                    )
                  })}
                </div>
              )}
              <div className="callout danger">Live restore is a critical operation. Argus re-runs preflight, snapshots current SSH/UFW/update configuration and arms a 120-second local rollback before applying allowlisted paths.</div>
            </>
          )}
        </div>
      </section>

      <section className="detail-card">
        <div className="detail-card-header"><div><h2>Containers</h2><p>Docker workloads discovered on this host.</p></div><span className="badge info">{snapshot?.docker.containers.length ?? 0} workloads</span></div>
        <div className="detail-card-body">
          {!snapshot?.docker.available ? <div className="empty-state"><strong>Docker unavailable</strong>This server is not reporting a Docker runtime.</div> : null}
          {snapshot?.docker.available && snapshot.docker.containers.length === 0 ? <div className="empty-state"><strong>No containers</strong>No Docker workloads were discovered.</div> : null}
          {snapshot?.docker.containers.length ? <div className="resource-list">{snapshot.docker.containers.map((container) => (
            <article className="resource-card" key={container.id}>
              <div className="resource-card-head"><div><h3>{container.name}</h3><div className="resource-meta">{container.image} · {container.ports || 'No published ports'}</div></div><span className={`badge ${container.state === 'running' ? 'success' : 'warning'}`}>{container.state}</span></div>
              <div className="resource-meta">{container.status}</div>
              <div className="action-row">
                {container.state === 'running' ? <form action={async () => { 'use server'; await actOnContainer(server.server_id, container.id, 'stop') }}><button type="submit">Stop</button></form> : <form action={async () => { 'use server'; await actOnContainer(server.server_id, container.id, 'start') }}><button type="submit">Start</button></form>}
                <form action={async () => { 'use server'; await actOnContainer(server.server_id, container.id, 'restart') }}><button type="submit">Restart</button></form>
              </div>
            </article>
          ))}</div> : null}
        </div>
      </section>

      <div className="detail-split">
        <section className="detail-card">
          <div className="detail-card-header"><div><h2>Maintenance</h2><p>Guard disruptive operations behind an explicit maintenance window.</p></div>{activeMaintenance ? <span className="badge warning">Active</span> : <span className="badge">Inactive</span>}</div>
          <div className="detail-card-body">
            {activeMaintenance ? <div className="callout warning"><strong>Until {formatDate(activeMaintenance.ends_at)}</strong><br />{activeMaintenance.reason}</div> : <div className="callout success">No active maintenance window.</div>}
            <div className="action-row">
              {activeMaintenance ? <form action={async () => { 'use server'; await finishMaintenance(server.server_id) }}><button type="submit">End maintenance</button></form> : <><form action={async () => { 'use server'; await beginMaintenance(server.server_id, 30, 'Manual server maintenance') }}><button type="submit">Start 30 min</button></form><form action={async () => { 'use server'; await beginMaintenance(server.server_id, 60, 'Manual server maintenance') }}><button type="submit">Start 60 min</button></form></>}
            </div>
          </div>
        </section>

        <section className="detail-card">
          <div className="detail-card-header"><div><h2>System updates</h2><p>APT inventory and guarded package operations.</p></div>{snapshot?.updates.reboot_required ? <span className="badge warning">Reboot required</span> : null}</div>
          <div className="detail-card-body">
            {snapshot?.updates.supported ? <div className="info-grid"><div className="info-item"><span className="info-label">Pending packages</span><span className="info-value">{snapshot.updates.pending_updates}</span></div><div className="info-item"><span className="info-label">Reboot</span><span className="info-value">{snapshot.updates.reboot_required ? 'Required' : 'Not required'}</span></div></div> : <div className="callout warning">APT update inventory unavailable on this server.</div>}
            <div className="action-row">
              <form action={async () => { 'use server'; await actOnServer(server.server_id, 'packages.refresh') }}><button type="submit">Check updates</button></form>
              <form action={async () => { 'use server'; await actOnServer(server.server_id, 'packages.upgrade.security') }}><button type="submit" disabled={!activeMaintenance}>Security updates</button></form>
              <form action={async () => { 'use server'; await actOnServer(server.server_id, 'packages.upgrade.all') }}><button type="submit" disabled={!activeMaintenance}>Install all</button></form>
              <form action={async () => { 'use server'; await actOnServer(server.server_id, 'system.reboot') }}><button className="danger" type="submit" disabled={!activeMaintenance}>Reboot</button></form>
            </div>
            {!activeMaintenance ? <div className="callout">Package upgrades and reboot require an active maintenance window.</div> : null}
          </div>
        </section>
      </div>

      <section className="detail-card">
        <div className="detail-card-header"><div><h2>Services</h2><p>System services tracked by the Argus agent.</p></div><span className="badge">{server.services.length} services</span></div>
        <div className="detail-card-body">
          {server.services.length === 0 ? <div className="empty-state"><strong>No tracked services</strong>Tracked system services will appear here.</div> : <div className="resource-list">{server.services.map((service) => (
            <div className="resource-card" key={service.name}>
              <div className="resource-card-head"><div><h3>{service.name}</h3></div><span className={`badge ${service.status === 'active' ? 'success' : 'warning'}`}>{service.status}</span></div>
              <div className="action-row">
                {service.status === 'active' ? <form action={async () => { 'use server'; await actOnService(server.server_id, service.name, 'stop') }}><button type="submit">Stop</button></form> : <form action={async () => { 'use server'; await actOnService(server.server_id, service.name, 'start') }}><button type="submit">Start</button></form>}
                <form action={async () => { 'use server'; await actOnService(server.server_id, service.name, 'restart') }}><button type="submit">Restart</button></form>
              </div>
            </div>
          ))}</div>}
        </div>
      </section>

      <div className="detail-split">
        <section className="detail-card">
          <div className="detail-card-header"><div><h2>Recent commands</h2><p>Latest privileged operations sent to this host.</p></div></div>
          <div className="detail-card-body">
            {commands.length === 0 ? <div className="empty-state"><strong>No commands yet</strong>Agent operations will appear here.</div> : <ol className="timeline">{commands.slice(0, 12).map((item) => {
              const target = item.command.command_type.service ?? item.command.command_type.container ?? item.command.command_type.backup ?? item.command.command_type.profile ?? ''
              return <li className="timeline-item" key={item.command.id}><div className="timeline-title">{item.command.command_type.kind} {target}</div><div className="timeline-meta"><span className={`badge ${item.command.status === 'SUCCEEDED' ? 'success' : item.command.status === 'FAILED' ? 'danger' : 'info'}`}>{item.command.status}</span></div>{item.error_code ? <div className="timeline-message text-danger">{item.error_code}: {item.error_message ?? ''}</div> : null}</li>
            })}</ol>}
          </div>
        </section>

        <section className="detail-card">
          <div className="detail-card-header"><div><h2>Maintenance history</h2><p>Recent maintenance windows for this server.</p></div></div>
          <div className="detail-card-body">
            {maintenance.length === 0 ? <div className="empty-state"><strong>No maintenance history</strong>Completed maintenance windows will appear here.</div> : <ol className="timeline">{maintenance.slice(0, 10).map((window) => <li className="timeline-item" key={window.id}><div className="timeline-title">{window.reason}</div><div className="timeline-meta">{formatDate(window.starts_at)} → {formatDate(window.ended_at ?? window.ends_at)}</div></li>)}</ol>}
          </div>
        </section>
      </div>
    </main>
  )
}
