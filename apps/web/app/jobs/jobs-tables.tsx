'use client'

import { useMemo, useState } from 'react'
import type { BackgroundJobView, JobScheduleView } from '../../lib/jobs-admin-api'
import usePersistentChoice from '../use-persistent-choice'
import { retryDeadJobAction } from './actions'

const STATUS_FILTERS = ['all', 'QUEUED', 'RUNNING', 'SUCCEEDED', 'DEAD'] as const
const SORTS = ['recent', 'attempts', 'project', 'kind'] as const

type StatusFilter = typeof STATUS_FILTERS[number]
type SortChoice = typeof SORTS[number]

function formatDate(value: string | null): string {
  return value ? new Date(value).toLocaleString() : '—'
}

function relativeTime(value: string | null): string {
  if (!value) return '—'
  const delta = Date.now() - new Date(value).getTime()
  const minutes = Math.max(0, Math.floor(delta / 60000))
  if (minutes < 60) return minutes < 1 ? 'just now' : `${minutes}m ago`
  const hours = Math.floor(minutes / 60)
  if (hours < 24) return `${hours}h ago`
  return `${Math.floor(hours / 24)}d ago`
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

export default function JobsTables({ jobs, schedules }: { jobs: BackgroundJobView[]; schedules: JobScheduleView[] }) {
  const [query, setQuery] = useState('')
  const [status, setStatus] = usePersistentChoice<StatusFilter>('argus:jobs:status', 'all', STATUS_FILTERS)
  const [sort, setSort] = usePersistentChoice<SortChoice>('argus:jobs:sort', 'recent', SORTS)

  const normalized = query.trim().toLowerCase()
  const visibleJobs = useMemo(() => jobs
    .filter((job) => status === 'all' || job.status === status)
    .filter((job) => !normalized || [job.job_kind, job.project_name ?? '', job.resource_key, job.status, job.last_error_code ?? '', job.last_error_message ?? ''].join(' ').toLowerCase().includes(normalized))
    .sort((left, right) => {
      if (sort === 'attempts') return right.attempts - left.attempts || Date.parse(right.updated_at) - Date.parse(left.updated_at)
      if (sort === 'project') return (left.project_name ?? 'Workspace').localeCompare(right.project_name ?? 'Workspace') || left.job_kind.localeCompare(right.job_kind)
      if (sort === 'kind') return left.job_kind.localeCompare(right.job_kind) || Date.parse(right.updated_at) - Date.parse(left.updated_at)
      return Date.parse(right.updated_at) - Date.parse(left.updated_at)
    }), [jobs, normalized, sort, status])

  const visibleSchedules = useMemo(() => schedules
    .filter((schedule) => !normalized || [schedule.job_kind, schedule.project_name ?? '', schedule.resource_key, schedule.enabled ? 'enabled' : 'disabled'].join(' ').toLowerCase().includes(normalized))
    .sort((left, right) => Number(right.enabled) - Number(left.enabled) || Date.parse(left.next_run_at) - Date.parse(right.next_run_at)), [normalized, schedules])

  return (
    <>
      <section className="panel" style={{ marginBottom: 16 }}>
        <div className="panel-header resource-toolbar-header">
          <div><h2>Schedules</h2><p>{visibleSchedules.length} of {schedules.length} recurring jobs</p></div>
          <div className="resource-toolbar"><input type="search" value={query} onChange={(event) => setQuery(event.target.value)} placeholder="Search jobs…" aria-label="Search jobs and schedules" /></div>
        </div>
        {visibleSchedules.length === 0 ? <div className="empty-state"><strong>No matching schedules</strong>Feature-owned schedules will appear here when configured.</div> : (
          <div className="table-wrap" style={{ border: 0, borderRadius: 0 }}>
            <table className="responsive-table">
              <thead><tr><th>Job</th><th>Scope</th><th>Resource</th><th>Interval</th><th>Status</th><th>Next run</th></tr></thead>
              <tbody>{visibleSchedules.map((schedule) => (
                <tr key={schedule.id}>
                  <td><strong>{schedule.job_kind}</strong></td>
                  <td data-label="Scope">{schedule.project_name ?? 'Workspace'}</td>
                  <td data-label="Resource"><code>{schedule.resource_key || 'default'}</code></td>
                  <td data-label="Interval">{formatInterval(schedule.interval_seconds)}</td>
                  <td data-label="Status"><span className={`badge ${schedule.enabled ? 'success' : ''}`}>{schedule.enabled ? 'Enabled' : 'Disabled'}</span></td>
                  <td data-label="Next run" title={formatDate(schedule.next_run_at)}>{schedule.enabled ? relativeTime(schedule.next_run_at) : 'Paused'}</td>
                </tr>
              ))}</tbody>
            </table>
          </div>
        )}
      </section>

      <section className="panel">
        <div className="panel-header resource-toolbar-header">
          <div><h2>Recent jobs</h2><p>{visibleJobs.length} of {jobs.length} executions · payloads stay hidden by design</p></div>
          <div className="resource-toolbar">
            <select value={status} onChange={(event) => setStatus(event.target.value as StatusFilter)} aria-label="Filter job status"><option value="all">All statuses</option><option value="QUEUED">Queued</option><option value="RUNNING">Running</option><option value="SUCCEEDED">Succeeded</option><option value="DEAD">Dead</option></select>
            <select value={sort} onChange={(event) => setSort(event.target.value as SortChoice)} aria-label="Sort jobs"><option value="recent">Recently updated</option><option value="attempts">Attempts</option><option value="project">Project</option><option value="kind">Job kind</option></select>
          </div>
        </div>
        {visibleJobs.length === 0 ? <div className="empty-state"><strong>No matching background jobs</strong>Change the search or status filter to show other executions.</div> : (
          <div className="table-wrap" style={{ border: 0, borderRadius: 0 }}>
            <table className="responsive-table">
              <thead><tr><th>Job</th><th>Project</th><th>Resource</th><th>Status</th><th>Attempts</th><th>Run at</th><th>Action</th></tr></thead>
              <tbody>{visibleJobs.map((job) => (
                <tr key={job.id}>
                  <td><strong>{job.job_kind}</strong>{job.last_error_message ? <div className="text-danger" style={{ marginTop: 4 }}>{job.last_error_code ?? 'EXECUTION_FAILED'} · {job.last_error_message}</div> : null}</td>
                  <td data-label="Project">{job.project_name ?? 'Workspace'}</td>
                  <td data-label="Resource"><code>{job.resource_key || 'default'}</code></td>
                  <td data-label="Status"><span className={`badge ${statusClass(job.status)}`}>{job.status}</span></td>
                  <td data-label="Attempts">{job.attempts}/{job.max_attempts}</td>
                  <td data-label="Run at" title={formatDate(job.run_at)}>{relativeTime(job.run_at)}</td>
                  <td data-label="Action">{job.status === 'DEAD' ? <form action={async () => { await retryDeadJobAction(job.id) }}><button className="small" type="submit">Retry</button></form> : <span className="muted">—</span>}</td>
                </tr>
              ))}</tbody>
            </table>
          </div>
        )}
      </section>
    </>
  )
}
