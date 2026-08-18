export type CorrelatedChange = {
  category: 'DEPLOYMENT' | 'RELEASE' | 'SERVER_COMMAND' | 'PROJECT_CHANGE' | string
  event_type: string
  occurred_at: string
  minutes_from_incident: number
  impact_related: boolean
  resource_type: string | null
  resource_id: string | null
  summary: string
}

export type CorrelationView = {
  incident_id: string
  incident_started_at: string
  window_minutes: number
  changes: CorrelatedChange[]
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

export async function getIncidentCorrelation(
  projectId: string,
  incidentId: string,
): Promise<CorrelationView> {
  const response = await fetch(
    `${controlApi}/projects/${projectId}/incidents/${incidentId}/correlation`,
    { cache: 'no-store', headers: authHeaders() },
  )
  if (!response.ok) {
    throw new Error(`Control API ${response.status}: ${await response.text()}`)
  }
  return response.json()
}
