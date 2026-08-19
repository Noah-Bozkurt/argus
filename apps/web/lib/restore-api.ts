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

async function queueRestoreCommand(
  serverId: string,
  backup: string,
  kind: 'backup.restore.preflight' | 'backup.restore.apply',
  riskLevel: 'LOW' | 'CRITICAL',
): Promise<void> {
  const response = await fetch(`${controlApi}/commands`, {
    method: 'POST',
    cache: 'no-store',
    headers: {
      ...authHeaders(),
      'content-type': 'application/json',
    },
    body: JSON.stringify({
      server_id: serverId,
      command_type: { kind, backup },
      risk_level: riskLevel,
      ttl_seconds: 300,
      idempotency_key: randomUUID(),
    }),
  })
  if (!response.ok) {
    throw new Error(`Control API ${response.status}: ${await response.text()}`)
  }
}

export async function preflightBackupRestore(serverId: string, backup: string): Promise<void> {
  await queueRestoreCommand(serverId, backup, 'backup.restore.preflight', 'LOW')
}

export async function applyBackupRestore(serverId: string, backup: string): Promise<void> {
  await queueRestoreCommand(serverId, backup, 'backup.restore.apply', 'CRITICAL')
}
