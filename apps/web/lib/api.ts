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
  diagnostics: {
    failed_units: string[]
    listening_tcp_ports: number[]
    journals: Array<{ service: string; output: string }>
  }
  docker: {
    available: boolean
    containers: Array<{ id: string; name: string; image: string; state: string; status: string; ports: string }>
  }
  security: {
    available: boolean
    firewall_status: string
    firewall_rules: string[]
    ssh_password_auth: boolean | null
    ssh_root_login: string
    automatic_security_updates: boolean
    findings: Array<{ severity: string; code: string; message: string }>
  }
  captured_at: string
}

export type ServerView = {
  server_id: string
  project_id: string
  environment_id: string
  hostname: string
  online: boolean
  last_heartbeat: string | null
  snapshot: SystemSnapshot | null
  services: Array<{ name: string; status: string }>
}

export type CommandHistoryItem = {
  command: {
    id: string
    server_id: string
    command_type: { kind: string; service?: string; container?: string }
    created_at: string
    expires_at: string
    status: string
    idempotency_key: string
    risk_level: string
  }
  started_at: string | null
  finished_at: string | null
  error_code: string | null
  error_message: string | null
  actor_user_id: string | null
}

export type MaintenanceWindow = {
  id: string
  server_id: string
  starts_at: string
  ends_at: string
  reason: string
  created_by: string
  created_at: string
  ended_at: string | null
}

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

export const getServers = (): Promise<ServerView[]> => request('/servers')
export const getServer = (serverId: string): Promise<ServerView> => request(`/servers/${serverId}`)
export const getCommandHistory = (serverId: string): Promise<CommandHistoryItem[]> => request(`/servers/${serverId}/commands`)
export const getMaintenanceHistory = (serverId: string): Promise<MaintenanceWindow[]> => request(`/servers/${serverId}/maintenance`)

export async function serviceAction(serverId: string, service: string, action: ServiceAction): Promise<void> {
  await queue(serverId, { kind: `service.${action}`, service }, action === 'stop' ? 'HIGH' : 'MEDIUM', 60)
}
export async function containerAction(serverId: string, container: string, action: ContainerAction): Promise<void> {
  await queue(serverId, { kind: `docker.${action}`, container }, action === 'stop' ? 'HIGH' : 'MEDIUM', 120)
}
export async function serverOperation(serverId: string, operation: ServerOperation): Promise<void> {
  const risk = operation === 'system.reboot' ? 'CRITICAL' : operation === 'packages.refresh' ? 'MEDIUM' : 'HIGH'
  const ttl = operation.startsWith('packages.upgrade') ? 3600 : 300
  await queue(serverId, { kind: operation }, risk, ttl)
}
async function queue(serverId: string, commandType: Record<string, string>, riskLevel: string, ttlSeconds: number): Promise<void> {
  await request('/commands', { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ server_id: serverId, command_type: commandType, risk_level: riskLevel, ttl_seconds: ttlSeconds, idempotency_key: randomUUID() }) })
}
export async function startMaintenance(serverId: string, durationMinutes: number, reason: string): Promise<void> {
  await request(`/servers/${serverId}/maintenance/start`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ duration_minutes: durationMinutes, reason }) })
}
export async function endMaintenance(serverId: string): Promise<void> { await request(`/servers/${serverId}/maintenance/end`, { method: 'POST' }) }
