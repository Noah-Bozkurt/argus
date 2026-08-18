export type ReadinessCheckStatus = 'PASS' | 'WARN' | 'BLOCKED' | 'SKIPPED'
export type ReadinessStatus = 'READY' | 'ATTENTION' | 'BLOCKED'

export type ReadinessCheck = {
  key: string
  category: string
  label: string
  status: ReadinessCheckStatus
  summary: string
  evidence: string[]
}

export type ReadinessAssessment = {
  project_id: string
  status: ReadinessStatus
  checked_at: string
  checks: ReadinessCheck[]
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

export async function getProjectReadiness(projectId: string): Promise<ReadinessAssessment> {
  const response = await fetch(`${controlApi}/projects/${projectId}/readiness`, {
    cache: 'no-store',
    headers: authHeaders(),
  })
  if (!response.ok) {
    throw new Error(`Control API ${response.status}: ${await response.text()}`)
  }
  return response.json()
}
