'use server'

import { revalidatePath } from 'next/cache'
import { saveMonitorSchedule } from '../../../lib/monitor-scheduling-api'

export async function saveMonitorScheduleAction(
  projectId: string,
  siteId: string,
  formData: FormData,
): Promise<void> {
  const enabled = formData.get('enabled') === 'on'
  const intervalSeconds = Number(formData.get('interval_seconds') ?? 300)
  if (!Number.isInteger(intervalSeconds) || intervalSeconds < 60 || intervalSeconds > 86_400) {
    throw new Error('Monitor interval must be between 60 and 86400 seconds')
  }
  await saveMonitorSchedule(projectId, siteId, enabled, intervalSeconds)
  revalidatePath(`/projects/${projectId}`)
}
