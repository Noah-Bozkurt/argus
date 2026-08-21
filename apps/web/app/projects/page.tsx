import { getProjects } from '../../lib/api'
import { createProjectAction } from './actions'
import ProjectList from './project-list'

export default async function ProjectsPage() {
  const projects = await getProjects()

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

      <ProjectList projects={projects} />
    </main>
  )
}
