import Link from 'next/link'
import { getDependencyGraph } from '../../../lib/dependency-graph-api'
import { createDependencyAction, deleteDependencyAction } from './dependency-actions'

function statusClass(status: string | null): string {
  const value = (status ?? '').toLowerCase()
  if (value.includes('down') || value.includes('fail') || value.includes('critical')) return 'danger'
  if (value.includes('degraded') || value.includes('warning')) return 'warning'
  if (value.includes('up') || value.includes('healthy') || value.includes('active')) return 'success'
  return ''
}

export default async function DependencyGraphSection({ projectId }: { projectId: string }) {
  const graph = await getDependencyGraph(projectId)
  const labels = new Map(graph.nodes.map((node) => [`${node.resource_type}:${node.resource_id}`, `${node.resource_type}: ${node.name}`]))

  return (
    <section>
      <h2>Dependency graph &amp; impact</h2>
      <p>Every edge means source depends on target. Derived edges come from existing Argus links; manual edges model cross-service dependencies such as Web → API or API → Database.</p>

      <h3>Resources &amp; dependencies</h3>
      <details className="create-drawer">
        <summary className="button">+ Add manual dependency</summary>
        <div className="drawer-content">
          <form action={async (formData) => { 'use server'; await createDependencyAction(projectId, formData) }}>
            <div className="form-grid">
              <label>Dependent resource<select name="source" required defaultValue=""><option value="" disabled>Select resource</option>{graph.nodes.map((node) => <option key={`source-${node.resource_type}-${node.resource_id}`} value={`${node.resource_type}:${node.resource_id}`}>{node.resource_type}: {node.name}</option>)}</select></label>
              <label>Relationship<select name="relationship" defaultValue="DEPENDS_ON"><option value="DEPENDS_ON">depends on</option><option value="USES">uses</option></select></label>
              <label>Dependency<select name="target" required defaultValue=""><option value="" disabled>Select resource</option>{graph.nodes.map((node) => <option key={`target-${node.resource_type}-${node.resource_id}`} value={`${node.resource_type}:${node.resource_id}`}>{node.resource_type}: {node.name}</option>)}</select></label>
            </div>
            <button className="primary" type="submit">Add dependency</button>
          </form>
        </div>
      </details>

      <div className="info-grid" style={{ margin: '0 17px 14px' }}>
        <div className="info-item"><span className="info-label">Resources</span><span className="info-value">{graph.nodes.length}</span></div>
        <div className="info-item"><span className="info-label">Dependencies</span><span className="info-value">{graph.edges.length}</span></div>
      </div>

      {graph.nodes.length === 0 ? <div className="empty-state"><strong>No graph resources</strong>Resources appear as project infrastructure and services are connected.</div> : (
        <div className="resource-list" style={{ margin: '0 17px 14px' }}>
          {graph.nodes.map((node) => (
            <Link className="resource-card" key={`${node.resource_type}-${node.resource_id}`} href={`/projects/${projectId}/impact/${node.resource_type.toLowerCase()}/${node.resource_id}`}>
              <div className="resource-card-head"><div><h4>{node.name}</h4><div className="resource-meta">{node.resource_type}</div></div>{node.status ? <span className={`badge ${statusClass(node.status)}`}>{node.status}</span> : <span className="badge">No status</span>}</div>
            </Link>
          ))}
        </div>
      )}

      {graph.edges.length === 0 ? <div className="empty-state"><strong>No dependency edges</strong>Add a manual dependency or link resources that imply dependencies.</div> : (
        <div className="resource-list" style={{ margin: '0 17px 17px' }}>
          {graph.edges.map((edge, index) => {
            const source = labels.get(`${edge.source_type}:${edge.source_id}`) ?? `${edge.source_type}:${edge.source_id}`
            const target = labels.get(`${edge.target_type}:${edge.target_id}`) ?? `${edge.target_type}:${edge.target_id}`
            return (
              <div className="resource-card" key={edge.id ?? `derived-${index}-${edge.source_id}-${edge.target_id}`}>
                <div className="resource-card-head"><div><h4>{source}</h4><div className="resource-meta">{edge.relationship.replaceAll('_', ' ').toLowerCase()} → {target}</div></div><span className={`badge ${edge.origin === 'MANUAL' ? 'info' : ''}`}>{edge.origin}</span></div>
                {edge.origin === 'MANUAL' && edge.id ? <div className="action-row"><form action={async () => { 'use server'; await deleteDependencyAction(projectId, edge.id!) }}><button className="small danger" type="submit">Delete edge</button></form></div> : null}
              </div>
            )
          })}
        </div>
      )}
    </section>
  )
}
