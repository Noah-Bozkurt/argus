'use server'

import {
  containerAction,
  endMaintenance,
  serverOperation,
  serviceAction,
  startMaintenance,
  updateDesiredState,
  type ContainerAction,
  type DesiredState,
  type ServerOperation,
  type ServiceAction,
} from '../../../../lib/api'

export async function actOnService(serverId: string, service: string, action: ServiceAction) {
  await serviceAction(serverId, service, action)
}

export async function actOnContainer(serverId: string, container: string, action: ContainerAction) {
  await containerAction(serverId, container, action)
}

export async function actOnServer(serverId: string, operation: ServerOperation) {
  await serverOperation(serverId, operation)
}

export async function beginMaintenance(serverId: string, durationMinutes: number, reason: string) {
  await startMaintenance(serverId, durationMinutes, reason)
}

export async function finishMaintenance(serverId: string) {
  await endMaintenance(serverId)
}

function optionalBoolean(value: FormDataEntryValue | null): boolean | null {
  if (value === 'true') return true
  if (value === 'false') return false
  return null
}

export async function saveDesiredState(serverId: string, formData: FormData) {
  const rootLogin = formData.get('ssh_root_login')
  const policy: DesiredState = {
    mode: 'MONITOR',
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
