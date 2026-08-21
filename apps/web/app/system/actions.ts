'use server'

import { currentSessionToken, getWorkspaceUser, loginWorkspace } from '../../lib/auth'
import { endMaintenance, queueArgusUpdate, startMaintenance } from '../../lib/api'

export async function startArgusUpdate(formData: FormData) {
  const token = currentSessionToken()
  const user = token ? await getWorkspaceUser(token) : null
  if (!user || user.role !== 'owner' || !user.argusUserId) throw new Error('Only workspace owners can update Argus')
  const verified = await loginWorkspace(user.email, String(formData.get('password') ?? ''))
  if (!verified || verified.user.id !== user.id || verified.user.role !== 'owner') throw new Error('Password re-authentication failed')
  const serverId = process.env.ARGUS_SERVER_ID
  if (!serverId) throw new Error('The local control-plane server is not configured')
  const version = String(formData.get('version') ?? 'main').trim() || 'main'
  await startMaintenance(serverId, 60, 'Argus control-plane update')
  try {
    await queueArgusUpdate(serverId, version)
  } catch (error) {
    await endMaintenance(serverId).catch(() => undefined)
    throw error
  }
}
