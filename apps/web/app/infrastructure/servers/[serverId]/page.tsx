import { getCommandHistory, getServer } from '../../../../lib/api'
import { actOnService } from './actions'

export default async function ServerPage({ params }: { params: { serverId: string } }) {
  const [server, commands] = await Promise.all([
    getServer(params.serverId),
    getCommandHistory(params.serverId),
  ])
  const snapshot = server.snapshot

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

      <h2>Updates</h2>
      {snapshot?.updates.supported ? (
        <>
          <p>Pending package updates: {snapshot.updates.pending_updates}</p>
          <p>Reboot required: {snapshot.updates.reboot_required ? 'YES' : 'NO'}</p>
        </>
      ) : (
        <p>APT update inventory unavailable on this server.</p>
      )}

      <h2>Services</h2>
      <ul>
        {server.services.map((service) => (
          <li key={service.name}>
            {service.name} — {service.status}
            {service.status === 'active' ? (
              <form action={async () => { 'use server'; await actOnService(server.server_id, service.name, 'stop') }}>
                <button type="submit">Stop</button>
              </form>
            ) : (
              <form action={async () => { 'use server'; await actOnService(server.server_id, service.name, 'start') }}>
                <button type="submit">Start</button>
              </form>
            )}
            <form action={async () => { 'use server'; await actOnService(server.server_id, service.name, 'restart') }}>
              <button type="submit">Restart</button>
            </form>
          </li>
        ))}
      </ul>

      <h2>Recent commands</h2>
      <ul>
        {commands.map((item) => (
          <li key={item.command.id}>
            {item.command.command_type.kind} {item.command.command_type.service ?? ''} — {item.command.status}
            {item.error_code ? ` (${item.error_code}: ${item.error_message ?? ''})` : ''}
          </li>
        ))}
      </ul>
    </main>
  )
}
