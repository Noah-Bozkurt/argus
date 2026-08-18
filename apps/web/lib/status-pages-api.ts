export type StatusPageComponent = {
  id: string
  resource_type: 'SITE' | 'SERVICE'
  resource_id: string
  display_name: string
  internal_status: string
  public_status: string
  sort_order: number
}

export type StatusIncidentPublication = {
  id: string
  incident_id: string
  incident_status: string
  incident_severity: string
  public_title: string
  public_message: string
  is_published: boolean
  published_at: string
  updated_at: string
}

export type StatusPageView = {
  id: string
  project_id: string
  name: string
  slug: string
  visibility: 'INTERNAL' | 'PUBLIC'
  overall_status: string
  components: StatusPageComponent[]
  incident_publications: StatusIncidentPublication[]
  created_at: string
  updated_at: string
}

export type PublicStatusPage = {
  name: string
  overall_status: string
  components: Array<{ name: string; status: string }>
  incidents: Array<{
    title: string
    message: string
    status: string
    started_at: string
    resolved_at: string | null
    updated_at: string
  }>
  updated_at: string
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

export const getStatusPages = (projectId: string): Promise<StatusPageView[]> =>
  request(`/projects/${projectId}/status-pages`)

export async function createStatusPage(projectId: string, name: string, slug: string): Promise<StatusPageView> {
  return request(`/projects/${projectId}/status-pages`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ name, slug }),
  })
}

export async function updateStatusPage(
  projectId: string,
  pageId: string,
  name: string,
  slug: string,
  visibility: 'INTERNAL' | 'PUBLIC',
): Promise<StatusPageView> {
  return request(`/projects/${projectId}/status-pages/${pageId}`, {
    method: 'PUT',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ name, slug, visibility }),
  })
}

export async function deleteStatusPage(projectId: string, pageId: string): Promise<void> {
  await request(`/projects/${projectId}/status-pages/${pageId}`, { method: 'DELETE' })
}

export async function addStatusPageComponent(
  projectId: string,
  pageId: string,
  resourceType: 'SITE' | 'SERVICE',
  resourceId: string,
  displayName: string,
): Promise<StatusPageView> {
  return request(`/projects/${projectId}/status-pages/${pageId}/components`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({
      resource_type: resourceType,
      resource_id: resourceId,
      display_name: displayName,
      sort_order: 100,
    }),
  })
}

export async function removeStatusPageComponent(
  projectId: string,
  pageId: string,
  componentId: string,
): Promise<StatusPageView> {
  return request(`/projects/${projectId}/status-pages/${pageId}/components/${componentId}`, {
    method: 'DELETE',
  })
}

export async function updateStatusIncidentPublication(
  projectId: string,
  pageId: string,
  incidentId: string,
  publicTitle: string,
  publicMessage: string,
  isPublished: boolean,
): Promise<StatusPageView> {
  return request(`/projects/${projectId}/status-pages/${pageId}/incidents`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({
      incident_id: incidentId,
      public_title: publicTitle,
      public_message: publicMessage,
      is_published: isPublished,
    }),
  })
}

export async function getPublicStatusPage(slug: string): Promise<PublicStatusPage> {
  const response = await fetch(`${controlApi}/public/status/${encodeURIComponent(slug)}`, {
    cache: 'no-store',
  })
  if (!response.ok) throw new Error(`Public status API ${response.status}`)
  return response.json()
}
