import Link from 'next/link'
import { getIncident, type IncidentStatus } from '../../../../../lib/incidents-api'
import { addIncidentNoteAction, updateIncidentStatusAction } from '../../incident-actions'
import IncidentCorrelationSection from './correlation-section'

function nextStatuses(status: IncidentStatus): IncidentStatus[] {
  switch (status) {
    case 'INVESTIGATING': return ['IDENTIFIED', 'MONITORING']
    case 'IDENTIFIED': return ['INVESTIGATING', 'MONITORING']
    case 'MONITORING': return ['INVESTIGATING', 'RESOLVED']
    case 'RESOLVED': return []
  }
}

function severityClass(severity: string): string {
  if (severity === 'CRITICAL') return 'danger'
  if (severity === 'WARNING') return 'warning'
  return 'info'
}

export default async function IncidentPage({ params }: { params: { projectId: string; incidentId: string } }) {
  const detail = await getIncident(params.projectId, params.incidentId)
  const { incident, affected, timeline } = detail
  const transitions = nextStatuses(incident.status)

  return (
    <main className="detail-page">
      <div className="page-header">
        <div>
          <Link className="panel-link" href={`/projects/${params.projectId}`}>← Project</Link>
          <div style={{ marginTop: 10 }}><span className="eyebrow">Incident</span><h1>{incident.title}</h1></div>
          <div className="detail-hero-meta">
            <span className={`badge ${severityClass(incident.severity)}`}>{incident.severity}</span>
            <span className={`badge ${incident.status === 'RESOLVED' ? 'success' : 'warning'}`}>{incident.status}</span>
            <span className="badge">{incident.source_type}: {incident.source_name}</span>
          </div>
        </div>
      </div>

      <div className="detail-split">
        <section className="detail-card">
          <div className="detail-card-header"><div><h2>Incident status</h2><p>Current lifecycle and operator transitions.</p></div></div>
          <div className="detail-card-body">
            <div className="info-grid">
              <div className="info-item"><span className="info-label">Started</span><span className="info-value">{new Date(incident.started_at).toLocaleString()}</span></div>
              <div className="info-item"><span className="info-label">Resolved</span><span className="info-value">{incident.resolved_at ? new Date(incident.resolved_at).toLocaleString() : 'Open'}</span></div>
            </div>
            {incident.summary ? <div className="callout">{incident.summary}</div> : null}
            {transitions.length === 0 ? <div className="callout success">This incident is resolved.</div> : (
              <div className="action-row">{transitions.map((status) => <form key={status} action={async () => { 'use server'; await updateIncidentStatusAction(params.projectId, incident.id, status) }}><button type="submit">Move to {status.toLowerCase()}</button></form>)}</div>
            )}
          </div>
        </section>

        <section className="detail-card">
          <div className="detail-card-header"><div><h2>Blast radius</h2><p>Dependency snapshot captured when the incident was created.</p></div><span className="badge info">{affected.length} affected</span></div>
          <div className="detail-card-body">
            {affected.length === 0 ? <div className="empty-state"><strong>No dependent resources</strong>The dependency graph snapshot had no downstream resources.</div> : (
              <ol className="path-list">{affected.map((resource) => <li className="path-card" key={resource.id}><div className="resource-card-head"><strong>{resource.resource_type}: {resource.resource_name}</strong><span className="badge">Distance {resource.distance}</span></div><div className="path-line">{resource.impact_path.map((pathItem) => `${pathItem.resource_type}: ${pathItem.name}`).join(' → ')}</div></li>)}</ol>
            )}
          </div>
        </section>
      </div>

      <IncidentCorrelationSection projectId={params.projectId} incidentId={incident.id} />

      <section className="detail-card">
        <div className="detail-card-header"><div><h2>Timeline</h2><p>Investigation notes and lifecycle events in chronological context.</p></div></div>
        <div className="detail-card-body">
          <form action={async (formData) => { 'use server'; await addIncidentNoteAction(params.projectId, incident.id, formData) }}>
            <label>New timeline note<textarea name="message" required maxLength={12000} placeholder="Investigation update, mitigation, observation..." /></label>
            <button className="primary" type="submit">Add note</button>
          </form>
          {timeline.length === 0 ? <div className="empty-state"><strong>No timeline events</strong>Incident updates will appear here.</div> : (
            <ol className="timeline">{timeline.map((event) => <li className="timeline-item" key={event.id}><div className="timeline-title">{event.event_type}</div><div className="timeline-meta">{new Date(event.created_at).toLocaleString()}</div><div className="timeline-message">{event.message}</div></li>)}</ol>
          )}
        </div>
      </section>
    </main>
  )
}
