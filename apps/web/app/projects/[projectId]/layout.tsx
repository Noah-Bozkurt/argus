import Link from 'next/link'
import { getProjectWorkspace } from '../../../lib/api'
import { ProjectNavigation } from './project-navigation'
export default async function Layout({ children, params }: { children: React.ReactNode; params: { projectId: string } }) {
  const { project } = await getProjectWorkspace(params.projectId)
  return <><div className="page-header"><Link href="/projects">← Projects</Link><strong>{project.name}</strong></div><ProjectNavigation projectId={project.id} tools={project.tools} />{children}</>
}
