'use server'

import { revalidatePath } from 'next/cache'
import {
  runSiteMonitorCheck,
  saveSiteMonitorConfig,
} from '../../../lib/site-monitoring-api'

function text(formData: FormData, name: string): string {
  return String(formData.get(name) ?? '').trim()
}

export async function saveSiteMonitorAction(projectId: string, siteId: string, formData: FormData) {
  const timeoutSeconds = Number.parseInt(text(formData, 'timeout_seconds'), 10)
  await saveSiteMonitorConfig(projectId, siteId, {
    target_url: text(formData, 'target_url'),
    check_robots: formData.get('check_robots') === 'on',
    check_sitemap: formData.get('check_sitemap') === 'on',
    timeout_seconds: Number.isFinite(timeoutSeconds) ? timeoutSeconds : 10,
  })
  revalidatePath(`/projects/${projectId}`)
}

export async function runSiteMonitorAction(projectId: string, siteId: string) {
  await runSiteMonitorCheck(projectId, siteId)
  revalidatePath(`/projects/${projectId}`)
}
