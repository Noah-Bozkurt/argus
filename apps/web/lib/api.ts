import { randomUUID } from 'node:crypto'
import { currentSessionToken, getWorkspaceUser } from './auth'

export type SystemSnapshot = {
  server_id: string
  hostname: string
  os: string
  kernel: string
  architecture: string
  cpu_percent: number
  ram_percent: number
  disk_percent: number
  load: number
  uptime_seconds: number
  agent_version: string
  updates: { supported: boolean; pending_updates: number; reboot_required: boolean; packages: Array<{ name: string; installed_version: string; candidate_version: string; security: boolean }> }
  diagnostics: { failed_units: string[]; listening_tcp_ports: number[]; journals: Array<{ service: string; output: string }> }
  docker: { available: boolean; containers: Array<{ id: string; name: string; image: string; state: string; status: string; ports: string }> }
  security: { available: boolean; firewall_status: string; firewall_rules: string[]; ssh_password_auth: boolean | null; ssh_root_login: string; automatic_security_updates: boolean; findings: Array<{ severity: string; code: string; message: string }> }
  backups: {
    available: boolean
    target: string
    artifacts: Array<{ name: string; profile: string; size_bytes: number; created_unix: number; sha256: string; verified: boolean }>
  }
  mounts: Array<{ name: string; mount_point: string; file_system: string; total_bytes: number; available_bytes: number }>
  network: Array<{ name: string; received_bytes: number; transmitted_bytes: number; receive_errors: number; transmit_errors: number }>
  top_processes: Array<{ pid: number; name: string; cpu_percent: number; memory_bytes: number }>
  captured_at: string
}

export type ServerView = { server_id: string; project_id: string; environment_id: string; hostname: string; online: boolean; last_heartbeat: string | null; snapshot: SystemSnapshot | null; services: Array<{ name: string; status: string }>; capabilities: Array<{ name: string; version: string }> }
export type CommandHistoryItem = { command: { id: string; server_id: string; command_type: { kind: string; service?: string; container?: string; profile?: string; backup?: string; version?: string }; created_at: string; expires_at: string; status: string; idempotency_key: string; risk_level: string }; started_at: string | null; finished_at: string | null; error_code: string | null; error_message: string | null; actor_user_id: string | null; phase: string | null; output: string | null; output_truncated: boolean }
export type MetricSample = { captured_at: string; cpu_percent: number; ram_percent: number; disk_percent: number; load: number }
export type MaintenanceWindow = { id: string; server_id: string; starts_at: string; ends_at: string; reason: string; created_by: string; created_at: string; ended_at: string | null }
export type DesiredState = { mode: 'MONITOR' | 'ENFORCE'; firewall_enabled: boolean | null; ssh_password_auth: boolean | null; ssh_root_login: string | null; automatic_security_updates: boolean | null }
export type DesiredStateView = { policy: DesiredState; drift: Array<{ field: string; desired: string; actual: string; severity: string }>; enforcement_available: boolean }

export type ProjectSummary = {
  id: string
  name: string
  description: string
  preset: 'empty' | 'software' | 'website' | 'infrastructure' | 'client'
  status: string
  tags: string[]
  client_id: string | null
  open_tasks: number
  created_at: string
  updated_at: string
}
export type ProjectTask = {
  id: string
  project_id: string
  milestone_id: string | null
  title: string
  description: string
  status: 'TODO' | 'IN_PROGRESS' | 'BLOCKED' | 'DONE' | 'CANCELLED'
  priority: 'LOW' | 'MEDIUM' | 'HIGH' | 'URGENT'
  assignee_user_id: string | null
  due_at: string | null
  labels: string[]
  created_by: string
  created_at: string
  updated_at: string
}
export type ProjectNote = { id: string; project_id: string; title: string; content: string; created_by: string; created_at: string; updated_at: string }
export type Milestone = { id: string; project_id: string; name: string; description: string; status: 'OPEN' | 'COMPLETED' | 'CANCELLED'; due_at: string | null; created_by: string; created_at: string; updated_at: string }
export type ProjectActivity = { event_type: string; data: Record<string, unknown>; occurred_at: string }
export type ProjectWorkspace = { project: ProjectSummary; tasks: ProjectTask[]; notes: ProjectNote[]; milestones: Milestone[]; activity: ProjectActivity[] }
export type CreateProjectInput = { name: string; description?: string; preset?: ProjectSummary['preset']; tags?: string[] }
export type CreateTaskInput = { title: string; description?: string; priority?: ProjectTask['priority']; due_at?: string | null; milestone_id?: string | null; assignee_user_id?: string | null; labels?: string[] }
export type CreateMilestoneInput = { name: string; description?: string; due_at?: string | null }

