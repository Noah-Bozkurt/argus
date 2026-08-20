import { getProjectEnvironments, getProjectRepositories, getProjectServices, getServers } from '../../../lib/api'
import { createCatalogServiceAction, deleteCatalogServiceAction, updateCatalogServiceAction } from './service-actions'

const serviceTypes = ['web', 'api', 'worker', 'database', 'queue', 'cron', 'other'] as const

function healthStatusClass(status: string): string {
  const value = status.toLowerCase()
  if (value.includes('unhealthy') || value.includes('fail') || value.includes('error')) return 'danger'
  if (value.includes('degraded') || value.includes('warning')) return 'warning'
  if (value.includes('healthy') || value.includes('active') || value.includes('ok')) return 'success'
  return ''
}

export default async function ServiceCatalogSection({ projectId }: { projectId: string }) {
  const [services, repositories, environments, allServers] = await Promise.all([
    getProjectServices(projectId),
    getProjectRepositories(projectId),
    getProjectEnvironments(projectId),
    getServers(),
  ])
  const servers = allServers.filter((server) => server.project_id === projectId)
  const repositoryNames = new Map(repositories.map((repository) => [repository.id, `${repository.owner}/${repository.name}`]))
  const environmentNames = new Map(environments.map((environment) => [environment.id, environment.name]))
  const serverNames = new Map(servers.map((server) => [server.server_id, server.hostname]))

  return (
    <section>
      <h2>Service catalog</h2>
      <p>Project-owned application components such as web apps, APIs, workers and databases. These are separate from host-level systemd services.</p>

      <h3>Services</h3>
      <details className="create-drawer">
        <summary className="button">+ Add service</summary>
        <div className="drawer-content">
          <form action={async (formData) => { 'use server'; await createCatalogServiceAction(projectId, formData) }}>
            <div className="form-grid">
              <label>Name<input name="name" required maxLength={160} placeholder="Control API" /></label>
              <label>Type<select name="service_type" defaultValue="web">{serviceTypes.map((type) => <option key={type} value={type}>{type}</option>)}</select></label>
              <label>Runtime<input name="runtime" maxLength={120} placeholder="Rust / Axum" /></label>
              <label>Endpoint<input name="endpoint_url" type="url" maxLength={2048} placeholder="https://api.example.com" /></label>
              <label>Repository<select name="repository_id" defaultValue=""><option value="">Unlinked</option>{repositories.map((repository) => <option key={repository.id} value={repository.id}>{repository.owner}/{repository.name}</option>)}</select></label>
              <label>Environment<select name="environment_id" defaultValue=""><option value="">Unassigned</option>{environments.map((environment) => <option key={environment.id} value={environment.id}>{environment.name}</option>)}</select></label>
              <label>Server<select name="server_id" defaultValue=""><option value="">Unassigned</option>{servers.map((server) => <option key={server.server_id} value={server.server_id}>{server.hostname} — {environmentNames.get(server.environment_id) ?? 'unknown environment'}</option>)}</select></label>
              <label className="full">Description<textarea name="description" maxLength={8000} /></label>
            </div>
            <div className="callout">Selecting a server automatically uses that server&apos;s environment.</div>
            <button className="primary" type="submit">Add service</button>
          </form>
        </div>
      </details>

      {services.length === 0 ? <div className="empty-state"><strong>No catalog services</strong>Add the first application component for this project.</div> : (
        services.map((service) => (
          <article key={service.id}>
            <div className="resource-card-head">
              <div><h4>{service.name}</h4><div className="resource-meta">{service.description || 'No description'}</div></div>
              <div className="action-row"><span className="badge info">{service.service_type}</span><span className={`badge ${service.lifecycle_status === 'ACTIVE' ? 'success' : service.lifecycle_status === 'PAUSED' ? 'warning' : ''}`}>{service.lifecycle_status}</span><span className={`badge ${healthStatusClass(service.health_status)}`}>{service.health_status}</span></div>
            </div>
            <div className="info-grid" style={{ marginTop: 12 }}>
              <div className="info-item"><span className="info-label">Repository</span><span className="info-value">{service.repository_id ? repositoryNames.get(service.repository_id) ?? service.repository_id : 'Unlinked'}</span></div>
              <div className="info-item"><span className="info-label">Environment</span><span className="info-value">{service.environment_id ? environmentNames.get(service.environment_id) ?? service.environment_id : 'Unassigned'}</span></div>
              <div className="info-item"><span className="info-label">Server</span><span className="info-value">{service.server_id ? serverNames.get(service.server_id) ?? service.server_id : 'Unassigned'}</span></div>
              <div className="info-item"><span className="info-label">Runtime</span><span className="info-value">{service.runtime ?? 'Not specified'}</span></div>
            </div>
            {service.endpoint_url ? <div className="action-row"><a className="button small" href={service.endpoint_url} target="_blank" rel="noreferrer">Open endpoint ↗</a></div> : null}
            <details className="resource-editor">
              <summary className="button small">Edit service</summary>
              <div className="resource-editor-body">
                <form action={async (formData) => { 'use server'; await updateCatalogServiceAction(projectId, service.id, service.owner_user_id, formData) }}>
                  <div className="form-grid">
                    <label>Name<input name="name" required maxLength={160} defaultValue={service.name} /></label>
                    <label>Type<select name="service_type" defaultValue={service.service_type}>{serviceTypes.map((type) => <option key={type} value={type}>{type}</option>)}</select></label>
                    <label>Runtime<input name="runtime" maxLength={120} defaultValue={service.runtime ?? ''} /></label>
                    <label>Endpoint<input name="endpoint_url" type="url" maxLength={2048} defaultValue={service.endpoint_url ?? ''} /></label>
                    <label>Repository<select name="repository_id" defaultValue={service.repository_id ?? ''}><option value="">Unlinked</option>{repositories.map((repository) => <option key={repository.id} value={repository.id}>{repository.owner}/{repository.name}</option>)}</select></label>
                    <label>Environment<select name="environment_id" defaultValue={service.environment_id ?? ''}><option value="">Unassigned</option>{environments.map((environment) => <option key={environment.id} value={environment.id}>{environment.name}</option>)}</select></label>
                    <label>Server<select name="server_id" defaultValue={service.server_id ?? ''}><option value="">Unassigned</option>{servers.map((server) => <option key={server.server_id} value={server.server_id}>{server.hostname} — {environmentNames.get(server.environment_id) ?? 'unknown environment'}</option>)}</select></label>
                    <label>Lifecycle<select name="lifecycle_status" defaultValue={service.lifecycle_status}><option value="ACTIVE">Active</option><option value="PAUSED">Paused</option><option value="ARCHIVED">Archived</option></select></label>
                    <label className="full">Description<textarea name="description" maxLength={8000} defaultValue={service.description} /></label>
                  </div>
                  <button type="submit">Save service</button>
                </form>
                <form action={async () => { 'use server'; await deleteCatalogServiceAction(projectId, service.id) }}><button className="danger" type="submit">Delete service</button></form>
              </div>
            </details>
          </article>
        ))
      )}
    </section>
  )
}
