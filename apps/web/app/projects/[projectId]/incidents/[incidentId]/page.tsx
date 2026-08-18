import Link from 'next/link'
import { getIncident, type IncidentStatus } from '../../../../../lib/incidents-api'
import { addIncidentNoteAction, updateIncidentStatusAction } from '../../incident-actions'
import IncidentCorrelationSection from './correlation-section'

function nextStatuses(status: IncidentStatus): IncidentStatus[] {
  switch (status) {
    case 'INVESTIGATING':
      return ['IDENTIFIED', 'MONITORING']
    case 'IDENTIFIED':
      return ['INVESTIGATING', 'MONITORING']
    case 'MONITORING':
      return ['INVESTIGATING', 'RESOLVED']
    case 'RESOLVED':
      return []
  }
}

export default async function IncidentPage({
  params,
}: {
  params: { projectId: string; incidentId: string }
}) {
  const detail = await getIncident(params.projectId, params.incidentId)
  const { incident, affected, timeline } = detail

  return (
    <main>
      <p><Link href={`/projects/${params.projectId}`}>← Project</Link></p>
      <h1>{incident.title}</h1>
      <p>{incident.severity} — {incident.status}</p>
      <p>Started {new Date(incident.started_at).toLocaleString()}</p>
      {incident.resolved_at ? <p>Resolved {new Date(incident.resolved_at).toLocaleString()}</p> : null}
      <p>Root: {incident.source_type}: {incident.source_name}</p>
      {incident.summary ? <p>{incident.summary}</p> : null}

      <h2>Status</h2>
      {nextStatuses(incident.status).length === 0 ? <p>This incident is resolved.</p> : (
        <div>
          {nextStatuses(incident.status).map((status) => (
            <form key={status} action={async () => {
              'use server'
              await updateIncidentStatusAction(params.projectId, incident.id, status)
            }}>
              <button type="submit">Move to {status}</button>
            </form>
          ))}
        </div>
      )}

      <h2>Blast radius snapshot</h2>
      <p>{affected.length} dependent resource(s) were captured when this incident was created.</p>
      {affected.length === 0 ? <p>No dependent resources were in the graph snapshot.</p> : (
        <ol>
          {affected.map((resource) => (
            <li key={resource.id}>
              <p>
                <strong>{resource.resource_type}: {resource.resource_name}</strong>
                {' — '}distance {resource.distance}
              </p>
              <p>
                {resource.impact_path
                  .map((pathItem) => `${pathItem.resource_type}: ${pathItem.name}`)
                  .join(' → ')}
              </p>
            </li>
          ))}
        </ol>
      )}

      <IncidentCorrelationSection projectId={params.projectId} incidentId={incident.id} />

      <h2>Add timeline note</h2>
      <form action={async (formData) => {
        'use server'
        await addIncidentNoteAction(params.projectId, incident.id, formData)
      }}>
        <textarea name="message" required maxLength={12000} placeholder="Investigation update, mitigation, observation..." />
        <button type="submit">Add note</button>
      </form>

      <h2>Timeline</h2>
      <ol>
        {timeline.map((event) => (
          <li key={event.id}>
            <p><strong>{event.event_type}</strong> — {new Date(event.created_at).toLocaleString()}</p>
            <p>{event.message}</p>
          </li>
        ))}
      </ol>
    </main>
  )
}
