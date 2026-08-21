import Link from 'next/link'
import { getDependencyImpact } from '../../../../../../lib/dependency-graph-api'

export default async function DependencyImpactPage({ params }: { params: { projectId: string; resourceType: string; resourceId: string } }) {
  const impact = await getDependencyImpact(params.projectId, params.resourceType, params.resourceId)

  return (
    <main className="detail-page">
      <div className="page-header">
        <div>
          <Link className="panel-link" href={`/projects/${params.projectId}`}>← Project</Link>
          <div style={{ marginTop: 10 }}><span className="eyebrow">Dependency impact</span><h1>{impact.root.name}</h1></div>
          <div className="detail-hero-meta"><span className="badge info">{impact.root.resource_type}</span><span className="badge">{impact.affected_count} dependent resources</span></div>
        </div>
      </div>

      <section className="detail-card">
        <div className="detail-card-header"><div><h2>Potential blast radius</h2><p>Resources downstream from this node if it becomes unavailable.</p></div></div>
        <div className="detail-card-body">
          {impact.affected.length === 0 ? <div className="empty-state"><strong>No dependent resources</strong>This resource currently has no downstream dependencies.</div> : (
            <ol className="path-list">
              {impact.affected.map((item) => (
                <li className="path-card" key={`${item.resource.resource_type}-${item.resource.resource_id}`}>
                  <div className="resource-card-head"><strong>{item.resource.resource_type}: {item.resource.name}</strong><span className="badge">Distance {item.distance}</span></div>
                  <div className="path-line">{item.path.map((resource) => `${resource.resource_type}: ${resource.name}`).join(' → ')}</div>
                </li>
              ))}
            </ol>
          )}
        </div>
      </section>
    </main>
  )
}
