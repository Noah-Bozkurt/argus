import Link from 'next/link'
import { getProjects } from '../../lib/api'
import { createProjectAction } from './actions'

function relativeTime(value: string): string {
  const delta = Date.now() - new Date(value).getTime()
  const minutes = Math.max(0, Math.floor(delta / 60000))
  if (minutes < 60) return minutes < 1 ? 'just now' : `${minutes}m ago`
  const hours = Math.floor(minutes / 60)
  if (hours < 24) return `${hours}h ago`
  return `${Math.floor(hours / 24)}d ago`
}

export default async function ProjectsPage() {
  const projects = await getProjects()
  const orderedProjects = [...projects].sort((a, b) => Date.parse(b.updated_at) - Date.parse(a.updated_at))

  return (
    <main>
      <div className="page-header">
        <div>
          <span className="eyebrow">Workspace</span>
          <h1>Projects</h1>
          <p>Projects are first-class workspaces. Client context stays optional, so personal software, websites and infrastructure fit the same model.</p>
        </div>
      </div>

      <details className="create-drawer">
        <summary className="button primary">+ New project</summary>
        <div className="drawer-content">
          <form action={createProjectAction}>
            <div className="form-grid">
              <label>
                Name
                <input name="name" required maxLength={120} placeholder="Argus" />
              </label>
              <label>
                Preset
                <select name="preset" defaultValue="empty">
                  <option value="empty">Empty Project</option>
                  <option value="software">Software Project</option>
                  <option value="website">Website</option>
                  <option value="infrastructure">Infrastructure</option>
                  <option value="client">Client Project</option>
                </select>
              </label>
              <label className="full">
                Description
                <textarea name="description" maxLength={4000} placeholder="What does this project contain?" />
              </label>
              <label className="full">
                Tags
                <input name="tags" placeholder="personal, rust, production" />
              </label>
            </div>
            <button className="primary" type="submit">Create project</button>
          </form>
        </div>
      </details>

      <section className="panel">
        <div className="panel-header">
          <div><h2>All projects</h2><p>{projects.length} workspace{projects.length === 1 ? '' : 's'}</p></div>
        </div>
        {orderedProjects.length === 0 ? (
          <div className="empty-state"><strong>No projects yet</strong>Use “New project” to create the first workspace.</div>
        ) : (
          <ul className="data-list">
            {orderedProjects.map((project) => (
              <li className="data-row" key={project.id}>
                <div>
                  <div className="row-title">
                    <span className="status-dot online" />
                    <Link href={`/projects/${project.id}`}>{project.name}</Link>
                    <span className="badge">{project.preset}</span>
                  </div>
                  <div className="row-subtitle">{project.description || 'No project description'}{project.tags.length ? ` · ${project.tags.join(' · ')}` : ''}</div>
                </div>
                <div className="row-meta">
                  <span>{project.open_tasks} open task{project.open_tasks === 1 ? '' : 's'}</span>
                  <span className="badge success">{project.status}</span>
                  <span>Updated {relativeTime(project.updated_at)}</span>
                </div>
              </li>
            ))}
          </ul>
        )}
      </section>
    </main>
  )
}
