import Link from 'next/link'
import { getDependencyGraph } from '../../../lib/dependency-graph-api'
import { createDependencyAction, deleteDependencyAction } from './dependency-actions'

export default async function DependencyGraphSection({ projectId }: { projectId: string }) {
  const graph = await getDependencyGraph(projectId)
  const labels = new Map(
    graph.nodes.map((node) => [`${node.resource_type}:${node.resource_id}`, `${node.resource_type}: ${node.name}`]),
  )

  return (
    <section>
      <h2>Dependency Graph &amp; Impact</h2>
      <p>
        Every edge means source depends on target. Derived edges come from existing Argus links; manual edges model cross-service dependencies such as Web → API or API → Database.
      </p>

      <h3>Add manual dependency</h3>
      <form action={async (formData) => { 'use server'; await createDependencyAction(projectId, formData) }}>
        <label>
          Dependent resource (source)
          <select name="source" required defaultValue="">
            <option value="" disabled>Select resource</option>
            {graph.nodes.map((node) => (
              <option key={`source-${node.resource_type}-${node.resource_id}`} value={`${node.resource_type}:${node.resource_id}`}>
                {node.resource_type}: {node.name}
              </option>
            ))}
          </select>
        </label>
        <label>
          Relationship
          <select name="relationship" defaultValue="DEPENDS_ON">
            <option value="DEPENDS_ON">depends on</option>
            <option value="USES">uses</option>
          </select>
        </label>
        <label>
          Dependency (target)
          <select name="target" required defaultValue="">
            <option value="" disabled>Select resource</option>
            {graph.nodes.map((node) => (
              <option key={`target-${node.resource_type}-${node.resource_id}`} value={`${node.resource_type}:${node.resource_id}`}>
                {node.resource_type}: {node.name}
              </option>
            ))}
          </select>
        </label>
        <button type="submit">Add dependency</button>
      </form>

      <h3>Resources</h3>
      {graph.nodes.length === 0 ? <p>No graph resources yet.</p> : (
        <ul>
          {graph.nodes.map((node) => (
            <li key={`${node.resource_type}-${node.resource_id}`}>
              <Link href={`/projects/${projectId}/impact/${node.resource_type.toLowerCase()}/${node.resource_id}`}>
                {node.resource_type}: {node.name}
              </Link>
              {node.status ? ` — ${node.status}` : ''}
            </li>
          ))}
        </ul>
      )}

      <h3>Dependencies</h3>
      {graph.edges.length === 0 ? <p>No dependency edges yet.</p> : (
        <ul>
          {graph.edges.map((edge, index) => {
            const source = labels.get(`${edge.source_type}:${edge.source_id}`) ?? `${edge.source_type}:${edge.source_id}`
            const target = labels.get(`${edge.target_type}:${edge.target_id}`) ?? `${edge.target_type}:${edge.target_id}`
            return (
              <li key={edge.id ?? `derived-${index}-${edge.source_id}-${edge.target_id}`}>
                {source} — {edge.relationship} → {target} — {edge.origin}
                {edge.origin === 'MANUAL' && edge.id ? (
                  <form action={async () => { 'use server'; await deleteDependencyAction(projectId, edge.id!) }}>
                    <button type="submit">Delete manual edge</button>
                  </form>
                ) : null}
              </li>
            )
          })}
        </ul>
      )}
    </section>
  )
}
