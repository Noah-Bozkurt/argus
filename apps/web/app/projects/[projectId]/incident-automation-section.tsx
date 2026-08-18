import Link from 'next/link'
import { getSiteIncidentPolicies } from '../../../lib/incident-automation-api'
import { saveSiteIncidentPolicyAction } from './incident-automation-actions'

export default async function IncidentAutomationSection({ projectId }: { projectId: string }) {
  const policies = await getSiteIncidentPolicies(projectId)

  return (
    <section>
      <h2>Incident automation</h2>
      <p>
        Optionally create one internal Incident after repeated DOWN/ERROR Site Monitoring checks.
        DEGRADED checks do not count, Incidents are never auto-published, and resolution remains manual.
      </p>
      {policies.length === 0 ? <p>No active Sites.</p> : (
        <ul>
          {policies.map((policy) => (
            <li key={policy.site_id}>
              <p>
                <strong>{policy.site_name}</strong>
                {' — '}{policy.has_monitor_config ? 'monitor configured' : 'configure monitoring first'}
                {' — '}{policy.enabled ? `enabled after ${policy.failure_threshold} failures` : 'disabled'}
                {' — '}severity {policy.severity}
              </p>
              {policy.active_incident_id ? (
                <p>
                  Active link: <Link href={`/projects/${projectId}/incidents/${policy.active_incident_id}`}>
                    {policy.active_incident_status ?? 'Incident'}
                  </Link>
                </p>
              ) : null}
              {policy.has_monitor_config ? (
                <form
                  action={async (formData) => {
                    'use server'
                    await saveSiteIncidentPolicyAction(projectId, policy.site_id, formData)
                  }}
                >
                  <label>
                    <input name="enabled" type="checkbox" defaultChecked={policy.enabled} />
                    Enable automatic internal Incident creation
                  </label>
                  <label>
                    Consecutive DOWN/ERROR checks
                    <input
                      name="failure_threshold"
                      type="number"
                      min={2}
                      max={10}
                      defaultValue={policy.failure_threshold}
                      required
                    />
                  </label>
                  <label>
                    Incident severity
                    <select name="severity" defaultValue={policy.severity}>
                      <option value="MINOR">Minor</option>
                      <option value="MAJOR">Major</option>
                      <option value="CRITICAL">Critical</option>
                    </select>
                  </label>
                  <button type="submit">Save Incident policy</button>
                </form>
              ) : null}
            </li>
          ))}
        </ul>
      )}
    </section>
  )
}
