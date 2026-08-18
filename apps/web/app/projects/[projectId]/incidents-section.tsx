import Link from 'next/link'
import { getDependencyGraph } from '../../../lib/dependency-graph-api'
import { getIncidents } from '../../../lib/incidents-api'
import { createIncidentAction } from './incident-actions'

export default async function IncidentsSection({ projectId }: { projectId: string }) {
  const [incidents, graph] = await Promise.all([
    getIncidents(projectId),
    getDependencyGraph(projectId),
  ])

  return (
    <section>
      <h2>Incidents</h2>
      <p>
        Creating an incident snapshots the selected resource and its current dependency blast radius. Later graph edits do not rewrite incident history.
      </p>

      <h3>Create incident</h3>
      <form action={async (formData) => { 'use server'; await createIncidentAction(projectId, formData) }}>
        <label>
          Title
          <input name="title" required maxLength={240} placeholder="Production API unavailable" />
        </label>
        <label>
          Severity
          <select name="severity" defaultValue="MAJOR">
            <option value="MINOR">Minor</option>
            <option value="MAJOR">Major</option>
            <option value="CRITICAL">Critical</option>
          </select>
        </label>
        <label>
          Source / affected root
          <select name="source" required defaultValue="">
            <option value="" disabled>Select resource</option>
            {graph.nodes.map((node) => (
              <option key={`${node.resource_type}-${node.resource_id}`} value={`${node.resource_type}:${node.resource_id}`}>
                {node.resource_type}: {node.name}{node.status ? ` — ${node.status}` : ''}
              </option>
            ))}
          </select>
        </label>
        <label>
          Summary
          <textarea name="summary" maxLength={12000} placeholder="What is currently known?" />
        </label>
        <button type="submit">Create incident</button>
      </form>

      <h3>Incident history</h3>
      {incidents.length === 0 ? <p>No incidents yet.</p> : (
        <ul>
          {incidents.map((incident) => (
            <li key={incident.id}>
              <Link href={`/projects/${projectId}/incidents/${incident.id}`}>
                {incident.title}
              </Link>
              {' — '}{incident.severity} — {incident.status}
              {' — '}root {incident.source_type}: {incident.source_name}
              {' — '}{incident.affected_count} dependent resource(s)
              {' — '}{new Date(incident.started_at).toLocaleString()}
            </li>
          ))}
        </ul>
      )}
    </section>
  )
}
