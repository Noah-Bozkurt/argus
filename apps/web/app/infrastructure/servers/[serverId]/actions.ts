'use server'

import { serviceAction, type ServiceAction } from '../../../../lib/api'

export async function actOnService(
  serverId: string,
  service: string,
  action: ServiceAction,
) {
  await serviceAction(serverId, service, action)
}
