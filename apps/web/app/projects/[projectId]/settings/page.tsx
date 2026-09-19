import { revalidatePath } from 'next/cache'
import { getProjectWorkspace, updateProjectTools } from '../../../../lib/api'
import { projectTools } from '../../../../lib/project-tools'
export default async function Page({ params }: { params: { projectId: string } }) {
  const { project } = await getProjectWorkspace(params.projectId)
  async function save(data: FormData) {
    'use server'
    await updateProjectTools(params.projectId, data.getAll('tools').map(String))
    revalidatePath(`/projects/${params.projectId}`, 'layout')
  }
  return <main><h1>Project tools</h1><p>Choose what appears in this workspace. Hiding a tool keeps its data and running services intact.</p><form action={save}><div className="stack">{projectTools.map(tool => <label className="panel" key={tool.id}><input type="checkbox" name="tools" value={tool.id} defaultChecked={project.tools.includes(tool.id)} /><strong>{tool.label}</strong><p>{tool.description}</p></label>)}</div><button type="submit" className="primary">Save tools</button></form></main>
}
