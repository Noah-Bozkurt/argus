import { getDeploymentReleaseView } from '../../../lib/deployments-releases-api'
import {
  getProjectEnvironments,
  getProjectRepositories,
  getProjectServices,
} from '../../../lib/api'
import {
  addReleaseComponentAction,
  createDeploymentAction,
  createReleaseAction,
  updateDeploymentStatusAction,
  updateReleaseStatusAction,
} from './deployment-release-actions'

function formatDate(value: string | null): string {
  return value ? new Date(value).toLocaleString() : '—'
}

export default async function DeploymentsReleasesSection({ projectId }: { projectId: string }) {
  const [view, services, environments, repositories] = await Promise.all([
    getDeploymentReleaseView(projectId),
    getProjectServices(projectId),
    getProjectEnvironments(projectId),
    getProjectRepositories(projectId),
  ])
  const serviceNames = new Map(services.map((service) => [service.id, service.name]))
  const environmentNames = new Map(environments.map((environment) => [environment.id, environment.name]))
  const repositoryNames = new Map(repositories.map((repository) => [repository.id, `${repository.owner}/${repository.name}`]))

  return (
    <section>
      <h2>Deployments &amp; Releases</h2>
      <p>
        V1 records deployment attempts and releases. Provider execution is intentionally not implemented yet; these records establish immutable history and valid lifecycle transitions first.
      </p>

      <h3>Record deployment</h3>
      <form action={async (formData) => { 'use server'; await createDeploymentAction(projectId, formData) }}>
        <label>
          Service
          <select name="service_id" required defaultValue="">
            <option value="" disabled>Select service</option>
            {services.map((service) => <option key={service.id} value={service.id}>{service.name}</option>)}
          </select>
        </label>
        <label>
          Environment
          <select name="environment_id" required defaultValue="">
            <option value="" disabled>Select environment</option>
            {environments.map((environment) => <option key={environment.id} value={environment.id}>{environment.name}</option>)}
          </select>
        </label>
        <label>
          Repository
          <select name="repository_id" defaultValue="">
            <option value="">Use service repository / none</option>
            {repositories.map((repository) => (
              <option key={repository.id} value={repository.id}>{repository.owner}/{repository.name}</option>
            ))}
          </select>
        </label>
        <label>
          Commit SHA
          <input name="source_commit_sha" minLength={7} maxLength={64} placeholder="abcdef123456" />
        </label>
        <label>
          Version
          <input name="source_version" maxLength={120} placeholder="1.4.0" />
        </label>
        <label>
          Rollback of
          <select name="rollback_of_deployment_id" defaultValue="">
            <option value="">Normal deployment</option>
            {view.deployments.filter((deployment) => deployment.status === 'SUCCEEDED').map((deployment) => (
              <option key={deployment.id} value={deployment.id}>
                {serviceNames.get(deployment.service_id) ?? deployment.service_id} → {environmentNames.get(deployment.environment_id) ?? deployment.environment_id} — {deployment.id.slice(0, 8)}
              </option>
            ))}
          </select>
        </label>
        <label>
          Notes
          <textarea name="notes" maxLength={8000} />
        </label>
        <button type="submit">Create deployment record</button>
      </form>

      <h3>Deployment history</h3>
      {view.deployments.length === 0 ? <p>No deployments recorded.</p> : (
        <ul>
          {view.deployments.map((deployment) => (
            <li key={deployment.id}>
              <p>
                <strong>{serviceNames.get(deployment.service_id) ?? deployment.service_id}</strong>
                {' → '}{environmentNames.get(deployment.environment_id) ?? deployment.environment_id}
                {' — '}{deployment.status} — {deployment.provider}
              </p>
              <p>
                Source: {deployment.source_version ?? 'no version'}
                {deployment.source_commit_sha ? ` @ ${deployment.source_commit_sha.slice(0, 12)}` : ''}
                {deployment.repository_id ? ` — ${repositoryNames.get(deployment.repository_id) ?? deployment.repository_id}` : ''}
              </p>
              <p>Created {formatDate(deployment.created_at)} — started {formatDate(deployment.started_at)} — finished {formatDate(deployment.finished_at)}</p>
              {deployment.deployment_url ? <p><a href={deployment.deployment_url} target="_blank" rel="noreferrer">Open deployment</a></p> : null}
              {deployment.error_summary ? <p>Error: {deployment.error_summary}</p> : null}
              {deployment.notes ? <p>{deployment.notes}</p> : null}
              {deployment.rollback_of_deployment_id ? <p>Rollback deployment of {deployment.rollback_of_deployment_id}</p> : null}
              {deployment.status === 'QUEUED' || deployment.status === 'RUNNING' ? (
                <form action={async (formData) => { 'use server'; await updateDeploymentStatusAction(projectId, deployment.id, formData) }}>
                  <label>
                    Next status
                    <select name="status" defaultValue={deployment.status === 'QUEUED' ? 'RUNNING' : 'SUCCEEDED'}>
                      {deployment.status === 'QUEUED' ? (
                        <>
                          <option value="RUNNING">Running</option>
                          <option value="CANCELLED">Cancelled</option>
                        </>
                      ) : (
                        <>
                          <option value="SUCCEEDED">Succeeded</option>
                          <option value="FAILED">Failed</option>
                          <option value="CANCELLED">Cancelled</option>
                        </>
                      )}
                    </select>
                  </label>
                  <label>
                    Deployment URL
                    <input name="deployment_url" type="url" maxLength={2048} defaultValue={deployment.deployment_url ?? ''} />
                  </label>
                  <label>
                    Error summary (required when Failed)
                    <input name="error_summary" maxLength={1000} />
                  </label>
                  <button type="submit">Update deployment</button>
                </form>
              ) : null}
            </li>
          ))}
        </ul>
      )}

      <h3>Create release</h3>
      <form action={async (formData) => { 'use server'; await createReleaseAction(projectId, formData) }}>
        <label>
          Version
          <input name="version" required maxLength={120} placeholder="1.4.0" />
        </label>
        <label>
          Name
          <input name="name" required maxLength={200} placeholder="Argus 1.4.0" />
        </label>
        <label>
          Notes
          <textarea name="notes" maxLength={20000} />
        </label>
        <button type="submit">Create draft release</button>
      </form>

      <h3>Releases</h3>
      {view.releases.length === 0 ? <p>No releases yet.</p> : (
        view.releases.map((release) => (
          <article key={release.id}>
            <h4>{release.name} — {release.status}</h4>
            <p>Version {release.version} — released {formatDate(release.released_at)}</p>
            {release.notes ? <p>{release.notes}</p> : null}
            <ul>
              {release.components.map((component) => (
                <li key={component.id}>
                  {serviceNames.get(component.service_id) ?? component.service_id}
                  {' — '}deployment {component.deployment_id ? component.deployment_id.slice(0, 8) : 'none'}
                  {' — '}version {component.version ?? '—'}
                  {' — '}commit {component.commit_sha?.slice(0, 12) ?? '—'}
                </li>
              ))}
            </ul>
            {release.status === 'DRAFT' ? (
              <>
                <form action={async (formData) => { 'use server'; await addReleaseComponentAction(projectId, release.id, formData) }}>
                  <label>
                    Service
                    <select name="service_id" required defaultValue="">
                      <option value="" disabled>Select service</option>
                      {services.filter((service) => !release.components.some((component) => component.service_id === service.id)).map((service) => (
                        <option key={service.id} value={service.id}>{service.name}</option>
                      ))}
                    </select>
                  </label>
                  <label>
                    Successful deployment
                    <select name="deployment_id" defaultValue="">
                      <option value="">None</option>
                      {view.deployments.filter((deployment) => deployment.status === 'SUCCEEDED').map((deployment) => (
                        <option key={deployment.id} value={deployment.id}>
                          {serviceNames.get(deployment.service_id) ?? deployment.service_id} — {environmentNames.get(deployment.environment_id) ?? deployment.environment_id} — {deployment.id.slice(0, 8)}
                        </option>
                      ))}
                    </select>
                  </label>
                  <label>
                    Component version
                    <input name="version" maxLength={120} />
                  </label>
                  <label>
                    Component commit
                    <input name="commit_sha" minLength={7} maxLength={64} />
                  </label>
                  <button type="submit">Add component</button>
                </form>
                <form action={async () => { 'use server'; await updateReleaseStatusAction(projectId, release.id, 'READY') }}>
                  <button type="submit">Mark ready</button>
                </form>
                <form action={async () => { 'use server'; await updateReleaseStatusAction(projectId, release.id, 'FAILED') }}>
                  <button type="submit">Mark failed</button>
                </form>
              </>
            ) : null}
            {release.status === 'READY' ? (
              <>
                <form action={async () => { 'use server'; await updateReleaseStatusAction(projectId, release.id, 'RELEASED') }}>
                  <button type="submit">Release</button>
                </form>
                <form action={async () => { 'use server'; await updateReleaseStatusAction(projectId, release.id, 'FAILED') }}>
                  <button type="submit">Mark failed</button>
                </form>
              </>
            ) : null}
            {release.status === 'RELEASED' ? (
              <form action={async () => { 'use server'; await updateReleaseStatusAction(projectId, release.id, 'ROLLED_BACK') }}>
                <button type="submit">Mark release rolled back</button>
              </form>
            ) : null}
          </article>
        ))
      )}
    </section>
  )
}
