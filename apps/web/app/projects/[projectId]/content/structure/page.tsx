import { getContentWorkspace } from '../../../../../lib/content-api'
import { ContentModelForm } from '../editors'
export default async function Page({ params }: { params: { projectId: string } }) {
 const workspace = await getContentWorkspace(params.projectId)
 const components = workspace.models.filter(model => model.content_role === 'component' && model.status === 'active')
 return <main><h1>Content structure</h1><p>Define what can be edited. Your website controls how it looks.</p>{workspace.permissions.can_manage ? <><details className="panel"><summary>New content type</summary><ContentModelForm projectId={params.projectId} models={workspace.models} components={components} /></details>{workspace.models.map(model => <details className="panel" key={model.id}><summary>{model.name} · {model.content_role}</summary><ContentModelForm projectId={params.projectId} model={model} models={workspace.models} components={components} /></details>)}</> : <p>A content manager can configure this project.</p>}</main>
}
