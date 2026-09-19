import Link from 'next/link'
import { notFound } from 'next/navigation'
import { getContentWorkspace } from '../../../../../../lib/content-api'
export default async function Page({ params, searchParams }: { params: { projectId: string; modelId: string }; searchParams: { page?: string } }) {
 const workspace = await getContentWorkspace(params.projectId, Math.max(1, Number(searchParams.page) || 1), params.modelId)
 const model = workspace.models.find(model => model.id === params.modelId && model.content_role !== 'component')
 if (!model) notFound()
 const base = `/projects/${params.projectId}/content/collections/${model.id}`
 return <main><div className="page-header"><h1>{model.name}</h1>{workspace.permissions.can_edit && <Link className="button primary" href={`${base}/new`}>New entry</Link>}</div><div className="stack">{workspace.records.map(record => <Link className="panel" key={record.id} href={`${base}/${record.id}`}><strong>{String(record.values.title ?? record.values.name ?? record.values.slug ?? 'Untitled')}</strong><span className="badge">{record.editorial_status}</span><span>{record.lifecycle_status}</span></Link>)}</div>{!workspace.records.length && <p>No entries yet. Create your first entry to get started.</p>}<div className="action-row">{workspace.pagination.records.has_prev_page && <Link href={`${base}?page=${workspace.pagination.records.page - 1}`}>Previous</Link>}{workspace.pagination.records.has_next_page && <Link href={`${base}?page=${workspace.pagination.records.page + 1}`}>Next</Link>}</div></main>
}
