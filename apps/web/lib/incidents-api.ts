export type IncidentSeverity = 'MINOR' | 'MAJOR' | 'CRITICAL'
export type IncidentStatus = 'INVESTIGATING' | 'IDENTIFIED' | 'MONITORING' | 'RESOLVED'

export type IncidentSummary = {
  id: string
  project_id: string
  title: string
  summary: string
  severity: IncidentSeverity
  status: IncidentStatus
  source_type: string
  source_id: string
  source_name: string
  affected_count: number
  created_by: string
  started_at: string
  resolved_at: string | null
  created_at: string
  updated_at: string
}

export type IncidentAffectedResource = {
  id: string
  resource_type: string
  resource_id: string
  resource_name: string
  distance: number
  impact_path: Array<{ resource_type: string; resource_id: string; name: string }>
  created_at: string
}

export type IncidentTimelineEvent = {
  id: string
  event_type: 'CREATED' | 'STATUS_CHANGED' | 'NOTE'
  message: string
  data: Record<string, unknown>
  created_by: string
  created_at: string
}

export type IncidentDetail = {
  incident: IncidentSummary
  affected: IncidentAffectedResource[]
  timeline: IncidentTimelineEvent[]
}

export type CreateIncidentInput = {
  title: string
  summary?: string
  severity: IncidentSeverity
  source_type: string
  source_id: string
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
  if (!response.ok) {
    throw new Error(`Control API ${response.status}: ${await response.text()}`)
  }
  return response.status === 204 ? (undefined as T) : response.json()
}

export const getIncidents = (projectId: string): Promise<IncidentSummary[]> =>
  request(`/projects/${projectId}/incidents`)

export const getIncident = (projectId: string, incidentId: string): Promise<IncidentDetail> =>
  request(`/projects/${projectId}/incidents/${incidentId}`)

export async function createIncident(
  projectId: string,
  input: CreateIncidentInput,
): Promise<IncidentDetail> {
  return request(`/projects/${projectId}/incidents`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(input),
  })
}

export async function updateIncidentStatus(
  projectId: string,
  incidentId: string,
  status: IncidentStatus,
): Promise<IncidentDetail> {
  return request(`/projects/${projectId}/incidents/${incidentId}/status`, {
    method: 'PUT',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ status }),
  })
}

export async function addIncidentNote(
  projectId: string,
  incidentId: string,
  message: string,
): Promise<IncidentDetail> {
  return request(`/projects/${projectId}/incidents/${incidentId}/notes`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ message }),
  })
}
