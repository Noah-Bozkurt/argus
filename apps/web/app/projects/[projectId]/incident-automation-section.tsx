import Link from 'next/link'
import { getSiteIncidentPolicies } from '../../../lib/incident-automation-api'
import { saveSiteIncidentPolicyAction } from './incident-automation-actions'

function severityClass(severity: string): string {
  if (severity === 'CRITICAL') return 'danger'
  if (severity === 'MAJOR') return 'warning'
  return 'info'
}

export default async function IncidentAutomationSection({ projectId }: { projectId: string }) {
  const policies = await getSiteIncidentPolicies(projectId)

  return (
    <section>
      <h2>Incident automation</h2>
      <p>Create one internal incident after repeated DOWN/ERROR site checks. DEGRADED checks do not count, incidents are never auto-published and resolution remains manual.</p>

      <h3>Policies</h3>
      {policies.length === 0 ? <div className="empty-state"><strong>No active sites</strong>Incident automation becomes available after site monitoring is configured.</div> : (
        <div className="resource-list" style={{ margin: '0 17px 17px' }}>
          {policies.map((policy) => (
            <article className="resource-card" key={policy.site_id}>
              <div className="resource-card-head">
                <div><h4>{policy.site_name}</h4><div className="resource-meta">{policy.has_monitor_config ? `Trigger after ${policy.failure_threshold} consecutive DOWN/ERROR checks` : 'Configure site monitoring first'}</div></div>
                <div className="action-row"><span className={`badge ${policy.enabled ? 'success' : ''}`}>{policy.enabled ? 'Enabled' : 'Disabled'}</span><span className={`badge ${severityClass(policy.severity)}`}>{policy.severity}</span></div>
              </div>
              {policy.active_incident_id ? <div className="callout warning" style={{ marginTop: 12 }}>Active linked incident: <Link className="panel-link" href={`/projects/${projectId}/incidents/${policy.active_incident_id}`}>{policy.active_incident_status ?? 'Open incident'} →</Link></div> : null}
              {policy.has_monitor_config ? (
                <details className="resource-editor">
                  <summary className="button small">Configure policy</summary>
                  <div className="resource-editor-body">
                    <form action={async (formData) => { 'use server'; await saveSiteIncidentPolicyAction(projectId, policy.site_id, formData) }}>
                      <div className="form-grid">
                        <label style={{ display: 'flex', gridTemplateColumns: 'auto 1fr', alignItems: 'center' }}><input name="enabled" type="checkbox" defaultChecked={policy.enabled} /> Enable automatic incident creation</label>
                        <label>Consecutive failures<input name="failure_threshold" type="number" min={2} max={10} defaultValue={policy.failure_threshold} required /></label>
                        <label>Incident severity<select name="severity" defaultValue={policy.severity}><option value="MINOR">Minor</option><option value="MAJOR">Major</option><option value="CRITICAL">Critical</option></select></label>
                      </div>
                      <button type="submit">Save incident policy</button>
                    </form>
                  </div>
                </details>
              ) : null}
            </article>
          ))}
        </div>
      )}
    </section>
  )
}