export type RepositoryLink = {
  id: string
  project_id: string
  provider: 'github'
  owner: string
  name: string
  html_url: string
  default_branch: string
  visibility: string
  snapshot: {
    default_branch: string
    latest_commit: { sha: string; message: string; committed_at: string | null } | null
    open_pull_requests: number
    open_issues: number
    counts_truncated: boolean
    ci: { state: 'NONE' | 'RUNNING' | 'SUCCESS' | 'FAILURE' | 'UNAVAILABLE' | string; total_checks: number }
    warnings: string[]
  }
  sync_status: 'PENDING' | 'SYNCED' | 'ERROR'
  sync_error: string | null
  last_synced_at: string | null
  created_at: string
  updated_at: string
}

export type ServiceAction = 'start' | 'stop' | 'restart'
export type ContainerAction = 'start' | 'stop' | 'restart'
export type ServerOperation = 'packages.refresh' | 'packages.upgrade.security' | 'packages.upgrade.all' | 'system.reboot'

const controlApi = process.env.ARGUS_CONTROL_API_URL ?? 'http://localhost:8080'
async function authHeaders(): Promise<Record<string, string>> {
  const token = process.env.ARGUS_WEB_API_TOKEN
  const organizationId = process.env.ARGUS_ORG_ID
  const sessionToken = currentSessionToken()
  const sessionUser = sessionToken ? await getWorkspaceUser(sessionToken) : null
  const userId = sessionUser?.argusUserId ?? process.env.ARGUS_USER_ID
  if (!token || !organizationId || !userId) throw new Error('ARGUS_WEB_API_TOKEN, ARGUS_ORG_ID and ARGUS_USER_ID are required')
  return { authorization: `Bearer ${token}`, 'x-argus-org-id': organizationId, 'x-argus-user-id': userId, 'x-argus-user-role': sessionUser?.role ?? 'system' }
}
async function request<T>(path: string, init: RequestInit = {}): Promise<T> {
  const res = await fetch(`${controlApi}${path}`, { ...init, cache: 'no-store', headers: { ...(await authHeaders()), ...(init.headers ?? {}) } })
  if (!res.ok) throw new Error(`Control API ${res.status}: ${await res.text()}`)
  return res.status === 204 ? (undefined as T) : res.json()
}

export const getProjects = (): Promise<ProjectSummary[]> => request('/projects')
export const getProjectWorkspace = (projectId: string): Promise<ProjectWorkspace> => request(`/projects/${projectId}`)
export async function createProject(input: CreateProjectInput): Promise<ProjectSummary> {
  return request('/projects', { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify(input) })
}
export async function createProjectTask(projectId: string, input: CreateTaskInput): Promise<ProjectTask> {
  return request(`/projects/${projectId}/tasks`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify(input) })
}
export async function updateProjectTaskStatus(projectId: string, taskId: string, status: ProjectTask['status']): Promise<ProjectTask> {
  return request(`/projects/${projectId}/tasks/${taskId}/status`, { method: 'PUT', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ status }) })
}
export async function createProjectNote(projectId: string, title: string, content: string): Promise<ProjectNote> {
  return request(`/projects/${projectId}/notes`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ title, content }) })
}
export async function updateProjectNote(projectId: string, noteId: string, title: string, content: string): Promise<ProjectNote> {
  return request(`/projects/${projectId}/notes/${noteId}`, { method: 'PUT', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ title, content }) })
}
export async function createProjectMilestone(projectId: string, input: CreateMilestoneInput): Promise<Milestone> {
  return request(`/projects/${projectId}/milestones`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify(input) })
}
export async function updateProjectMilestoneStatus(projectId: string, milestoneId: string, status: Milestone['status']): Promise<Milestone> {
  return request(`/projects/${projectId}/milestones/${milestoneId}/status`, { method: 'PUT', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ status }) })
}

