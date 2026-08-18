import { request } from './api'

export type MonitorSchedule = {
  site_id: string
  site_name: string
  has_monitor_config: boolean
  schedule_id: string | null
  enabled: boolean
  interval_seconds: number
  next_run_at: string | null
  last_enqueued_at: string | null
  actor_user_id: string | null
}

export const getMonitorSchedules = (projectId: string): Promise<MonitorSchedule[]> =>
  request(`/projects/${projectId}/site-monitoring/schedules`)

export const saveMonitorSchedule = (
  projectId: string,
  siteId: string,
  enabled: boolean,
  intervalSeconds: number,
): Promise<MonitorSchedule> =>
  request(`/projects/${projectId}/sites/${siteId}/monitor/schedule`, {
    method: 'PUT',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ enabled, interval_seconds: intervalSeconds }),
  })
