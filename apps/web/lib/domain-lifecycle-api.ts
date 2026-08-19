export type DomainLifecycleStatus = {
  domain_id: string
  hostname: string
  expires_at: string | null
  expiration_status: 'UNKNOWN' | 'OK' | 'WARNING' | 'CRITICAL' | 'EXPIRED'
  tls_status: 'UNKNOWN' | 'VALID' | 'FAILED' | 'STALE'
  overall_status: 'OK' | 'ATTENTION' | 'CRITICAL' | 'UNKNOWN'
  days_until_expiry: number | null
  last_evaluated_at: string | null
  changed_at: string | null
}

export type DomainLifecycleEvaluation = {
  evaluated_domains: number
  changed_domains: number
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
  return response.json()
}

export const getProjectDomainLifecycle = (projectId: string): Promise<DomainLifecycleStatus[]> =>
  request(`/projects/${projectId}/domain-lifecycle`)

export const evaluateProjectDomainLifecycle = (
  projectId: string,
): Promise<DomainLifecycleEvaluation> =>
  request(`/projects/${projectId}/domain-lifecycle/evaluate`, { method: 'POST' })