export const getProjectRepositories = (projectId: string): Promise<RepositoryLink[]> => request(`/projects/${projectId}/repositories`)
export async function linkGitHubRepository(projectId: string, owner: string, name: string): Promise<RepositoryLink> {
  return request(`/projects/${projectId}/repositories`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ owner, name }) })
}
export async function syncProjectRepository(projectId: string, repositoryId: string): Promise<RepositoryLink> {
  return request(`/projects/${projectId}/repositories/${repositoryId}/sync`, { method: 'POST' })
}
export async function unlinkProjectRepository(projectId: string, repositoryId: string): Promise<void> {
  await request(`/projects/${projectId}/repositories/${repositoryId}`, { method: 'DELETE' })
}

export const getServers = (): Promise<ServerView[]> => request('/servers')
export async function createServer(projectId: string, environmentId: string, hostname: string): Promise<{ server_id: string }> {
  return request('/servers', { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ project_id: projectId, environment_id: environmentId, hostname }) })
}
export async function createEnrollmentToken(serverId: string): Promise<{ token: string; expires_at: string }> {
  return request('/enrollment/tokens', { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ server_id: serverId, ttl_seconds: 900 }) })
}
export const getServer = (serverId: string): Promise<ServerView> => request(`/servers/${serverId}`)
export const getCommandHistory = (serverId: string): Promise<CommandHistoryItem[]> => request(`/servers/${serverId}/commands`)
export const getMetricHistory = (serverId: string, hours = 24): Promise<MetricSample[]> => request(`/servers/${serverId}/metrics?hours=${Math.max(1, Math.min(720, hours))}`)
export const getMaintenanceHistory = (serverId: string): Promise<MaintenanceWindow[]> => request(`/servers/${serverId}/maintenance`)
export const getDesiredState = (serverId: string): Promise<DesiredStateView> => request(`/servers/${serverId}/desired-state`)

export async function updateDesiredState(serverId: string, policy: DesiredState): Promise<void> {
  await request(`/servers/${serverId}/desired-state`, { method: 'PUT', headers: { 'content-type': 'application/json' }, body: JSON.stringify(policy) })
}
export async function serviceAction(serverId: string, service: string, action: ServiceAction): Promise<void> { await queue(serverId, { kind: `service.${action}`, service }, action === 'stop' ? 'HIGH' : 'MEDIUM', 60) }
export async function containerAction(serverId: string, container: string, action: ContainerAction): Promise<void> { await queue(serverId, { kind: `docker.${action}`, container }, action === 'stop' ? 'HIGH' : 'MEDIUM', 120) }
export async function createBackup(serverId: string): Promise<void> { await queue(serverId, { kind: 'backup.create', profile: 'system-config' }, 'MEDIUM', 600) }
export async function verifyBackup(serverId: string, backup: string): Promise<void> { await queue(serverId, { kind: 'backup.verify', backup }, 'LOW', 300) }
export async function serverOperation(serverId: string, operation: ServerOperation): Promise<void> {
  const risk = operation === 'system.reboot' ? 'CRITICAL' : operation === 'packages.refresh' ? 'MEDIUM' : 'HIGH'
  const ttl = operation.startsWith('packages.upgrade') ? 3600 : 300
  await queue(serverId, { kind: operation }, risk, ttl)
}
export async function queueArgusUpdate(serverId: string, version: string): Promise<void> {
  if (!/^[A-Za-z0-9_][A-Za-z0-9_.-]{0,127}$/.test(version)) throw new Error('Invalid update version')
  await queue(serverId, { kind: 'argus.update', version }, 'CRITICAL', 3600)
}
async function queue(serverId: string, commandType: Record<string, string>, riskLevel: string, ttlSeconds: number): Promise<void> {
  await request('/commands', { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ server_id: serverId, command_type: commandType, risk_level: riskLevel, ttl_seconds: ttlSeconds, idempotency_key: randomUUID() }) })
}
export async function startMaintenance(serverId: string, durationMinutes: number, reason: string): Promise<void> { await request(`/servers/${serverId}/maintenance/start`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ duration_minutes: durationMinutes, reason }) }) }
export async function endMaintenance(serverId: string): Promise<void> { await request(`/servers/${serverId}/maintenance/end`, { method: 'POST' }) }


