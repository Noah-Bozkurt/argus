import Link from 'next/link'
import { getServers } from '../../../lib/api'

export default async function ServersPage() {
  const servers = await getServers()
  return (
    <main>
      <h1>Infrastructure / Servers</h1>
      <ul>
        {servers.map((server) => (
          <li key={server.server_id}>
            <Link href={`/infrastructure/servers/${server.server_id}`}>
              {server.hostname} {server.online ? 'ONLINE' : 'OFFLINE'}
            </Link>
          </li>
        ))}
      </ul>
    </main>
  )
}
