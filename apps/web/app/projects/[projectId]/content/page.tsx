import Link from 'next/link'
import { getContentWorkspace } from '../../../../lib/content-api'
export default async function Page({ params }: { params: { projectId: string } }) {
 const workspace = await getContentWorkspace(params.projectId)
 const models = workspace.models.filter(model => model.content_role === 'page' && model.status === 'active')
 return <main><div className="page-header"><div><h1>Pages</h1><p>Edit your website using its own design and sections.</p></div><Link className="button" href={`/projects/${params.projectId}/content/connection`}>Connect your site</Link></div>{!models.length && <div className="empty-state"><h2>Start your website workspace</h2><p>Connect your website and configure its page types to start editing.</p><Link className="button primary" href={`/projects/${params.projectId}/content/connection`}>Set up Content</Link></div>}<div className="stats-grid">{models.map(model => <Link className="stat-card" key={model.id} href={`/projects/${params.projectId}/content/collections/${model.id}`}><h2>{model.name}</h2><p>{model.description || 'View and edit pages'}</p></Link>)}</div></main>
}
