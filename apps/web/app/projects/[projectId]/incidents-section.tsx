import Link from 'next/link'
import { getDependencyGraph } from '../../../lib/dependency-graph-api'
import { getIncidents } from '../../../lib/incidents-api'
import { createIncidentAction } from './incident-actions'

function severityClass(severity: string): string {
  if (severity === 'CRITICAL') return 'danger'
  if (severity === 'MAJOR') return 'warning'
  return 'info'
}

export default async function IncidentsSection({ projectId }: { projectId: string }) {
  const [incidents, graph] = await Promise.all([getIncidents(projectId), getDependencyGraph(projectId)])
  const openCount = incidents.filter((incident) => incident.status !== 'RESOLVED').length

  return (
    <section>
      <h2>Incidents</h2>
      <p>Creating an incident snapshots the selected resource and its dependency blast radius so later graph changes do not rewrite incident history.</p>

      <h3>Incident history</h3>
      <details className="create-drawer">
        <summary className="button">+ Create incident</summary>
        <div className="drawer-content">
          <form action={async (formData) => { 'use server'; await createIncidentAction(projectId, formData) }}>
            <div className="form-grid">
              <label>Title<input name="title" required maxLength={240} placeholder="Production API unavailable" /></label>
              <label>Severity<select name="severity" defaultValue="MAJOR"><option value="MINOR">Minor</option><option value="MAJOR">Major</option><option value="CRITICAL">Critical</option></select></label>
              <label className="full">Source / affected root<select name="source" required defaultValue=""><option value="" disabled>Select resource</option>{graph.nodes.map((node) => <option key={`${node.resource_type}-${node.resource_id}`} value={`${node.resource_type}:${node.resource_id}`}>{node.resource_type}: {node.name}{node.status ? ` — ${node.status}` : ''}</option>)}</select></label>
              <label className="full">Summary<textarea name="summary" maxLength={12000} placeholder="What is currently known?" /></label>
            </div>
            <button className="primary" type="submit">Create incident</button>
          </form>
        </div>
      </details>

      <div className="info-grid" style={{ margin: '0 17px 14px' }}>
        <div className="info-item"><span className="info-label">Open incidents</span><span className="info-value">{openCount}</span></div>
        <div className="info-item"><span className="info-label">Total history</span><span className="info-value">{incidents.length}</span></div>
      </div>

      {incidents.length === 0 ? <div className="empty-state"><strong>No incidents</strong>Operational incidents will appear here when created.</div> : (
        <div className="resource-list" style={{ margin: '0 17px 17px' }}>
          {incidents.map((incident) => (
            <Link className="resource-card" key={incident.id} href={`/projects/${projectId}/incidents/${incident.id}`}>
              <div className="resource-card-head">
                <div><h4>{incident.title}</h4><div className="resource-meta">{incident.source_type}: {incident.source_name} · started {new Date(incident.started_at).toLocaleString()}</div></div>
                <div className="action-row"><span className={`badge ${severityClass(incident.severity)}`}>{incident.severity}</span><span className={`badge ${incident.status === 'RESOLVED' ? 'success' : 'warning'}`}>{incident.status}</span></div>
              </div>
              <div className="detail-hero-meta"><span className="badge">{incident.affected_count} dependent resources</span></div>
            </Link>
          ))}
        </div>
      )}
    </section>
  )
}