export type CatalogService = {
  id: string
  project_id: string
  environment_id: string | null
  server_id: string | null
  repository_id: string | null
  name: string
  description: string
  service_type: 'web' | 'api' | 'worker' | 'database' | 'queue' | 'cron' | 'other' | string
  runtime: string | null
  owner_user_id: string | null
  endpoint_url: string | null
  lifecycle_status: 'ACTIVE' | 'PAUSED' | 'ARCHIVED'
  health_status: string
  created_at: string
  updated_at: string
}

export type CreateCatalogServiceInput = {
  name: string
  description?: string
  service_type: string
  runtime?: string | null
  repository_id?: string | null
  environment_id?: string | null
  server_id?: string | null
  owner_user_id?: string | null
  endpoint_url?: string | null
}

export type UpdateCatalogServiceInput = CreateCatalogServiceInput & {
  lifecycle_status: CatalogService['lifecycle_status']
}

export const getProjectServices = (projectId: string): Promise<CatalogService[]> =>
  request(`/projects/${projectId}/services`)

export async function createCatalogService(
  projectId: string,
  input: CreateCatalogServiceInput,
): Promise<CatalogService> {
  return request(`/projects/${projectId}/services`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(input),
  })
}

export async function updateCatalogService(
  projectId: string,
  serviceId: string,
  input: UpdateCatalogServiceInput,
): Promise<CatalogService> {
  return request(`/projects/${projectId}/services/${serviceId}`, {
    method: 'PUT',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(input),
  })
}

export async function deleteCatalogService(projectId: string, serviceId: string): Promise<void> {
  await request(`/projects/${projectId}/services/${serviceId}`, { method: 'DELETE' })
}


export type ProjectEnvironment = {
  id: string
  project_id: string
  name: string
  environment_type: 'development' | 'preview' | 'staging' | 'production' | 'custom' | string
  description: string
  is_protected: boolean
  sort_order: number
  server_count: number
  service_count: number
  created_at: string
  updated_at: string
}

export type EnvironmentInput = {
  name: string
  environment_type: string
  description?: string
  is_protected?: boolean
}

export const getProjectEnvironments = (projectId: string): Promise<ProjectEnvironment[]> =>
  request(`/projects/${projectId}/environments`)

export async function createProjectEnvironment(
  projectId: string,
  input: EnvironmentInput,
): Promise<ProjectEnvironment> {
  return request(`/projects/${projectId}/environments`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(input),
  })
}

export async function updateProjectEnvironment(
  projectId: string,
  environmentId: string,
  input: EnvironmentInput,
): Promise<ProjectEnvironment> {
  return request(`/projects/${projectId}/environments/${environmentId}`, {
    method: 'PUT',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(input),
  })
}

export async function deleteProjectEnvironment(projectId: string, environmentId: string): Promise<void> {
  await request(`/projects/${projectId}/environments/${environmentId}`, { method: 'DELETE' })
}
