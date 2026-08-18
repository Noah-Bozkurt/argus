import { getProjectRepositories, getProjectServices } from '../../../lib/api'
import {
  createCatalogServiceAction,
  deleteCatalogServiceAction,
  updateCatalogServiceAction,
} from './service-actions'

const serviceTypes = ['web', 'api', 'worker', 'database', 'queue', 'cron', 'other'] as const

export default async function ServiceCatalogSection({ projectId }: { projectId: string }) {
  const [services, repositories] = await Promise.all([
    getProjectServices(projectId),
    getProjectRepositories(projectId),
  ])
  const repositoryNames = new Map(
    repositories.map((repository) => [repository.id, `${repository.owner}/${repository.name}`]),
  )

  return (
    <section>
      <h2>Service Catalog</h2>
      <p>
        Services are project-owned application components such as a web app, API, worker or database.
        They are separate from host-level systemd services.
      </p>

      <h3>Add service</h3>
      <form action={async (formData) => { 'use server'; await createCatalogServiceAction(projectId, formData) }}>
        <label>
          Name
          <input name="name" required maxLength={160} placeholder="Control API" />
        </label>
        <label>
          Type
          <select name="service_type" defaultValue="web">
            {serviceTypes.map((type) => <option key={type} value={type}>{type}</option>)}
          </select>
        </label>
        <label>
          Description
          <textarea name="description" maxLength={8000} />
        </label>
        <label>
          Runtime
          <input name="runtime" maxLength={120} placeholder="Rust / Axum" />
        </label>
        <label>
          Repository
          <select name="repository_id" defaultValue="">
            <option value="">Unlinked</option>
            {repositories.map((repository) => (
              <option key={repository.id} value={repository.id}>
                {repository.owner}/{repository.name}
              </option>
            ))}
          </select>
        </label>
        <label>
          Endpoint
          <input name="endpoint_url" type="url" maxLength={2048} placeholder="https://api.example.com" />
        </label>
        <button type="submit">Add service</button>
      </form>

      <h3>Services</h3>
      {services.length === 0 ? <p>No catalog services yet.</p> : (
        services.map((service) => (
          <article key={service.id}>
            <h4>{service.name}</h4>
            <p>
              {service.service_type} — lifecycle {service.lifecycle_status} — health {service.health_status}
            </p>
            <p>
              Repository: {service.repository_id ? repositoryNames.get(service.repository_id) ?? service.repository_id : 'unlinked'}
              {' — '}Environment: {service.environment_id ?? 'unassigned'}
              {' — '}Server: {service.server_id ?? 'unassigned'}
            </p>
            {service.endpoint_url ? (
              <p><a href={service.endpoint_url} target="_blank" rel="noreferrer">Open endpoint</a></p>
            ) : null}
            <form action={async (formData) => {
              'use server'
              await updateCatalogServiceAction(
                projectId,
                service.id,
                service.environment_id,
                service.server_id,
                service.owner_user_id,
                formData,
              )
            }}>
              <label>
                Name
                <input name="name" required maxLength={160} defaultValue={service.name} />
              </label>
              <label>
                Type
                <select name="service_type" defaultValue={service.service_type}>
                  {serviceTypes.map((type) => <option key={type} value={type}>{type}</option>)}
                </select>
              </label>
              <label>
                Description
                <textarea name="description" maxLength={8000} defaultValue={service.description} />
              </label>
              <label>
                Runtime
                <input name="runtime" maxLength={120} defaultValue={service.runtime ?? ''} />
              </label>
              <label>
                Repository
                <select name="repository_id" defaultValue={service.repository_id ?? ''}>
                  <option value="">Unlinked</option>
                  {repositories.map((repository) => (
                    <option key={repository.id} value={repository.id}>
                      {repository.owner}/{repository.name}
                    </option>
                  ))}
                </select>
              </label>
              <label>
                Endpoint
                <input name="endpoint_url" type="url" maxLength={2048} defaultValue={service.endpoint_url ?? ''} />
              </label>
              <label>
                Lifecycle
                <select name="lifecycle_status" defaultValue={service.lifecycle_status}>
                  <option value="ACTIVE">Active</option>
                  <option value="PAUSED">Paused</option>
                  <option value="ARCHIVED">Archived</option>
                </select>
              </label>
              <button type="submit">Save service</button>
            </form>
            <form action={async () => { 'use server'; await deleteCatalogServiceAction(projectId, service.id) }}>
              <button type="submit">Delete service</button>
            </form>
          </article>
        ))
      )}
    </section>
  )
}
