import Link from 'next/link'
import { getProjects } from '../../lib/api'
import { createProjectAction } from './actions'

export default async function ProjectsPage() {
  const projects = await getProjects()

  return (
    <main>
      <h1>Projects</h1>
      <p>Projects are first-class workspaces. A client relationship is optional and not required for any project.</p>

      <h2>Create project</h2>
      <form action={createProjectAction}>
        <label>
          Name
          <input name="name" required maxLength={120} />
        </label>
        <label>
          Description
          <textarea name="description" maxLength={4000} />
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
        <label>
          Tags
          <input name="tags" placeholder="personal, rust, production" />
        </label>
        <button type="submit">Create project</button>
      </form>

      <h2>Workspace</h2>
      {projects.length === 0 ? (
        <p>No projects yet.</p>
      ) : (
        <ul>
          {projects.map((project) => (
            <li key={project.id}>
              <Link href={`/projects/${project.id}`}><strong>{project.name}</strong></Link>
              {' — '}{project.preset} — {project.open_tasks} open task{project.open_tasks === 1 ? '' : 's'}
              {project.tags.length ? ` — ${project.tags.join(', ')}` : ''}
              {project.description ? <p>{project.description}</p> : null}
            </li>
          ))}
        </ul>
      )}
    </main>
  )
}
