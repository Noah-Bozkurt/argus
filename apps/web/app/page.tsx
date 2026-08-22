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
  if (normalized.includes('run') || normalized.includes('pending') || normalized.includes('queued')) return 'info'
  return ''
}

function average(values: Array<number | undefined>): number | null {
  const present = values.filter((value): value is number => typeof value === 'number')
  if (!present.length) return null
  return Math.round(present.reduce((sum, value) => sum + value, 0) / present.length)
}

export default async function DashboardPage() {
  const [projects, servers, jobs] = await Promise.all([getProjects(), getServers(), getJobsAdminView()])
  const onlineServers = servers.filter((server) => server.online).length
  const recentProjects = [...projects].sort((a, b) => Date.parse(b.updated_at) - Date.parse(a.updated_at)).slice(0, 5)
  const recentJobs = jobs.jobs.slice(0, 6)
  const openTasks = projects.reduce((sum, project) => sum + project.open_tasks, 0)
  const cpu = average(servers.map((server) => server.snapshot?.cpu_percent))
  const ram = average(servers.map((server) => server.snapshot?.ram_percent))
  const disk = average(servers.map((server) => server.snapshot?.disk_percent))
  const healthy = onlineServers === servers.length && jobs.dead_count === 0

  return (
    <main className="overview-page">
      <div className="page-header compact-page-header">
        <div>
          <h1>Overview</h1>
          <p>Your projects, infrastructure and control-plane activity in one place.</p>
        </div>
      </div>

      <section className={`control-summary${healthy ? '' : ' needs-attention'}`}>
        <div className="control-summary-state">
          <span className={`signal-indicator ${healthy ? 'healthy' : 'warning'}`} aria-hidden="true"><span /></span>
          <div>
            <strong>{healthy ? 'Everything is operational' : 'Argus needs your attention'}</strong>
            <p>{healthy ? `${onlineServers} of ${servers.length} servers online · no failed operations` : `${servers.length - onlineServers} offline server${servers.length - onlineServers === 1 ? '' : 's'} · ${jobs.dead_count} failed operation${jobs.dead_count === 1 ? '' : 's'}`}</p>
          </div>
        </div>
        <div className="control-summary-facts" aria-label="Workspace summary">
          <Link href="/projects"><span>Projects</span><strong>{projects.length}</strong><small>{openTasks} open tasks</small></Link>
          <Link href="/infrastructure/servers"><span>Servers</span><strong>{onlineServers}/{servers.length}</strong><small>reporting online</small></Link>
          <Link href="/jobs"><span>Operations</span><strong>{jobs.running_count}</strong><small>{jobs.queued_count} queued</small></Link>
        </div>
      </section>

      <div className="overview-layout">
        <section className="resource-section overview-projects">
          <div className="section-bar">
            <div><h2>Projects</h2><p>Recently active workspaces</p></div>
            <Link className="text-action" href="/projects">View all</Link>
          </div>
          {recentProjects.length === 0 ? (
            <div className="empty-state"><strong>No projects yet</strong>Create your first workspace from Projects.</div>
          ) : (
            <ul className="clean-list">
              {recentProjects.map((project) => (
                <li key={project.id}>
                  <Link className="resource-row-link" href={`/projects/${project.id}`}>
                    <span className="resource-primary">
                      <span className="resource-name">{project.name}</span>
                      <span className="resource-description">{project.description || `${project.preset} project`}</span>
                    </span>
                    <span className="resource-row-meta"><span>{project.open_tasks} task{project.open_tasks === 1 ? '' : 's'}</span><span className={`state-label ${statusClass(project.status)}`}>{project.status}</span><time>{relativeTime(project.updated_at)}</time></span>
                  </Link>
                </li>
              ))}
            </ul>
          )}
        </section>

        <div className="overview-side">
          <section className="resource-section infrastructure-summary">
            <div className="section-bar">
              <div><h2>Infrastructure</h2><p>Current fleet utilization</p></div>
              <Link className="text-action" href="/infrastructure/servers">Servers</Link>
            </div>
            <div className="metric-lines">
              <div className="metric-line"><span>CPU</span><div className="metric-track"><i style={{ width: `${cpu ?? 0}%` }} /></div><strong>{cpu === null ? '—' : `${cpu}%`}</strong></div>
              <div className="metric-line"><span>Memory</span><div className="metric-track"><i style={{ width: `${ram ?? 0}%` }} /></div><strong>{ram === null ? '—' : `${ram}%`}</strong></div>
              <div className="metric-line"><span>Disk</span><div className="metric-track"><i style={{ width: `${disk ?? 0}%` }} /></div><strong>{disk === null ? '—' : `${disk}%`}</strong></div>
            </div>
            <div className="signal-strip" aria-hidden="true"><i /><i /><i /><i /><i /><i /><i /><i /><i /><i /><i /><i /></div>
          </section>

          <section className="resource-section activity-section">
            <div className="section-bar">
              <div><h2>Recent activity</h2><p>Latest control-plane operations</p></div>
              <Link className="text-action" href="/jobs">Open</Link>
            </div>
            {recentJobs.length === 0 ? (
              <div className="empty-state compact"><strong>No recent activity</strong>Operations will appear here.</div>
            ) : (
              <ul className="activity-list">
                {recentJobs.map((job) => (
                  <li key={job.id}>
                    <span className={`activity-dot ${statusClass(job.status)}`} />
                    <div><strong>{job.job_kind}</strong><span>{job.project_name ?? 'Workspace'} · {job.resource_key || 'default'}</span></div>
                    <time>{relativeTime(job.updated_at)}</time>
                  </li>
                ))}
              </ul>
            )}
          </section>
        </div>
      </div>
    </main>
  )
}
