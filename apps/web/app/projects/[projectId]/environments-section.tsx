import { getProjectEnvironments } from '../../../lib/api'
import {
  createEnvironmentAction,
  deleteEnvironmentAction,
  updateEnvironmentAction,
} from './environment-actions'

const environmentTypes = ['development', 'preview', 'staging', 'production', 'custom'] as const

export default async function EnvironmentsSection({ projectId }: { projectId: string }) {
  const environments = await getProjectEnvironments(projectId)

  return (
    <section>
      <h2>Environments</h2>
      <p>
        Environments are project-owned deployment contexts. Production is always protected from deletion.
      </p>

      <h3>Add environment</h3>
      <form action={async (formData) => { 'use server'; await createEnvironmentAction(projectId, formData) }}>
        <label>
          Name
          <input name="name" required maxLength={120} placeholder="Staging" />
        </label>
        <label>
          Type
          <select name="environment_type" defaultValue="development">
            {environmentTypes.map((type) => <option key={type} value={type}>{type}</option>)}
          </select>
        </label>
        <label>
          Description
          <textarea name="description" maxLength={4000} />
        </label>
        <label>
          <input name="is_protected" type="checkbox" /> Protected
        </label>
        <button type="submit">Add environment</button>
      </form>

      <h3>Project environments</h3>
      {environments.length === 0 ? <p>No environments yet.</p> : (
        environments.map((environment) => (
          <article key={environment.id}>
            <h4>{environment.name}</h4>
            <p>
              {environment.environment_type} — {environment.is_protected ? 'PROTECTED' : 'unprotected'} — {environment.server_count} server(s) — {environment.service_count} service(s)
            </p>
            <form action={async (formData) => { 'use server'; await updateEnvironmentAction(projectId, environment.id, formData) }}>
              <label>
                Name
                <input name="name" required maxLength={120} defaultValue={environment.name} />
              </label>
              <label>
                Type
                <select name="environment_type" defaultValue={environment.environment_type}>
                  {environmentTypes.map((type) => <option key={type} value={type}>{type}</option>)}
                </select>
              </label>
              <label>
                Description
                <textarea name="description" maxLength={4000} defaultValue={environment.description} />
              </label>
              <label>
                <input name="is_protected" type="checkbox" defaultChecked={environment.is_protected} /> Protected
              </label>
              <button type="submit">Save environment</button>
            </form>
            <form action={async () => { 'use server'; await deleteEnvironmentAction(projectId, environment.id) }}>
              <button type="submit" disabled={environment.is_protected || environment.server_count > 0 || environment.service_count > 0}>
                Delete environment
              </button>
            </form>
          </article>
        ))
      )}
    </section>
  )
}
