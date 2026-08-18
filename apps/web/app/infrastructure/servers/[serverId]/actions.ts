'use server'

import { restartService } from '../../../../lib/api'

export async function restart(serverId: string, service: string) {
  await restartService(serverId, service)
}
