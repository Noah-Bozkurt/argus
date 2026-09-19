import Link from 'next/link'
import { getContentWorkspace } from '../../../../../lib/content-api'
import { PublishForm } from './publish-form'
export default async function Page({ params, searchParams }: { params: { projectId: string }; searchParams: { page?: string } }) {
 const page = Math.max(1, Number(searchParams.page) || 1)
 const workspace = await getContentWorkspace(params.projectId, page)
 const publicModels = new Set(workspace.models.filter(model => model.public_read).map(model => model.id))
 return <main><h1>Publish changes</h1>{workspace.permissions.can_edit ? <PublishForm projectId={params.projectId} entries={workspace.records.filter(record => record.lifecycle_status === 'active' && publicModels.has(record.model_id)).map(record => ({ id: record.id, label: String(record.values.title ?? record.values.name ?? record.id), status: record.editorial_status }))} /> : <p>An editor can publish changes.</p>}<nav>{workspace.pagination.records.has_prev_page && <Link href={`?page=${page-1}`}>Previous</Link>}{workspace.pagination.records.has_next_page && <Link href={`?page=${page+1}`}>Next</Link>}</nav></main>
}
