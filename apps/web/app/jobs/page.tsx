import Link from 'next/link'
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

export default async function JobsPage() {
  const view = await getJobsAdminView()

  return (
    <main>
      <p><Link href="/">← Dashboard</Link></p>
      <h1>Background Jobs</h1>
      <p>
        Queue and schedule visibility for Argus background work. Schedule configuration remains in
        the feature that owns it; this page does not edit typed payloads or bypass feature validation.
      </p>

      <h2>Queue</h2>
      <p>
        Queued: <strong>{view.queued_count}</strong>
        {' — '}Running: <strong>{view.running_count}</strong>
        {' — '}Dead: <strong>{view.dead_count}</strong>
      </p>

      <h2>Schedules</h2>
      {view.schedules.length === 0 ? <p>No schedules configured.</p> : (
        <ul>
          {view.schedules.map((schedule) => (
            <li key={schedule.id}>
              <p>
                <strong>{schedule.job_kind}</strong>
                {' — '}{schedule.project_name ?? 'Workspace'}
                {' — '}{schedule.enabled ? 'Enabled' : 'Disabled'}
                {' — '}every {formatInterval(schedule.interval_seconds)}
              </p>
              <p>
                Resource: <code>{schedule.resource_key || 'default'}</code>
                {' — '}Next: {formatDate(schedule.next_run_at)}
                {' — '}Last queued: {formatDate(schedule.last_enqueued_at)}
              </p>
            </li>
          ))}
        </ul>
      )}

      <h2>Recent jobs</h2>
      <p>Showing the latest {view.jobs.length} jobs. Job payloads are intentionally not rendered.</p>
      {view.jobs.length === 0 ? <p>No background jobs yet.</p> : (
        <ul>
          {view.jobs.map((job) => (
            <li key={job.id}>
              <p>
                <strong>{job.job_kind}</strong>
                {' — '}{job.project_name ?? 'Workspace'}
                {' — '}{job.status}
                {' — '}attempt {job.attempts}/{job.max_attempts}
              </p>
              <p>
                Resource: <code>{job.resource_key || 'default'}</code>
                {' — '}Run at: {formatDate(job.run_at)}
                {' — '}Created: {formatDate(job.created_at)}
              </p>
              {job.status === 'RUNNING' ? (
                <p>Lease: {job.lease_owner ?? 'unknown'} until {formatDate(job.lease_expires_at)}</p>
              ) : null}
              {job.last_error_code || job.last_error_message ? (
                <p>
                  Error: {job.last_error_code ?? 'EXECUTION_FAILED'}
                  {job.last_error_message ? ` — ${job.last_error_message}` : ''}
                </p>
              ) : null}
              {job.status === 'DEAD' ? (
                <form action={async () => { 'use server'; await retryDeadJobAction(job.id) }}>
                  <button type="submit">Retry dead job</button>
                </form>
              ) : null}
            </li>
          ))}
        </ul>
      )}
    </main>
  )
}
