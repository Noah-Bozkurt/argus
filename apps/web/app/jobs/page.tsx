import { getJobsAdminView } from '../../lib/jobs-admin-api'
import JobsTables from './jobs-tables'

export default async function JobsPage() {
  const view = await getJobsAdminView()

  return (
    <main>
      <div className="page-header">
        <div>
          <span className="eyebrow">Operations</span>
          <h1>Background jobs</h1>
          <p>Queue health, schedules and recent background work across the Argus control plane.</p>
        </div>
      </div>

      <div className="stats-grid">
        <div className="stat-card"><div className="stat-label"><span>Queued</span><span className="badge warning">Pending</span></div><div className="stat-value">{view.queued_count}</div><div className="stat-meta">Waiting for a worker</div></div>
        <div className="stat-card"><div className="stat-label"><span>Running</span><span className="badge info">Live</span></div><div className="stat-value">{view.running_count}</div><div className="stat-meta">Currently executing</div></div>
        <div className="stat-card"><div className="stat-label"><span>Dead</span><span className={`status-dot ${view.dead_count ? 'danger' : 'online'}`} /></div><div className="stat-value">{view.dead_count}</div><div className="stat-meta">Requires operator attention</div></div>
        <div className="stat-card"><div className="stat-label"><span>Schedules</span><span className="badge">Configured</span></div><div className="stat-value">{view.schedules.length}</div><div className="stat-meta">{view.schedules.filter((schedule) => schedule.enabled).length} enabled</div></div>
      </div>

      <JobsTables jobs={view.jobs} schedules={view.schedules} />
    </main>
  )
}
