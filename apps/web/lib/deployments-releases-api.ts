export type DeploymentStatus =
  | 'QUEUED'
  | 'RUNNING'
  | 'SUCCEEDED'
  | 'FAILED'
  | 'CANCELLED'
  | 'ROLLED_BACK'

export type Deployment = {
  id: string
  project_id: string
  service_id: string
  environment_id: string
  repository_id: string | null
  source_commit_sha: string | null
  source_version: string | null
  provider: string
  status: DeploymentStatus
  deployment_url: string | null
  error_summary: string | null
  notes: string
  previous_deployment_id: string | null
  rollback_of_deployment_id: string | null
  triggered_by: string
  started_at: string | null
  finished_at: string | null
  created_at: string
  updated_at: string
}

export type ReleaseStatus = 'DRAFT' | 'READY' | 'RELEASED' | 'FAILED' | 'ROLLED_BACK'

export type ReleaseComponent = {
  id: string
  release_id: string
  service_id: string
  deployment_id: string | null
  version: string | null
  commit_sha: string | null
  created_at: string
}

export type ProjectRelease = {
  id: string
  project_id: string
  version: string
  name: string
  notes: string
  status: ReleaseStatus
  created_by: string
  released_at: string | null
  created_at: string
  updated_at: string
  components: ReleaseComponent[]
}

export type DeploymentReleaseView = {
  deployments: Deployment[]
  releases: ProjectRelease[]
}

export type CreateDeploymentInput = {
  service_id: string
  environment_id: string
  repository_id?: string | null
  source_commit_sha?: string | null
  source_version?: string | null
  provider?: string
  notes?: string
  rollback_of_deployment_id?: string | null
}

export type CreateReleaseInput = {
  version: string
  name: string
  notes?: string
}

export type AddReleaseComponentInput = {
  service_id: string
  deployment_id?: string | null
  version?: string | null
  commit_sha?: string | null
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

export const getDeploymentReleaseView = (projectId: string): Promise<DeploymentReleaseView> =>
  request(`/projects/${projectId}/deployments-releases`)

export async function createDeployment(
  projectId: string,
  input: CreateDeploymentInput,
): Promise<Deployment> {
  return request(`/projects/${projectId}/deployments`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(input),
  })
}

export async function updateDeploymentStatus(
  projectId: string,
  deploymentId: string,
  status: DeploymentStatus,
  deploymentUrl: string | null,
  errorSummary: string | null,
): Promise<Deployment> {
  return request(`/projects/${projectId}/deployments/${deploymentId}/status`, {
    method: 'PUT',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({
      status,
      deployment_url: deploymentUrl,
      error_summary: errorSummary,
    }),
  })
}

export async function createRelease(
  projectId: string,
  input: CreateReleaseInput,
): Promise<ProjectRelease> {
  return request(`/projects/${projectId}/releases`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(input),
  })
}

export async function addReleaseComponent(
  projectId: string,
  releaseId: string,
  input: AddReleaseComponentInput,
): Promise<ProjectRelease> {
  return request(`/projects/${projectId}/releases/${releaseId}/components`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(input),
  })
}

export async function updateReleaseStatus(
  projectId: string,
  releaseId: string,
  status: ReleaseStatus,
): Promise<ProjectRelease> {
  return request(`/projects/${projectId}/releases/${releaseId}/status`, {
    method: 'PUT',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ status }),
  })
}
