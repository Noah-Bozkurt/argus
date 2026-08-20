import { getServers } from '../../../lib/api'
import { getProjectComposeStacks } from '../../../lib/compose-stacks-api'
import { createComposeStackAction, deleteComposeStackAction, runComposeStackAction, updateComposeStackAction } from './compose-stack-actions'

const lifecycleStatuses = ['ACTIVE', 'PAUSED', 'ARCHIVED'] as const

export default async function ComposeStacksSection({ projectId }: { projectId: string }) {
  const [stacks, allServers] = await Promise.all([getProjectComposeStacks(projectId), getServers()])
  const servers = allServers.filter((server) => server.project_id === projectId)

  return (
    <section>
      <h2>Compose stacks</h2>
      <p>Existing Docker Compose projects registered as first-class Argus resources. Runtime controls use Docker-discovered Compose identity instead of accepting paths or YAML from the browser.</p>

      <h3>Compose stacks</h3>
      {servers.length > 0 ? (
        <details className="create-drawer">
          <summary className="button">+ Register stack</summary>
          <div className="drawer-content">
            <form action={async (formData) => { 'use server'; await createComposeStackAction(projectId, formData) }}>
              <div className="form-grid">
                <label>Display name<input name="name" required maxLength={120} placeholder="Production web stack" /></label>
                <label>Compose project name<input name="compose_project_name" required maxLength={128} placeholder="my_stack" pattern="[a-z0-9][a-z0-9_-]*" /></label>
                <label>Server<select name="server_id" required defaultValue=""><option value="" disabled>Select server</option>{servers.map((server) => <option key={server.server_id} value={server.server_id}>{server.hostname}</option>)}</select></label>
                <label className="full">Description<textarea name="description" maxLength={4000} /></label>
              </div>
              <button className="primary" type="submit">Register stack</button>
            </form>
          </div>
        </details>
      ) : <div className="callout warning">Add a managed server to this project before registering a Compose stack.</div>}

      {stacks.length === 0 ? <div className="empty-state"><strong>No Compose stacks</strong>Registered Docker Compose workloads will appear here.</div> : (
        stacks.map((stack) => {
          const archived = stack.lifecycle_status === 'ARCHIVED'
          return (
            <article key={stack.id}>
              <div className="resource-card-head">
                <div><h4>{stack.name}</h4><div className="resource-meta"><code>{stack.compose_project_name}</code> · {stack.server_hostname} · {stack.environment_name}</div></div>
                <span className={`badge ${archived ? '' : stack.lifecycle_status === 'ACTIVE' ? 'success' : 'warning'}`}>{stack.lifecycle_status}</span>
              </div>
              <div className="action-row">
                <form action={async () => { 'use server'; await runComposeStackAction(projectId, stack.id, 'start') }}><button type="submit" disabled={archived}>Start</button></form>
                <form action={async () => { 'use server'; await runComposeStackAction(projectId, stack.id, 'restart') }}><button type="submit" disabled={archived}>Restart</button></form>
                <form action={async () => { 'use server'; await runComposeStackAction(projectId, stack.id, 'stop') }}><button type="submit" disabled={archived}>Stop</button></form>
              </div>
              {archived ? <div className="callout">Archived stacks are inventory-only and cannot be operated.</div> : null}
              <details className="resource-editor">
                <summary className="button small">Edit stack</summary>
                <div className="resource-editor-body">
                  <form action={async (formData) => { 'use server'; await updateComposeStackAction(projectId, stack.id, formData) }}>
                    <div className="form-grid">
                      <label>Display name<input name="name" required maxLength={120} defaultValue={stack.name} /></label>
                      <label>Lifecycle<select name="lifecycle_status" defaultValue={stack.lifecycle_status}>{lifecycleStatuses.map((status) => <option key={status} value={status}>{status}</option>)}</select></label>
                      <label className="full">Description<textarea name="description" maxLength={4000} defaultValue={stack.description} /></label>
                    </div>
                    <button type="submit">Save stack</button>
                  </form>
                  <form action={async () => { 'use server'; await deleteComposeStackAction(projectId, stack.id) }}><button className="danger" type="submit">Unregister stack</button></form>
                  <small>Unregistering only removes the Argus record; it does not stop or delete the Docker Compose project.</small>
                </div>
              </details>
            </article>
          )
        })
      )}
    </section>
  )
}
