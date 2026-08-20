import { getProjectReadiness } from '../../../lib/readiness-api'

const statusOrder = ['BLOCKED', 'WARN', 'PASS', 'SKIPPED'] as const

function statusClass(status: string): string {
  if (status === 'PASS') return 'success'
  if (status === 'BLOCKED') return 'danger'
  if (status === 'WARN') return 'warning'
  return ''
}

export default async function ReadinessSection({ projectId }: { projectId: string }) {
  const assessment = await getProjectReadiness(projectId)
  const checks = [...assessment.checks].sort((left, right) => {
    const leftIndex = statusOrder.indexOf(left.status)
    const rightIndex = statusOrder.indexOf(right.status)
    return leftIndex - rightIndex || left.category.localeCompare(right.category) || left.label.localeCompare(right.label)
  })
  const counts = statusOrder.map((status) => ({ status, count: checks.filter((check) => check.status === status).length }))

  return (
    <section>
      <h2>Release / launch readiness</h2>
      <p>Read-only assessment of current Argus signals. This view never deploys, restarts, updates or otherwise mutates resources.</p>

      <h3>Readiness assessment</h3>
      <div className="info-grid" style={{ margin: '0 17px 14px' }}>
        <div className="info-item"><span className="info-label">Overall</span><span className="info-value"><span className={`badge ${statusClass(assessment.status)}`}>{assessment.status}</span></span></div>
        <div className="info-item"><span className="info-label">Checked</span><span className="info-value">{new Date(assessment.checked_at).toLocaleString()}</span></div>
      </div>
      <div className="status-grid" style={{ margin: '0 17px 14px' }}>
        {counts.map(({ status, count }) => <div className="status-item" key={status}><span className="info-label">{status}</span><span className="info-value">{count}</span></div>)}
      </div>

      {checks.length === 0 ? <div className="empty-state"><strong>No readiness checks</strong>Checks will appear when project signals are available.</div> : (
        <div className="resource-list" style={{ margin: '0 17px 17px' }}>
          {checks.map((check) => (
            <article className="resource-card" key={check.key}>
              <div className="resource-card-head"><div><h4>{check.label}</h4><div className="resource-meta">{check.category} · {check.summary}</div></div><span className={`badge ${statusClass(check.status)}`}>{check.status}</span></div>
              {check.evidence.length > 0 ? <details className="log-details" style={{ marginTop: 10 }}><summary>Evidence · {check.evidence.length}</summary><div style={{ padding: 12 }}><ul className="chip-list">{check.evidence.map((item, index) => <li className="chip" key={`${check.key}-${index}`}>{item}</li>)}</ul></div></details> : null}
            </article>
          ))}
        </div>
      )}
    </section>
  )
}
