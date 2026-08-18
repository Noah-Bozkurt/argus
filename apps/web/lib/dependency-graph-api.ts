export type ResourceNode = {
  resource_type: 'SERVICE' | 'SITE' | 'DOMAIN' | 'SERVER' | 'ENVIRONMENT' | 'REPOSITORY' | string
  resource_id: string
  name: string
  status: string | null
}

export type DependencyEdge = {
  id: string | null
  source_type: string
  source_id: string
  target_type: string
  target_id: string
  relationship: string
  origin: 'DERIVED' | 'MANUAL' | string
}

export type DependencyGraph = {
  nodes: ResourceNode[]
  edges: DependencyEdge[]
}

export type ResourceRef = {
  resource_type: string
  resource_id: string
  name: string
}

export type ImpactedResource = {
  resource: ResourceRef
  distance: number
  path: ResourceRef[]
}

export type ImpactView = {
  root: ResourceRef
  affected_count: number
  affected: ImpactedResource[]
}

export type CreateDependencyInput = {
  source_type: string
  source_id: string
  target_type: string
  target_id: string
  relationship: 'DEPENDS_ON' | 'USES'
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

export const getDependencyGraph = (projectId: string): Promise<DependencyGraph> =>
  request(`/projects/${projectId}/dependency-graph`)

export async function createDependency(
  projectId: string,
  input: CreateDependencyInput,
): Promise<DependencyEdge> {
  return request(`/projects/${projectId}/dependencies`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(input),
  })
}

export async function deleteDependency(projectId: string, dependencyId: string): Promise<void> {
  await request(`/projects/${projectId}/dependencies/${dependencyId}`, { method: 'DELETE' })
}

export const getDependencyImpact = (
  projectId: string,
  resourceType: string,
  resourceId: string,
): Promise<ImpactView> =>
  request(`/projects/${projectId}/dependency-impact/${encodeURIComponent(resourceType)}/${resourceId}`)
