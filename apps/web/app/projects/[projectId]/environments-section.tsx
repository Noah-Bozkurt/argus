import { getProjectEnvironments } from '../../../lib/api'
import { createEnvironmentAction, deleteEnvironmentAction, updateEnvironmentAction } from './environment-actions'

const environmentTypes = ['development', 'preview', 'staging', 'production', 'custom'] as const

function environmentBadge(type: string): { className: string; label: string } {
  if (type === 'production') return { className: 'danger', label: 'PROD' }
  if (type === 'staging') return { className: 'warning', label: 'STAGE' }
  if (type === 'preview') return { className: 'info', label: 'PREVIEW' }
  if (type === 'development') return { className: 'info', label: 'DEV' }
  return { className: '', label: 'CUSTOM' }
}

export default async function EnvironmentsSection({ projectId }: { projectId: string }) {
  const environments = await getProjectEnvironments(projectId)

  return (
    <section>
      <h2>Environments</h2>
      <p>Project-owned deployment contexts. Protected environments and environments with attached resources cannot be deleted.</p>

      <h3>Environments</h3>
      <details className="create-drawer">
        <summary className="button">+ Add environment</summary>
        <div className="drawer-content">
          <form action={async (formData) => { 'use server'; await createEnvironmentAction(projectId, formData) }}>
            <div className="form-grid">
              <label>Name<input name="name" required maxLength={120} placeholder="Staging" /></label>
              <label>Type<select name="environment_type" defaultValue="development">{environmentTypes.map((type) => <option key={type} value={type}>{type}</option>)}</select></label>
              <label className="full">Description<textarea name="description" maxLength={4000} /></label>
              <label style={{ display: 'flex', gridTemplateColumns: 'auto 1fr', alignItems: 'center' }}><input name="is_protected" type="checkbox" /> Protected</label>
            </div>
            <button className="primary" type="submit">Add environment</button>
          </form>
        </div>
      </details>

      {environments.length === 0 ? <div className="empty-state"><strong>No environments yet</strong>Add a deployment context when this project needs one.</div> : (
        environments.map((environment) => {
          const identity = environmentBadge(environment.environment_type)
          return (
            <article key={environment.id}>
              <div className="resource-card-head">
                <div><h4>{environment.name}</h4><div className="resource-meta">{environment.description || 'No description'}</div></div>
                <div className="action-row"><span className={`badge ${identity.className}`} title={environment.environment_type}>{identity.label}</span>{environment.is_protected ? <span className="badge warning">Protected</span> : null}</div>
              </div>
              <div className="detail-hero-meta"><span className="badge">{environment.server_count} servers</span><span className="badge">{environment.service_count} services</span>{environment.environment_type === 'production' ? <span className="badge danger">Production safety</span> : null}</div>
              <details className="resource-editor">
                <summary className="button small">Edit environment</summary>
                <div className="resource-editor-body">
                  <form action={async (formData) => { 'use server'; await updateEnvironmentAction(projectId, environment.id, formData) }}>
                    <div className="form-grid">
                      <label>Name<input name="name" required maxLength={120} defaultValue={environment.name} /></label>
                      <label>Type<select name="environment_type" defaultValue={environment.environment_type}>{environmentTypes.map((type) => <option key={type} value={type}>{type}</option>)}</select></label>
                      <label className="full">Description<textarea name="description" maxLength={4000} defaultValue={environment.description} /></label>
                      <label style={{ display: 'flex', gridTemplateColumns: 'auto 1fr', alignItems: 'center' }}><input name="is_protected" type="checkbox" defaultChecked={environment.is_protected} /> Protected</label>
                    </div>
                    <button type="submit">Save environment</button>
                  </form>
                  <div className="resource-inline-actions">
                    <form action={async (formData) => { 'use server'; await createEnvironmentAction(projectId, formData) }}>
                      <input type="hidden" name="name" value={`${environment.name} copy`} />
                      <input type="hidden" name="environment_type" value={environment.environment_type} />
                      <input type="hidden" name="description" value={environment.description} />
                      {environment.is_protected ? <input type="hidden" name="is_protected" value="on" /> : null}
                      <button type="submit">Duplicate config</button>
                    </form>
                    <form action={async () => { 'use server'; await deleteEnvironmentAction(projectId, environment.id) }}>
                      <button className="danger" type="submit" disabled={environment.is_protected || environment.server_count > 0 || environment.service_count > 0}>Delete environment</button>
                    </form>
                  </div>
                </div>
              </details>
            </article>
          )
        })
      )}
    </section>
  )
}
