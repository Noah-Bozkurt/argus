export type ComposeStack = {
  id: string
  project_id: string
  environment_id: string
  environment_name: string
  server_id: string
  server_hostname: string
  name: string
  compose_project_name: string
  description: string
  lifecycle_status: 'ACTIVE' | 'PAUSED' | 'ARCHIVED'
  created_at: string
  updated_at: string
}

export type ComposeStackAction = 'start' | 'stop' | 'restart'

export type CreateComposeStackInput = {
  server_id: string
  name: string
  compose_project_name: string
  description?: string
}

export type UpdateComposeStackInput = {
  name: string
  description?: string
  lifecycle_status: ComposeStack['lifecycle_status']
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

export const getProjectComposeStacks = (projectId: string): Promise<ComposeStack[]> =>
  request(`/projects/${projectId}/stacks`)

export const createProjectComposeStack = (
  projectId: string,
  input: CreateComposeStackInput,
): Promise<ComposeStack> =>
  request(`/projects/${projectId}/stacks`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(input),
  })

export const updateProjectComposeStack = (
  projectId: string,
  stackId: string,
  input: UpdateComposeStackInput,
): Promise<ComposeStack> =>
  request(`/projects/${projectId}/stacks/${stackId}`, {
    method: 'PUT',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(input),
  })

export async function deleteProjectComposeStack(projectId: string, stackId: string): Promise<void> {
  await request(`/projects/${projectId}/stacks/${stackId}`, { method: 'DELETE' })
}

export async function runProjectComposeStackAction(
  projectId: string,
  stackId: string,
  action: ComposeStackAction,
): Promise<void> {
  await request<unknown>(`/projects/${projectId}/stacks/${stackId}/actions/${action}`, {
    method: 'POST',
  })
}
