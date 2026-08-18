import { randomUUID } from 'node:crypto'

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
  updates: { supported: boolean; pending_updates: number; reboot_required: boolean }
  diagnostics: { failed_units: string[]; listening_tcp_ports: number[]; journals: Array<{ service: string; output: string }> }
  docker: { available: boolean; containers: Array<{ id: string; name: string; image: string; state: string; status: string; ports: string }> }
  security: { available: boolean; firewall_status: string; firewall_rules: string[]; ssh_password_auth: boolean | null; ssh_root_login: string; automatic_security_updates: boolean; findings: Array<{ severity: string; code: string; message: string }> }
  backups: {
    available: boolean
    target: string
    artifacts: Array<{ name: string; profile: string; size_bytes: number; created_unix: number; sha256: string; verified: boolean }>
  }
  captured_at: string
}

export type ServerView = { server_id: string; project_id: string; environment_id: string; hostname: string; online: boolean; last_heartbeat: string | null; snapshot: SystemSnapshot | null; services: Array<{ name: string; status: string }> }
export type CommandHistoryItem = { command: { id: string; server_id: string; command_type: { kind: string; service?: string; container?: string; profile?: string; backup?: string }; created_at: string; expires_at: string; status: string; idempotency_key: string; risk_level: string }; started_at: string | null; finished_at: string | null; error_code: string | null; error_message: string | null; actor_user_id: string | null }
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

export type ServiceAction = 'start' | 'stop' | 'restart'
export type ContainerAction = 'start' | 'stop' | 'restart'
export type ServerOperation = 'packages.refresh' | 'packages.upgrade.security' | 'packages.upgrade.all' | 'system.reboot'

const controlApi = process.env.ARGUS_CONTROL_API_URL ?? 'http://localhost:8080'
function authHeaders(): Record<string, string> {
  const token = process.env.ARGUS_WEB_API_TOKEN
  const organizationId = process.env.ARGUS_ORG_ID
  const userId = process.env.ARGUS_USER_ID
  if (!token || !organizationId || !userId) throw new Error('ARGUS_WEB_API_TOKEN, ARGUS_ORG_ID and ARGUS_USER_ID are required')
  return { authorization: `Bearer ${token}`, 'x-argus-org-id': organizationId, 'x-argus-user-id': userId }
}
async function request<T>(path: string, init: RequestInit = {}): Promise<T> {
  const res = await fetch(`${controlApi}${path}`, { ...init, cache: 'no-store', headers: { ...authHeaders(), ...(init.headers ?? {}) } })
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

export const getServers = (): Promise<ServerView[]> => request('/servers')
export const getServer = (serverId: string): Promise<ServerView> => request(`/servers/${serverId}`)
export const getCommandHistory = (serverId: string): Promise<CommandHistoryItem[]> => request(`/servers/${serverId}/commands`)
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
async function queue(serverId: string, commandType: Record<string, string>, riskLevel: string, ttlSeconds: number): Promise<void> {
  await request('/commands', { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ server_id: serverId, command_type: commandType, risk_level: riskLevel, ttl_seconds: ttlSeconds, idempotency_key: randomUUID() }) })
}
export async function startMaintenance(serverId: string, durationMinutes: number, reason: string): Promise<void> { await request(`/servers/${serverId}/maintenance/start`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ duration_minutes: durationMinutes, reason }) }) }
export async function endMaintenance(serverId: string): Promise<void> { await request(`/servers/${serverId}/maintenance/end`, { method: 'POST' }) }
