import { getServers } from '../../../lib/api'
import ServerFleet from './server-fleet'

export default async function ServersPage() {
  const servers = await getServers()

  return (
    <main>
      <div className="page-header compact-page-header">
        <div>
          <h1>Servers</h1>
          <p>Host health, utilization and connected workloads reported by Argus agents.</p>
        </div>
      </div>

      <ServerFleet initialServers={servers} />
    </main>
  )
}
