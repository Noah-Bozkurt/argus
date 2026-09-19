import Link from 'next/link'
import { getContentWorkspace } from '../../../../../lib/content-api'
export default async function Page({ params }: { params: { projectId: string } }) {
 const workspace = await getContentWorkspace(params.projectId)
 const models = workspace.models.filter(model => model.content_role === 'collection' && model.status === 'active')
 return <main><h1>Collections</h1><p>Write and organize repeatable content.</p><div className="stats-grid">{models.map(model => <Link className="stat-card" key={model.id} href={`/projects/${params.projectId}/content/collections/${model.id}`}><h2>{model.name}</h2><p>{model.description}</p></Link>)}</div>{!models.length && <Link className="button" href={`/projects/${params.projectId}/content/structure`}>Create a content type</Link>}</main>
}
