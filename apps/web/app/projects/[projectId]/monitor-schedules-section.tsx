import { getMonitorSchedules } from '../../../lib/monitor-scheduling-api'
import { saveMonitorScheduleAction } from './monitor-schedule-actions'

function formatDate(value: string | null): string {
  return value ? new Date(value).toLocaleString() : '—'
}

export default async function MonitorSchedulesSection({ projectId }: { projectId: string }) {
  const schedules = await getMonitorSchedules(projectId)

  return (
    <section>
      <h2>Monitoring schedules</h2>
      <p>
        Run configured Site Monitoring checks automatically through the Argus worker. The same
        SSRF-safe monitor probe is used for manual and scheduled checks.
      </p>
      {schedules.length === 0 ? <p>No active sites.</p> : (
        <ul>
          {schedules.map((schedule) => (
            <li key={schedule.site_id}>
              <p>
                <strong>{schedule.site_name}</strong>
                {' — '}{schedule.has_monitor_config ? 'monitor configured' : 'configure monitoring first'}
                {' — '}{schedule.enabled ? 'scheduled' : 'manual only'}
              </p>
              {schedule.has_monitor_config ? (
                <form
                  action={async (formData) => {
                    'use server'
                    await saveMonitorScheduleAction(projectId, schedule.site_id, formData)
                  }}
                >
                  <label>
                    <input name="enabled" type="checkbox" defaultChecked={schedule.enabled} />
                    Enable automatic checks
                  </label>
                  <label>
                    Interval seconds
                    <input
                      name="interval_seconds"
                      type="number"
                      min={60}
                      max={86400}
                      step={60}
                      defaultValue={schedule.interval_seconds}
                      required
                    />
                  </label>
                  <button type="submit">Save schedule</button>
                </form>
              ) : null}
              {schedule.schedule_id ? (
                <p>
                  Next: {formatDate(schedule.next_run_at)} — Last queued: {formatDate(schedule.last_enqueued_at)}
                </p>
              ) : null}
            </li>
          ))}
        </ul>
      )}
    </section>
  )
}
