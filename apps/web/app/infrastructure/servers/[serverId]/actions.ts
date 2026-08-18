'use server'

import {
  endMaintenance,
  serverOperation,
  serviceAction,
  startMaintenance,
  type ServerOperation,
  type ServiceAction,
} from '../../../../lib/api'

export async function actOnService(serverId: string, service: string, action: ServiceAction) {
  await serviceAction(serverId, service, action)
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
