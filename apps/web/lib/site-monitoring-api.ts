export type SiteMonitorConfig = {
  id: string
  site_id: string
  target_url: string
  check_robots: boolean
  check_sitemap: boolean
  timeout_seconds: number
  created_at: string
  updated_at: string
}

export type SiteMonitorCheck = {
  id: string
  site_id: string
  config_id: string
  overall_status: 'HEALTHY' | 'DEGRADED' | 'DOWN' | 'ERROR'
  target_url: string
  resolved_ips: string[]
  dns_ok: boolean
  http_status: number | null
  http_latency_ms: number | null
  tls_status: string
  robots_status: number | null
  sitemap_status: number | null
  error_code: string | null
  error_message: string | null
  checked_by: string
  checked_at: string
}

export type SiteMonitorView = {
  site_id: string
  config: SiteMonitorConfig | null
  checks: SiteMonitorCheck[]
}

export type ProjectMonitoringView = {
  monitors: SiteMonitorView[]
}

export type SaveMonitorConfigInput = {
  target_url: string
  check_robots: boolean
  check_sitemap: boolean
  timeout_seconds: number
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

export const getProjectMonitoringView = (projectId: string): Promise<ProjectMonitoringView> =>
  request(`/projects/${projectId}/site-monitoring`)

export async function saveSiteMonitorConfig(
  projectId: string,
  siteId: string,
  input: SaveMonitorConfigInput,
): Promise<SiteMonitorConfig> {
  return request(`/projects/${projectId}/sites/${siteId}/monitor`, {
    method: 'PUT',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(input),
  })
}

export async function runSiteMonitorCheck(
  projectId: string,
  siteId: string,
): Promise<SiteMonitorCheck> {
  return request(`/projects/${projectId}/sites/${siteId}/monitor/check`, {
    method: 'POST',
  })
}
