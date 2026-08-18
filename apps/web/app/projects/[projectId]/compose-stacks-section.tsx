import { getServers } from '../../../lib/api'
import { getProjectComposeStacks } from '../../../lib/compose-stacks-api'
import {
  createComposeStackAction,
  deleteComposeStackAction,
  runComposeStackAction,
  updateComposeStackAction,
} from './compose-stack-actions'

const lifecycleStatuses = ['ACTIVE', 'PAUSED', 'ARCHIVED'] as const

export default async function ComposeStacksSection({ projectId }: { projectId: string }) {
  const [stacks, allServers] = await Promise.all([
    getProjectComposeStacks(projectId),
    getServers(),
  ])
  const servers = allServers.filter((server) => server.project_id === projectId)

  return (
    <section>
      <h2>Compose stacks</h2>
      <p>
        Register existing Docker Compose projects as first-class Argus resources. Runtime controls use the stored Compose project identity; the privileged helper discovers its config files from Docker instead of accepting paths or YAML from the browser.
      </p>

      <h3>Register existing stack</h3>
      {servers.length === 0 ? (
        <p>Add a managed server to this project before registering a stack.</p>
      ) : (
        <form action={async (formData) => { 'use server'; await createComposeStackAction(projectId, formData) }}>
          <label>
            Display name
            <input name="name" required maxLength={120} placeholder="Production web stack" />
          </label>
          <label>
            Compose project name
            <input name="compose_project_name" required maxLength={128} placeholder="my_stack" pattern="[a-z0-9][a-z0-9_-]*" />
          </label>
          <label>
            Server
            <select name="server_id" required defaultValue="">
              <option value="" disabled>Select server</option>
              {servers.map((server) => (
                <option key={server.server_id} value={server.server_id}>{server.hostname}</option>
              ))}
            </select>
          </label>
          <label>
            Description
            <textarea name="description" maxLength={4000} />
          </label>
          <button type="submit">Register stack</button>
        </form>
      )}

      <h3>Registered stacks</h3>
      {stacks.length === 0 ? <p>No Compose stacks registered.</p> : (
        stacks.map((stack) => {
          const archived = stack.lifecycle_status === 'ARCHIVED'
          return (
            <article key={stack.id}>
              <h4>{stack.name}</h4>
              <p>
                <code>{stack.compose_project_name}</code> — {stack.server_hostname} — {stack.environment_name} — {stack.lifecycle_status}
              </p>
              <div>
                <form action={async () => { 'use server'; await runComposeStackAction(projectId, stack.id, 'start') }}>
                  <button type="submit" disabled={archived}>Start stack</button>
                </form>
                <form action={async () => { 'use server'; await runComposeStackAction(projectId, stack.id, 'restart') }}>
                  <button type="submit" disabled={archived}>Restart stack</button>
                </form>
                <form action={async () => { 'use server'; await runComposeStackAction(projectId, stack.id, 'stop') }}>
                  <button type="submit" disabled={archived}>Stop stack</button>
                </form>
              </div>
              {archived ? <p>Archived stacks are inventory-only and cannot be operated.</p> : null}
              <form action={async (formData) => { 'use server'; await updateComposeStackAction(projectId, stack.id, formData) }}>
                <label>
                  Display name
                  <input name="name" required maxLength={120} defaultValue={stack.name} />
                </label>
                <label>
                  Description
                  <textarea name="description" maxLength={4000} defaultValue={stack.description} />
                </label>
                <label>
                  Lifecycle
                  <select name="lifecycle_status" defaultValue={stack.lifecycle_status}>
                    {lifecycleStatuses.map((status) => <option key={status} value={status}>{status}</option>)}
                  </select>
                </label>
                <button type="submit">Save stack</button>
              </form>
              <form action={async () => { 'use server'; await deleteComposeStackAction(projectId, stack.id) }}>
                <button type="submit">Unregister stack</button>
              </form>
              <small>Unregistering only removes the Argus record; it does not stop or delete the Docker Compose project.</small>
            </article>
          )
        })
      )}
    </section>
  )
}
