import { getIncidentCorrelation } from '../../../../../lib/change-correlation-api'

function relativeTime(minutes: number): string {
  if (minutes === 0) return 'at incident start'
  if (minutes < 0) return `${Math.abs(minutes)} min before`
  return `${minutes} min after`
}

export default async function IncidentCorrelationSection({
  projectId,
  incidentId,
}: {
  projectId: string
  incidentId: string
}) {
  const correlation = await getIncidentCorrelation(projectId, incidentId)

  return (
    <section>
      <h2>Change correlation</h2>
      <p>
        Changes within ±{correlation.window_minutes} minutes of incident start. These are investigation signals, not proof of root cause.
      </p>
      {correlation.changes.length === 0 ? <p>No nearby tracked changes found.</p> : (
        <ol>
          {correlation.changes.map((change, index) => (
            <li key={`${change.category}-${change.occurred_at}-${index}`}>
              <p>
                <strong>{change.impact_related ? 'Impact-related' : 'Nearby'} {change.category}</strong>
                {' — '}{relativeTime(change.minutes_from_incident)}
                {' — '}{new Date(change.occurred_at).toLocaleString()}
              </p>
              <p>{change.summary}</p>
              <p>Event: {change.event_type}</p>
            </li>
          ))}
        </ol>
      )}
    </section>
  )
}
