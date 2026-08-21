import { getJobsAdminView } from '../../lib/jobs-admin-api'
import { retryDeadJobAction } from './actions'

function formatDate(value: string | null): string {
  return value ? new Date(value).toLocaleString() : '—'
}

function formatInterval(seconds: number): string {
  if (seconds % 3600 === 0) return `${seconds / 3600}h`
  if (seconds % 60 === 0) return `${seconds / 60}m`
  return `${seconds}s`
}

function statusClass(status: string): string {
  if (status === 'SUCCEEDED') return 'success'
  if (status === 'DEAD') return 'danger'
  if (status === 'RUNNING') return 'info'
  if (status === 'QUEUED') return 'warning'
  return ''
}

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

      <section className="panel" style={{ marginBottom: 16 }}>
        <div className="panel-header"><div><h2>Schedules</h2><p>Recurring work configured by its owning feature</p></div></div>
        {view.schedules.length === 0 ? <div className="empty-state"><strong>No schedules configured</strong>Feature-owned schedules will appear here.</div> : (
          <div className="table-wrap" style={{ border: 0, borderRadius: 0 }}>
            <table className="responsive-table">
              <thead><tr><th>Job</th><th>Scope</th><th>Resource</th><th>Interval</th><th>Status</th><th>Next run</th></tr></thead>
              <tbody>
                {view.schedules.map((schedule) => (
                  <tr key={schedule.id}>
                    <td><strong>{schedule.job_kind}</strong></td>
                    <td data-label="Scope">{schedule.project_name ?? 'Workspace'}</td>
                    <td data-label="Resource"><code>{schedule.resource_key || 'default'}</code></td>
                    <td data-label="Interval">{formatInterval(schedule.interval_seconds)}</td>
                    <td data-label="Status"><span className={`badge ${schedule.enabled ? 'success' : ''}`}>{schedule.enabled ? 'Enabled' : 'Disabled'}</span></td>
                    <td data-label="Next run">{formatDate(schedule.next_run_at)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </section>

      <section className="panel">
        <div className="panel-header"><div><h2>Recent jobs</h2><p>Latest {view.jobs.length} executions; payloads stay hidden by design</p></div></div>
        {view.jobs.length === 0 ? <div className="empty-state"><strong>No background jobs yet</strong>Executions will appear here when work is queued.</div> : (
          <div className="table-wrap" style={{ border: 0, borderRadius: 0 }}>
            <table className="responsive-table">
              <thead><tr><th>Job</th><th>Project</th><th>Resource</th><th>Status</th><th>Attempts</th><th>Run at</th><th>Action</th></tr></thead>
              <tbody>
                {view.jobs.map((job) => (
                  <tr key={job.id}>
                    <td>
                      <strong>{job.job_kind}</strong>
                      {job.last_error_message ? <div className="text-danger" style={{ marginTop: 4 }}>{job.last_error_code ?? 'EXECUTION_FAILED'} · {job.last_error_message}</div> : null}
                    </td>
                    <td data-label="Project">{job.project_name ?? 'Workspace'}</td>
                    <td data-label="Resource"><code>{job.resource_key || 'default'}</code></td>
                    <td data-label="Status"><span className={`badge ${statusClass(job.status)}`}>{job.status}</span></td>
                    <td data-label="Attempts">{job.attempts}/{job.max_attempts}</td>
                    <td data-label="Run at">{formatDate(job.run_at)}</td>
                    <td data-label="Action">{job.status === 'DEAD' ? <form action={async () => { 'use server'; await retryDeadJobAction(job.id) }}><button className="small" type="submit">Retry</button></form> : <span className="muted">—</span>}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </section>
    </main>
  )
}
