import { getDeploymentReleaseView } from '../../../lib/deployments-releases-api'
import { getProjectEnvironments, getProjectRepositories, getProjectServices } from '../../../lib/api'
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

function statusClass(status: string): string {
  if (status === 'SUCCEEDED' || status === 'RELEASED') return 'success'
  if (status === 'FAILED' || status === 'ROLLED_BACK' || status === 'CANCELLED') return 'danger'
  if (status === 'RUNNING' || status === 'READY') return 'info'
  if (status === 'QUEUED' || status === 'DRAFT') return 'warning'
  return ''
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
      <h2>Deployments &amp; releases</h2>
      <p>Argus records immutable deployment and release history. Provider execution is intentionally separate from these lifecycle records.</p>

      <h3>Deployments</h3>
      <details className="create-drawer">
        <summary className="button">+ Record deployment</summary>
        <div className="drawer-content">
          <form action={async (formData) => { 'use server'; await createDeploymentAction(projectId, formData) }}>
            <div className="form-grid">
              <label>Service<select name="service_id" required defaultValue=""><option value="" disabled>Select service</option>{services.map((service) => <option key={service.id} value={service.id}>{service.name}</option>)}</select></label>
              <label>Environment<select name="environment_id" required defaultValue=""><option value="" disabled>Select environment</option>{environments.map((environment) => <option key={environment.id} value={environment.id}>{environment.name}</option>)}</select></label>
              <label>Repository<select name="repository_id" defaultValue=""><option value="">Use service repository / none</option>{repositories.map((repository) => <option key={repository.id} value={repository.id}>{repository.owner}/{repository.name}</option>)}</select></label>
              <label>Commit SHA<input name="source_commit_sha" minLength={7} maxLength={64} placeholder="abcdef123456" /></label>
              <label>Version<input name="source_version" maxLength={120} placeholder="1.4.0" /></label>
              <label>Rollback of<select name="rollback_of_deployment_id" defaultValue=""><option value="">Normal deployment</option>{view.deployments.filter((deployment) => deployment.status === 'SUCCEEDED').map((deployment) => <option key={deployment.id} value={deployment.id}>{serviceNames.get(deployment.service_id) ?? deployment.service_id} → {environmentNames.get(deployment.environment_id) ?? deployment.environment_id} — {deployment.id.slice(0, 8)}</option>)}</select></label>
              <label className="full">Notes<textarea name="notes" maxLength={8000} /></label>
            </div>
            <button className="primary" type="submit">Create deployment record</button>
          </form>
        </div>
      </details>

      {view.deployments.length === 0 ? <div className="empty-state"><strong>No deployments</strong>Deployment attempts will appear here.</div> : (
        <div className="resource-list" style={{ margin: '0 17px 17px' }}>
          {view.deployments.map((deployment) => (
            <article className="resource-card" key={deployment.id}>
              <div className="resource-card-head">
                <div><h4>{serviceNames.get(deployment.service_id) ?? deployment.service_id} → {environmentNames.get(deployment.environment_id) ?? deployment.environment_id}</h4><div className="resource-meta">{deployment.source_version ?? 'No version'}{deployment.source_commit_sha ? ` @ ${deployment.source_commit_sha.slice(0, 12)}` : ''}{deployment.repository_id ? ` · ${repositoryNames.get(deployment.repository_id) ?? deployment.repository_id}` : ''}</div></div>
                <span className={`badge ${statusClass(deployment.status)}`}>{deployment.status}</span>
              </div>
              <div className="info-grid" style={{ marginTop: 12 }}>
                <div className="info-item"><span className="info-label">Provider</span><span className="info-value">{deployment.provider}</span></div>
                <div className="info-item"><span className="info-label">Created</span><span className="info-value">{formatDate(deployment.created_at)}</span></div>
                <div className="info-item"><span className="info-label">Started</span><span className="info-value">{formatDate(deployment.started_at)}</span></div>
                <div className="info-item"><span className="info-label">Finished</span><span className="info-value">{formatDate(deployment.finished_at)}</span></div>
              </div>
              {deployment.error_summary ? <div className="callout danger">{deployment.error_summary}</div> : null}
              {deployment.notes ? <div className="resource-meta">{deployment.notes}</div> : null}
              {deployment.rollback_of_deployment_id ? <div className="badge warning">Rollback of {deployment.rollback_of_deployment_id.slice(0, 8)}</div> : null}
              <div className="action-row">
                {deployment.deployment_url ? <a className="button small" href={deployment.deployment_url} target="_blank" rel="noreferrer">Open deployment ↗</a> : null}
                {deployment.status === 'QUEUED' || deployment.status === 'RUNNING' ? (
                  <details className="resource-editor">
                    <summary className="button small">Update status</summary>
                    <div className="resource-editor-body">
                      <form action={async (formData) => { 'use server'; await updateDeploymentStatusAction(projectId, deployment.id, formData) }}>
                        <div className="form-grid">
                          <label>Next status<select name="status" defaultValue={deployment.status === 'QUEUED' ? 'RUNNING' : 'SUCCEEDED'}>{deployment.status === 'QUEUED' ? <><option value="RUNNING">Running</option><option value="CANCELLED">Cancelled</option></> : <><option value="SUCCEEDED">Succeeded</option><option value="FAILED">Failed</option><option value="CANCELLED">Cancelled</option></>}</select></label>
                          <label>Deployment URL<input name="deployment_url" type="url" maxLength={2048} defaultValue={deployment.deployment_url ?? ''} /></label>
                          <label className="full">Error summary (required when failed)<input name="error_summary" maxLength={1000} /></label>
                        </div>
                        <button type="submit">Update deployment</button>
                      </form>
                    </div>
                  </details>
                ) : null}
              </div>
            </article>
          ))}
        </div>
      )}

      <h3>Releases</h3>
      <details className="create-drawer">
        <summary className="button">+ Create release</summary>
        <div className="drawer-content">
          <form action={async (formData) => { 'use server'; await createReleaseAction(projectId, formData) }}>
            <div className="form-grid">
              <label>Version<input name="version" required maxLength={120} placeholder="1.4.0" /></label>
              <label>Name<input name="name" required maxLength={200} placeholder="Argus 1.4.0" /></label>
              <label className="full">Notes<textarea name="notes" maxLength={20000} /></label>
            </div>
            <button className="primary" type="submit">Create draft release</button>
          </form>
        </div>
      </details>

      {view.releases.length === 0 ? <div className="empty-state"><strong>No releases</strong>Release records will appear here.</div> : (
        <div className="resource-list" style={{ margin: '0 17px 17px' }}>
          {view.releases.map((release) => (
            <article className="resource-card" key={release.id}>
              <div className="resource-card-head"><div><h4>{release.name}</h4><div className="resource-meta">Version {release.version} · released {formatDate(release.released_at)}</div></div><span className={`badge ${statusClass(release.status)}`}>{release.status}</span></div>
              {release.notes ? <div className="resource-meta">{release.notes}</div> : null}
              {release.components.length > 0 ? <div className="resource-list" style={{ marginTop: 12 }}>{release.components.map((component) => <div className="resource-card" key={component.id}><div className="resource-card-head"><strong>{serviceNames.get(component.service_id) ?? component.service_id}</strong><span className="badge">{component.version ?? 'No version'}</span></div><div className="resource-meta">Deployment {component.deployment_id ? component.deployment_id.slice(0, 8) : 'none'} · commit {component.commit_sha?.slice(0, 12) ?? '—'}</div></div>)}</div> : null}

              {release.status === 'DRAFT' ? (
                <details className="resource-editor">
                  <summary className="button small">Configure draft</summary>
                  <div className="resource-editor-body">
                    <form action={async (formData) => { 'use server'; await addReleaseComponentAction(projectId, release.id, formData) }}>
                      <strong>Add component</strong>
                      <div className="form-grid">
                        <label>Service<select name="service_id" required defaultValue=""><option value="" disabled>Select service</option>{services.filter((service) => !release.components.some((component) => component.service_id === service.id)).map((service) => <option key={service.id} value={service.id}>{service.name}</option>)}</select></label>
                        <label>Successful deployment<select name="deployment_id" defaultValue=""><option value="">None</option>{view.deployments.filter((deployment) => deployment.status === 'SUCCEEDED').map((deployment) => <option key={deployment.id} value={deployment.id}>{serviceNames.get(deployment.service_id) ?? deployment.service_id} — {environmentNames.get(deployment.environment_id) ?? deployment.environment_id} — {deployment.id.slice(0, 8)}</option>)}</select></label>
                        <label>Component version<input name="version" maxLength={120} /></label>
                        <label>Component commit<input name="commit_sha" minLength={7} maxLength={64} /></label>
                      </div>
                      <button type="submit">Add component</button>
                    </form>
                    <div className="action-row"><form action={async () => { 'use server'; await updateReleaseStatusAction(projectId, release.id, 'READY') }}><button type="submit">Mark ready</button></form><form action={async () => { 'use server'; await updateReleaseStatusAction(projectId, release.id, 'FAILED') }}><button className="danger" type="submit">Mark failed</button></form></div>
                  </div>
                </details>
              ) : null}
              {release.status === 'READY' ? <div className="action-row"><form action={async () => { 'use server'; await updateReleaseStatusAction(projectId, release.id, 'RELEASED') }}><button className="primary" type="submit">Release</button></form><form action={async () => { 'use server'; await updateReleaseStatusAction(projectId, release.id, 'FAILED') }}><button className="danger" type="submit">Mark failed</button></form></div> : null}
              {release.status === 'RELEASED' ? <div className="action-row"><form action={async () => { 'use server'; await updateReleaseStatusAction(projectId, release.id, 'ROLLED_BACK') }}><button className="danger" type="submit">Mark rolled back</button></form></div> : null}
            </article>
          ))}
        </div>
      )}
    </section>
  )
}
