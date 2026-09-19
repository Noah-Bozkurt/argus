 'use client'
import Link from 'next/link'
import { usePathname } from 'next/navigation'
import { projectTools } from '../../../lib/project-tools'
export function ProjectNavigation({ projectId, tools }: { projectId: string; tools: string[] }) {
  const pathname = usePathname()
  const base = `/projects/${projectId}`
  return <nav className="project-tabs" aria-label="Project"><Link href={base} aria-current={pathname === base ? 'page' : undefined}>Overview</Link>{projectTools.filter(tool => tools.includes(tool.id)).map(tool => <Link key={tool.id} href={`${base}/${tool.path}`} aria-current={pathname.startsWith(`${base}/${tool.path}`) ? 'page' : undefined}>{tool.label}</Link>)}<Link href={`${base}/settings`} aria-current={pathname.endsWith('/settings') ? 'page' : undefined}>Settings</Link></nav>
}
