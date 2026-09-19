import { notFound } from 'next/navigation'
import { getContentWorkspace, getMediaLibrary } from '../../../../../../../lib/content-api'
import { RecordForm } from '../../../editors'
export default async function Page({ params }: { params: { projectId: string; modelId: string; recordId: string } }) {
 const [workspace, media] = await Promise.all([getContentWorkspace(params.projectId, 1, params.modelId, params.recordId === 'new' ? undefined : params.recordId), getMediaLibrary(params.projectId)])
 const model = workspace.models.find(model => model.id === params.modelId && model.content_role !== 'component')
 const record = workspace.records.find(record => record.id === params.recordId)
 if (!model || (params.recordId !== 'new' && !record)) notFound()
 if (!workspace.permissions.can_edit) return <main><h1>Read-only entry</h1><pre>{JSON.stringify(record?.values, null, 2)}</pre></main>
 return <main><h1>{record ? String(record.values.title ?? record.values.name ?? model.name) : `New ${model.name} entry`}</h1><RecordForm projectId={params.projectId} model={model} record={record} components={workspace.models.filter(component => model.allowed_component_ids.includes(component.id))} workspace={workspace} media={media} publicBase={process.env.ARGUS_CONTENT_PUBLIC_URL} /></main>
}
