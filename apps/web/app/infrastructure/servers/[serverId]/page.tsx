import { getCommandHistory, getMaintenanceHistory, getServer } from '../../../../lib/api'
import { actOnContainer, actOnServer, actOnService, beginMaintenance, finishMaintenance } from './actions'

export default async function ServerPage({ params }: { params: { serverId: string } }) {
  const [server, commands, maintenance] = await Promise.all([
    getServer(params.serverId),
    getCommandHistory(params.serverId),
    getMaintenanceHistory(params.serverId),
  ])
  const snapshot = server.snapshot
  const now = Date.now()
  const activeMaintenance = maintenance.find((window) =>
    !window.ended_at && new Date(window.starts_at).getTime() <= now && new Date(window.ends_at).getTime() > now,
  )

  return (
    <main>
      <h1>{server.hostname} {server.online ? 'ONLINE' : 'OFFLINE'}</h1>
      <p>Last heartbeat: {server.last_heartbeat ?? 'never'}</p>
      <p>OS: {snapshot?.os ?? 'unknown'}</p>
      <p>Kernel: {snapshot?.kernel ?? 'unknown'}</p>
      <p>Architecture: {snapshot?.architecture ?? 'unknown'}</p>
      <p>Agent version: {snapshot?.agent_version ?? 'unknown'}</p>
      <p>CPU: {snapshot ? `${snapshot.cpu_percent.toFixed(1)}%` : '—'}</p>
      <p>RAM: {snapshot ? `${snapshot.ram_percent.toFixed(1)}%` : '—'}</p>
      <p>Disk: {snapshot ? `${snapshot.disk_percent.toFixed(1)}%` : '—'}</p>
      <p>Load: {snapshot?.load ?? '—'}</p>
      <p>Uptime: {snapshot ? `${snapshot.uptime_seconds}s` : '—'}</p>

      <h2>Diagnostics</h2>
      <p>Failed systemd units: {snapshot?.diagnostics.failed_units.length ?? 0}</p>
      {snapshot?.diagnostics.failed_units.length ? (
        <ul>{snapshot.diagnostics.failed_units.map((unit) => <li key={unit}>{unit}</li>)}</ul>
      ) : null}
      <p>Listening TCP ports: {snapshot?.diagnostics.listening_tcp_ports.join(', ') || 'none detected'}</p>

      <h3>Recent service logs</h3>
      {snapshot?.diagnostics.journals.length ? snapshot.diagnostics.journals.map((journal) => (
        <section key={journal.service}>
          <h4>{journal.service}</h4>
          <pre>{journal.output || 'No recent journal entries.'}</pre>
        </section>
      )) : <p>No journal snapshots available.</p>}

      <h2>Containers</h2>
      {!snapshot?.docker.available ? <p>Docker is unavailable on this server.</p> : null}
      {snapshot?.docker.available && snapshot.docker.containers.length === 0 ? <p>No containers found.</p> : null}
      {snapshot?.docker.containers.map((container) => (
        <section key={container.id}>
          <h3>{container.name}</h3>
          <p>{container.image}</p>
          <p>{container.state} — {container.status}</p>
          <p>Ports: {container.ports || 'none'}</p>
          {container.state === 'running' ? (
            <form action={async () => { 'use server'; await actOnContainer(server.server_id, container.id, 'stop') }}>
              <button type="submit">Stop</button>
            </form>
          ) : (
            <form action={async () => { 'use server'; await actOnContainer(server.server_id, container.id, 'start') }}>
              <button type="submit">Start</button>
            </form>
          )}
          <form action={async () => { 'use server'; await actOnContainer(server.server_id, container.id, 'restart') }}>
            <button type="submit">Restart</button>
          </form>
        </section>
      ))}

      <h2>Maintenance</h2>
      {activeMaintenance ? (
        <>
          <p>ACTIVE until {activeMaintenance.ends_at} — {activeMaintenance.reason}</p>
          <form action={async () => { 'use server'; await finishMaintenance(server.server_id) }}><button type="submit">End maintenance</button></form>
        </>
      ) : (
        <>
          <p>No active maintenance window.</p>
          <form action={async () => { 'use server'; await beginMaintenance(server.server_id, 30, 'Manual server maintenance') }}><button type="submit">Start 30 minute maintenance</button></form>
          <form action={async () => { 'use server'; await beginMaintenance(server.server_id, 60, 'Manual server maintenance') }}><button type="submit">Start 60 minute maintenance</button></form>
        </>
      )}

      <h2>Updates</h2>
      {snapshot?.updates.supported ? (
        <><p>Pending package updates: {snapshot.updates.pending_updates}</p><p>Reboot required: {snapshot.updates.reboot_required ? 'YES' : 'NO'}</p></>
      ) : <p>APT update inventory unavailable on this server.</p>}
      <form action={async () => { 'use server'; await actOnServer(server.server_id, 'packages.refresh') }}><button type="submit">Check for updates</button></form>
      <form action={async () => { 'use server'; await actOnServer(server.server_id, 'packages.upgrade.security') }}><button type="submit" disabled={!activeMaintenance}>Install security updates</button></form>
      <form action={async () => { 'use server'; await actOnServer(server.server_id, 'packages.upgrade.all') }}><button type="submit" disabled={!activeMaintenance}>Install all updates</button></form>
      <form action={async () => { 'use server'; await actOnServer(server.server_id, 'system.reboot') }}><button type="submit" disabled={!activeMaintenance}>Reboot server</button></form>
      {!activeMaintenance && <p>Package upgrades and reboot require an active maintenance window.</p>}

      <h2>Services</h2>
      <ul>
        {server.services.map((service) => (
          <li key={service.name}>
            {service.name} — {service.status}
            {service.status === 'active' ? (
              <form action={async () => { 'use server'; await actOnService(server.server_id, service.name, 'stop') }}><button type="submit">Stop</button></form>
            ) : (
              <form action={async () => { 'use server'; await actOnService(server.server_id, service.name, 'start') }}><button type="submit">Start</button></form>
            )}
            <form action={async () => { 'use server'; await actOnService(server.server_id, service.name, 'restart') }}><button type="submit">Restart</button></form>
          </li>
        ))}
      </ul>

      <h2>Recent commands</h2>
      <ul>
        {commands.map((item) => (
          <li key={item.command.id}>
            {item.command.command_type.kind} {item.command.command_type.service ?? item.command.command_type.container ?? ''} — {item.command.status}
            {item.error_code ? ` (${item.error_code}: ${item.error_message ?? ''})` : ''}
          </li>
        ))}
      </ul>

      <h2>Maintenance history</h2>
      <ul>{maintenance.slice(0, 10).map((window) => <li key={window.id}>{window.starts_at} → {window.ended_at ?? window.ends_at}: {window.reason}</li>)}</ul>
    </main>
  )
}
