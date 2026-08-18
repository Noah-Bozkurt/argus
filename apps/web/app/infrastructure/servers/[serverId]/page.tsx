import { getServer } from '../../../../lib/api'
import { restart } from './actions'

export default async function ServerPage({ params }: { params: { serverId: string } }) {
  const server = await getServer(params.serverId)

  return (
    <main>
      <h1>{server.hostname} {server.online ? 'ONLINE' : 'OFFLINE'}</h1>
      <p>{server.os}</p>
      <p>Agent version: {server.agentVersion}</p>
      <p>CPU: {server.cpuPercent}%</p>
      <p>RAM: {server.ramPercent}%</p>
      <p>Disk: {server.diskPercent}%</p>
      <p>Load: {server.load}</p>
      <p>Uptime: {server.uptimeSeconds}s</p>

      <h2>Services</h2>
      <ul>
        {server.services.map((service) => (
          <li key={service.name}>
            {service.name} {service.status}
            <form action={async () => { 'use server'; await restart(server.serverId, service.name) }}>
              <button type="submit">Restart</button>
            </form>
          </li>
        ))}
      </ul>
    </main>
  )
}
