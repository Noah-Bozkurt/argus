import { getProjectReadiness } from '../../../lib/readiness-api'

const statusOrder = ['BLOCKED', 'WARN', 'PASS', 'SKIPPED'] as const

export default async function ReadinessSection({ projectId }: { projectId: string }) {
  const assessment = await getProjectReadiness(projectId)
  const checks = [...assessment.checks].sort((left, right) => {
    const leftIndex = statusOrder.indexOf(left.status)
    const rightIndex = statusOrder.indexOf(right.status)
    return leftIndex - rightIndex || left.category.localeCompare(right.category) || left.label.localeCompare(right.label)
  })

  return (
    <section>
      <h2>Release / Launch Readiness — {assessment.status}</h2>
      <p>
        Read-only assessment of current Argus signals. This view does not deploy, update, restart, back up or otherwise change project resources.
      </p>
      <p>Checked: {new Date(assessment.checked_at).toLocaleString()}</p>

      {checks.map((check) => (
        <article key={check.key}>
          <h3>{check.status} — {check.label}</h3>
          <p>{check.category} — {check.summary}</p>
          {check.evidence.length > 0 ? (
            <details>
              <summary>Evidence</summary>
              <ul>
                {check.evidence.map((item, index) => <li key={`${check.key}-${index}`}>{item}</li>)}
              </ul>
            </details>
          ) : null}
        </article>
      ))}
    </section>
  )
}
