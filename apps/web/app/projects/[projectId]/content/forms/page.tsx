import { getContentWorkspace, getFormsWorkspace } from '../../../../../lib/content-api'
import { FormsSection } from '../forms-section'
export default async function Page({ params, searchParams }: { params: { projectId: string }; searchParams: { submission_page?: string } }) {
 const [workspace, forms] = await Promise.all([getContentWorkspace(params.projectId), getFormsWorkspace(params.projectId, Math.max(1, Number(searchParams.submission_page) || 1))])
 return <main><h1>Forms</h1><FormsSection projectId={params.projectId} workspace={forms} publicBase={process.env.ARGUS_CONTENT_PUBLIC_URL} canEdit={workspace.permissions.can_edit} canManage={workspace.permissions.can_manage} /></main>
}
