'use server'

import {
  containerAction,
  createBackup,
  endMaintenance,
  serverOperation,
  serviceAction,
  startMaintenance,
  updateDesiredState,
  verifyBackup,
  type ContainerAction,
  type DesiredState,
  type ServerOperation,
  type ServiceAction,
} from '../../../../lib/api'
import { enableDesiredFirewall } from '../../../../lib/firewall-api'
import { preflightBackupRestore } from '../../../../lib/restore-api'

export async function actOnService(serverId: string, service: string, action: ServiceAction) {
  await serviceAction(serverId, service, action)
}

export async function actOnContainer(serverId: string, container: string, action: ContainerAction) {
  await containerAction(serverId, container, action)
}

export async function actOnServer(serverId: string, operation: ServerOperation) {
  await serverOperation(serverId, operation)
}

export async function createSystemConfigBackup(serverId: string) {
  await createBackup(serverId)
}

export async function verifySystemConfigBackup(serverId: string, backup: string) {
  await verifyBackup(serverId, backup)
}

export async function preflightSystemConfigRestore(serverId: string, backup: string) {
  await preflightBackupRestore(serverId, backup)
}

export async function beginMaintenance(serverId: string, durationMinutes: number, reason: string) {
  await startMaintenance(serverId, durationMinutes, reason)
}

export async function finishMaintenance(serverId: string) {
  await endMaintenance(serverId)
}

export async function enforceDesiredFirewall(serverId: string) {
  await enableDesiredFirewall(serverId)
}

function optionalBoolean(value: FormDataEntryValue | null): boolean | null {
  if (value === 'true') return true
  if (value === 'false') return false
  return null
}

export async function saveDesiredState(serverId: string, formData: FormData) {
  const rootLogin = formData.get('ssh_root_login')
  const requestedMode = String(formData.get('mode') ?? 'MONITOR')
  const policy: DesiredState = {
    mode: requestedMode === 'ENFORCE' ? 'ENFORCE' : 'MONITOR',
    firewall_enabled: optionalBoolean(formData.get('firewall_enabled')),
    ssh_password_auth: optionalBoolean(formData.get('ssh_password_auth')),
    ssh_root_login:
      rootLogin === 'no' || rootLogin === 'prohibit-password' || rootLogin === 'yes'
        ? rootLogin
        : null,
    automatic_security_updates: optionalBoolean(formData.get('automatic_security_updates')),
  }
  await updateDesiredState(serverId, policy)
}
