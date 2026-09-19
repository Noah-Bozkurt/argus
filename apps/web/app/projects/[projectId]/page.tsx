import Link from 'next/link'
import { getProjectWorkspace } from '../../../lib/api'
import { projectTools } from '../../../lib/project-tools'
export default async function Page({ params }: { params: { projectId: string } }) {
  const { project, activity, tasks } = await getProjectWorkspace(params.projectId)
  return <main><div className="page-header"><div><h1>{project.name}</h1><p>{project.description || 'Your project workspace.'}</p></div><Link className="button" href={`/projects/${project.id}/settings`}>Add tools</Link></div>
    <div className="stats-grid">{projectTools.filter(tool => project.tools.includes(tool.id)).map(tool => <Link className="stat-card" href={`/projects/${project.id}/${tool.path}`} key={tool.id}><h2>{tool.label}</h2><p>{tool.description}</p></Link>)}</div>
    {!project.tools.length && <div className="empty-state"><h2>Make this workspace yours</h2><p>Add the tools you need. You can change them at any time.</p><Link className="button primary" href={`/projects/${project.id}/settings`}>Choose tools</Link></div>}
    {project.tools.includes('work') && <section className="panel"><h2>Next up</h2>{tasks.filter(task => !['DONE', 'CANCELLED'].includes(task.status)).slice(0, 5).map(task => <p key={task.id}>{task.title} · {task.status}</p>)}</section>}
    <section className="panel"><h2>Recent activity</h2>{activity.slice(0, 10).map((event, index) => <p key={index}>{event.event_type} · {new Date(event.occurred_at).toLocaleString()}</p>)}{!activity.length && <p>Your project activity will appear here.</p>}</section>
  </main>
}
