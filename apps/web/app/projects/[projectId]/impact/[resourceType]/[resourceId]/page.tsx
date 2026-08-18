import Link from 'next/link'
import { getDependencyImpact } from '../../../../../lib/dependency-graph-api'

export default async function DependencyImpactPage({
  params,
}: {
  params: { projectId: string; resourceType: string; resourceId: string }
}) {
  const impact = await getDependencyImpact(
    params.projectId,
    params.resourceType,
    params.resourceId,
  )

  return (
    <main>
      <p><Link href={`/projects/${params.projectId}`}>← Project</Link></p>
      <h1>Impact: {impact.root.resource_type}: {impact.root.name}</h1>
      <p>{impact.affected_count} dependent resource(s) would be affected if this resource became unavailable.</p>

      {impact.affected.length === 0 ? <p>No dependent resources found.</p> : (
        <ol>
          {impact.affected.map((item) => (
            <li key={`${item.resource.resource_type}-${item.resource.resource_id}`}>
              <p>
                <strong>{item.resource.resource_type}: {item.resource.name}</strong>
                {' — '}distance {item.distance}
              </p>
              <p>
                {item.path.map((resource) => `${resource.resource_type}: ${resource.name}`).join(' → ')}
              </p>
            </li>
          ))}
        </ol>
      )}
    </main>
  )
}
