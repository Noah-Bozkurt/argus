export type SiteLifecycleStatus = 'ACTIVE' | 'PAUSED' | 'ARCHIVED'
export type DomainRoutingMode = 'DIRECT' | 'CLOUDFLARE_PROXY' | 'CLOUDFLARE_TUNNEL'

export type ProjectSite = {
  id: string
  project_id: string
  service_id: string | null
  repository_id: string | null
  environment_id: string | null
  name: string
  description: string
  framework: string | null
  canonical_url: string | null
  lifecycle_status: SiteLifecycleStatus
  health_status: string
  created_at: string
  updated_at: string
}

export type ProjectDomain = {
  id: string
  project_id: string
  site_id: string | null
  hostname: string
  registrar: string | null
  dns_provider: string | null
  routing_mode: DomainRoutingMode
  is_primary: boolean
  expires_at: string | null
  tls_status: string
  created_at: string
  updated_at: string
}

export type SiteDomainView = {
  sites: ProjectSite[]
  domains: ProjectDomain[]
}

export type CreateSiteInput = {
  name: string
  description?: string
  service_id?: string | null
  repository_id?: string | null
  environment_id?: string | null
  framework?: string | null
  canonical_url?: string | null
}

export type UpdateSiteInput = CreateSiteInput & {
  lifecycle_status: SiteLifecycleStatus
}

export type DomainInput = {
  site_id?: string | null
  hostname: string
  registrar?: string | null
  dns_provider?: string | null
  routing_mode: DomainRoutingMode
  is_primary?: boolean
  expires_at?: string | null
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

export const getSiteDomainView = (projectId: string): Promise<SiteDomainView> =>
  request(`/projects/${projectId}/sites-domains`)

export async function createSite(projectId: string, input: CreateSiteInput): Promise<ProjectSite> {
  return request(`/projects/${projectId}/sites`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(input),
  })
}

export async function updateSite(
  projectId: string,
  siteId: string,
  input: UpdateSiteInput,
): Promise<ProjectSite> {
  return request(`/projects/${projectId}/sites/${siteId}`, {
    method: 'PUT',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(input),
  })
}

export async function deleteSite(projectId: string, siteId: string): Promise<void> {
  await request(`/projects/${projectId}/sites/${siteId}`, { method: 'DELETE' })
}

export async function createDomain(projectId: string, input: DomainInput): Promise<ProjectDomain> {
  return request(`/projects/${projectId}/domains`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(input),
  })
}

export async function updateDomain(
  projectId: string,
  domainId: string,
  input: DomainInput,
): Promise<ProjectDomain> {
  return request(`/projects/${projectId}/domains/${domainId}`, {
    method: 'PUT',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(input),
  })
}

export async function deleteDomain(projectId: string, domainId: string): Promise<void> {
  await request(`/projects/${projectId}/domains/${domainId}`, { method: 'DELETE' })
}
