export type BackgroundJobView = {
  id: string
  project_id: string | null
  project_name: string | null
  job_kind: string
  resource_key: string
  status: 'QUEUED' | 'RUNNING' | 'SUCCEEDED' | 'DEAD'
  run_at: string
  attempts: number
  max_attempts: number
  lease_owner: string | null
  lease_expires_at: string | null
  last_error_code: string | null
  last_error_message: string | null
  created_at: string
  updated_at: string
  completed_at: string | null
}

export type JobScheduleView = {
  id: string
  project_id: string | null
  project_name: string | null
  job_kind: string
  resource_key: string
  interval_seconds: number
  max_attempts: number
  enabled: boolean
  next_run_at: string
  last_enqueued_at: string | null
  updated_at: string
}

export type JobsAdminView = {
  queued_count: number
  running_count: number
  dead_count: number
  jobs: BackgroundJobView[]
  schedules: JobScheduleView[]
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

export const getJobsAdminView = (): Promise<JobsAdminView> => request('/background-jobs')

export const retryDeadJob = (jobId: string): Promise<void> =>
  request(`/background-jobs/${jobId}/retry`, { method: 'POST' })
