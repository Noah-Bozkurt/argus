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
    command_type: { kind: string; service?: string }
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
  const res = await fetch(`${controlApi}${path}`, {
    ...init,
    cache: 'no-store',
    headers: { ...authHeaders(), ...(init.headers ?? {}) },
  })
  if (!res.ok) {
    const body = await res.text()
    throw new Error(`Control API ${res.status}: ${body}`)
  }
  return res.status === 204 ? (undefined as T) : res.json()
}

export function getServers(): Promise<ServerView[]> {
  return request('/servers')
}

export function getServer(serverId: string): Promise<ServerView> {
  return request(`/servers/${serverId}`)
}

export function getCommandHistory(serverId: string): Promise<CommandHistoryItem[]> {
  return request(`/servers/${serverId}/commands`)
}

export async function restartService(serverId: string, service: string): Promise<void> {
  await request('/commands', {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({
      server_id: serverId,
      command_type: { kind: 'service.restart', service },
      risk_level: 'MEDIUM',
      ttl_seconds: 60,
      idempotency_key: randomUUID(),
    }),
  })
}
