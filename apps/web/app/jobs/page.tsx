import { getJobsAdminView } from '../../lib/jobs-admin-api'
import JobsTables from './jobs-tables'

export default async function JobsPage() {
  const view = await getJobsAdminView()

  return (
    <main>
      <div className="page-header compact-page-header">
        <div>
          <h1>Operations</h1>
          <p>Deployments, scheduled work and control-plane tasks running in the background.</p>
        </div>
      </div>

      <div className="stats-grid operations-summary">
        <div className="stat-card"><div className="stat-label"><span>Queued</span></div><div className="stat-value">{view.queued_count}</div><div className="stat-meta">waiting to run</div></div>
        <div className="stat-card"><div className="stat-label"><span>Running</span><span className={`status-dot ${view.running_count ? 'online' : ''}`} /></div><div className="stat-value">{view.running_count}</div><div className="stat-meta">executing now</div></div>
        <div className="stat-card"><div className="stat-label"><span>Failed</span><span className={`status-dot ${view.dead_count ? 'danger' : 'online'}`} /></div><div className="stat-value">{view.dead_count}</div><div className="stat-meta">needs attention</div></div>
        <div className="stat-card"><div className="stat-label"><span>Schedules</span></div><div className="stat-value">{view.schedules.length}</div><div className="stat-meta">{view.schedules.filter((schedule) => schedule.enabled).length} enabled</div></div>
      </div>

      <JobsTables jobs={view.jobs} schedules={view.schedules} />
    </main>
  )
}
