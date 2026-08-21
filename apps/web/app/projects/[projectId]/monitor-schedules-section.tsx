import { getMonitorSchedules } from '../../../lib/monitor-scheduling-api'
import { saveMonitorScheduleAction } from './monitor-schedule-actions'

function formatDate(value: string | null): string {
  return value ? new Date(value).toLocaleString() : '—'
}

function formatInterval(seconds: number): string {
  if (seconds % 3600 === 0) return `${seconds / 3600}h`
  if (seconds % 60 === 0) return `${seconds / 60}m`
  return `${seconds}s`
}

export default async function MonitorSchedulesSection({ projectId }: { projectId: string }) {
  const schedules = await getMonitorSchedules(projectId)

  return (
    <section>
      <h2>Monitoring schedules</h2>
      <p>Automatic site checks use the same SSRF-safe monitor probe as manual checks and run through the Argus worker.</p>

      <h3>Schedules</h3>
      {schedules.length === 0 ? <div className="empty-state"><strong>No active sites</strong>Monitoring schedules appear after sites are configured.</div> : (
        <div className="resource-list" style={{ margin: '0 17px 17px' }}>
          {schedules.map((schedule) => (
            <article className="resource-card" key={schedule.site_id}>
              <div className="resource-card-head">
                <div><h4>{schedule.site_name}</h4><div className="resource-meta">{schedule.has_monitor_config ? 'Monitor configured' : 'Configure site monitoring first'}</div></div>
                <span className={`badge ${schedule.enabled ? 'success' : ''}`}>{schedule.enabled ? 'Scheduled' : 'Manual only'}</span>
              </div>
              {schedule.schedule_id ? <div className="info-grid" style={{ marginTop: 12 }}><div className="info-item"><span className="info-label">Interval</span><span className="info-value">{formatInterval(schedule.interval_seconds)}</span></div><div className="info-item"><span className="info-label">Next run</span><span className="info-value">{formatDate(schedule.next_run_at)}</span></div><div className="info-item"><span className="info-label">Last queued</span><span className="info-value">{formatDate(schedule.last_enqueued_at)}</span></div></div> : null}
              {schedule.has_monitor_config ? (
                <details className="resource-editor">
                  <summary className="button small">Configure schedule</summary>
                  <div className="resource-editor-body">
                    <form action={async (formData) => { 'use server'; await saveMonitorScheduleAction(projectId, schedule.site_id, formData) }}>
                      <div className="form-grid">
                        <label style={{ display: 'flex', gridTemplateColumns: 'auto 1fr', alignItems: 'center' }}><input name="enabled" type="checkbox" defaultChecked={schedule.enabled} /> Enable automatic checks</label>
                        <label>Interval seconds<input name="interval_seconds" type="number" min={60} max={86400} step={60} defaultValue={schedule.interval_seconds} required /></label>
                      </div>
                      <button type="submit">Save schedule</button>
                    </form>
                  </div>
                </details>
              ) : null}
            </article>
          ))}
        </div>
      )}
    </section>
  )
}
