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

const controlApi = process.env.ARGUS_CONTROL_API_URL ?? 'http://localhost:8080'

function authHeaders(): Record<string, string> {
  const token = process.env.ARGUS_WEB_API_TOKEN
  const organizationId = process.env.ARGUS_ORG_ID
  const userId = process.env.ARGUS_USER_ID
  if (!token || !organizationId || !userId) {
    throw new Error('ARGUS_WEB_API_TOKEN, ARGUS_ORG_ID and ARGUS_USER_ID are required')
  }
  return {
    authorization: `Bearer ${token}`,
    'x-argus-org-id': organizationId,
    'x-argus-user-id': userId,
  }
}

async function request<T>(path: string, init: RequestInit = {}): Promise<T> {
  const response = await fetch(`${controlApi}${path}`, {
    ...init,
    cache: 'no-store',
    headers: { ...authHeaders(), ...(init.headers ?? {}) },
  })
  if (!response.ok) throw new Error(`Control API ${response.status}: ${await response.text()}`)
  return response.status === 204 ? (undefined as T) : response.json()
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
