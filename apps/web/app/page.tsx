import Link from 'next/link'
import { getProjects, getServers } from '../lib/api'
import { getJobsAdminView } from '../lib/jobs-admin-api'

function relativeTime(value: string): string {
  const delta = Date.now() - new Date(value).getTime()
  const minutes = Math.max(0, Math.floor(delta / 60000))
  if (minutes < 1) return 'just now'
  if (minutes < 60) return `${minutes}m ago`
  const hours = Math.floor(minutes / 60)
  if (hours < 24) return `${hours}h ago`
  return `${Math.floor(hours / 24)}d ago`
}

function statusClass(status: string): string {
  const normalized = status.toLowerCase()
  if (normalized.includes('active') || normalized.includes('success') || normalized.includes('healthy')) return 'success'
  if (normalized.includes('fail') || normalized.includes('dead') || normalized.includes('error')) return 'danger'
  if (normalized.includes('run') || normalized.includes('pending')) return 'info'
  return ''
}

export default async function DashboardPage() {
  const [projects, servers, jobs] = await Promise.all([getProjects(), getServers(), getJobsAdminView()])
  const onlineServers = servers.filter((server) => server.online).length
  const recentProjects = [...projects].sort((a, b) => Date.parse(b.updated_at) - Date.parse(a.updated_at)).slice(0, 6)
  const recentJobs = jobs.jobs.slice(0, 6)

  return (
    <main>
      <div className="page-header">
        <div>
          <span className="eyebrow">Control plane</span>
          <h1>Overview</h1>
          <p>A focused view of your projects, infrastructure and background operations.</p>
        </div>
        <div className="page-actions">
          <Link className="button" href="/infrastructure/servers">View infrastructure</Link>
          <Link className="button primary" href="/projects">Open projects</Link>
        </div>
      </div>

      <div className="stats-grid">
        <Link className="stat-card" href="/projects">
          <div className="stat-label"><span>Projects</span><span className="badge info">Workspace</span></div>
          <div className="stat-value">{projects.length}</div>
          <div className="stat-meta">{projects.reduce((sum, project) => sum + project.open_tasks, 0)} open tasks</div>
        </Link>
        <Link className="stat-card" href="/infrastructure/servers">
          <div className="stat-label"><span>Servers online</span><span className={`status-dot ${onlineServers === servers.length ? 'online' : 'warning'}`} /></div>
          <div className="stat-value">{onlineServers}<span className="muted">/{servers.length}</span></div>
          <div className="stat-meta">Control-plane heartbeats</div>
        </Link>
        <Link className="stat-card" href="/jobs">
          <div className="stat-label"><span>Running jobs</span><span className="badge info">Live</span></div>
          <div className="stat-value">{jobs.running_count}</div>
          <div className="stat-meta">{jobs.queued_count} queued</div>
        </Link>
        <Link className="stat-card" href="/jobs">
          <div className="stat-label"><span>Failed jobs</span><span className={`status-dot ${jobs.dead_count ? 'danger' : 'online'}`} /></div>
          <div className="stat-value">{jobs.dead_count}</div>
          <div className="stat-meta">Requires operator attention</div>
        </Link>
      </div>

      <div className="dashboard-grid">
        <section className="panel">
          <div className="panel-header">
            <div><h2>Recent projects</h2><p>Workspaces with the latest activity</p></div>
            <Link className="panel-link" href="/projects">View all</Link>
          </div>
          {recentProjects.length === 0 ? (
            <div className="empty-state"><strong>No projects yet</strong>Create a project to start building your workspace.</div>
          ) : (
            <ul className="data-list">
              {recentProjects.map((project) => (
                <li className="data-row" key={project.id}>
                  <div>
                    <div className="row-title"><span className="status-dot online" /><Link href={`/projects/${project.id}`}>{project.name}</Link></div>
                    <div className="row-subtitle">{project.description || `${project.preset} project`} · {project.tags.join(' · ') || 'no tags'}</div>
                  </div>
                  <div className="row-meta"><span>{project.open_tasks} tasks</span><span className={`badge ${statusClass(project.status)}`}>{project.status}</span><span>{relativeTime(project.updated_at)}</span></div>
                </li>
              ))}
            </ul>
          )}
        </section>

        <section className="panel">
          <div className="panel-header">
            <div><h2>Background activity</h2><p>Latest jobs across Argus</p></div>
            <Link className="panel-link" href="/jobs">Open jobs</Link>
          </div>
          {recentJobs.length === 0 ? (
            <div className="empty-state"><strong>No jobs yet</strong>Background work will appear here.</div>
          ) : (
            <ul className="data-list">
              {recentJobs.map((job) => (
                <li className="data-row" key={job.id}>
                  <div>
                    <div className="row-title"><span className={`status-dot ${job.status === 'DEAD' ? 'danger' : job.status === 'SUCCEEDED' ? 'online' : ''}`} />{job.job_kind}</div>
                    <div className="row-subtitle">{job.project_name ?? 'Workspace'} · {job.resource_key || 'default'}</div>
                  </div>
                  <div className="row-meta"><span className={`badge ${statusClass(job.status)}`}>{job.status}</span></div>
                </li>
              ))}
            </ul>
          )}
        </section>
      </div>
    </main>
  )
}
