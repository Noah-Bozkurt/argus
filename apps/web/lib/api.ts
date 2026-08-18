export type ServerSnapshot = {
  serverId: string
  hostname: string
  online: boolean
  os: string
  agentVersion: string
  cpuPercent: number
  ramPercent: number
  diskPercent: number
  load: number
  uptimeSeconds: number
  services: Array<{ name: string; status: string }>
}

const controlApi = process.env.NEXT_PUBLIC_CONTROL_API_URL ?? 'http://localhost:8080'

export async function getServers(): Promise<ServerSnapshot[]> {
  const res = await fetch(`${controlApi}/servers`, { cache: 'no-store' })
  if (!res.ok) throw new Error('Failed to load servers')
  return res.json()
}

export async function getServer(serverId: string): Promise<ServerSnapshot> {
  const res = await fetch(`${controlApi}/servers/${serverId}`, { cache: 'no-store' })
  if (!res.ok) throw new Error('Failed to load server')
  return res.json()
}

export async function restartService(serverId: string, service: string): Promise<void> {
  const res = await fetch(`${controlApi}/commands`, {
    method: 'POST',
    headers: { 'content-type': 'application/json', 'x-org-id': 'demo' },
    body: JSON.stringify({
      server_id: serverId,
      command_type: { kind: 'service.restart', service },
      risk_level: 'MEDIUM',
      ttl_seconds: 60,
      idempotency_key: `${serverId}-${service}`,
    }),
  })

  if (!res.ok) {
    throw new Error('Failed to request service restart')
  }
}
