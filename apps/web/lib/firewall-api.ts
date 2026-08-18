import { randomUUID } from 'node:crypto'

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

export async function enableDesiredFirewall(serverId: string): Promise<void> {
  const headers = authHeaders()
  const desiredResponse = await fetch(`${controlApi}/servers/${serverId}/desired-state`, {
    cache: 'no-store',
    headers,
  })
  if (!desiredResponse.ok) {
    throw new Error(`Control API ${desiredResponse.status}: ${await desiredResponse.text()}`)
  }
  const desiredState = await desiredResponse.json() as {
    policy?: { firewall_enabled?: boolean | null }
  }
  if (desiredState.policy?.firewall_enabled !== true) {
    throw new Error('Firewall enforcement requires desired state firewall_enabled=true')
  }

  const response = await fetch(`${controlApi}/commands`, {
    method: 'POST',
    cache: 'no-store',
    headers: {
      ...headers,
      'content-type': 'application/json',
    },
    body: JSON.stringify({
      server_id: serverId,
      command_type: { kind: 'security.firewall.enable' },
      risk_level: 'HIGH',
      ttl_seconds: 300,
      idempotency_key: randomUUID(),
    }),
  })
  if (!response.ok) {
    throw new Error(`Control API ${response.status}: ${await response.text()}`)
  }
}
