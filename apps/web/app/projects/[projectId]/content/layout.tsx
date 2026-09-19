import Link from 'next/link'
import { getProjectWorkspace } from '../../../../lib/api'
import './content-editor.css'
export default async function Layout({ params, children }: { params: { projectId: string }; children: React.ReactNode }) {
 const { project } = await getProjectWorkspace(params.projectId)
 const base = `/projects/${params.projectId}/content`
 return <><nav className="project-tabs" aria-label="Content"><Link href={base}>Pages</Link><Link href={`${base}/collections`}>Collections</Link><Link href={`${base}/media`}>Media</Link>{project.tools.includes('forms') && <Link href={`${base}/forms`}>Forms</Link>}<Link href={`${base}/connection`}>Site connection</Link><Link href={`${base}/structure`}>Content structure</Link></nav>{children}</>
}
