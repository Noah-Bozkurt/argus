'use server'

import { revalidatePath } from 'next/cache'
import {
  saveSiteIncidentPolicy,
  type SiteIncidentPolicyView,
} from '../../../lib/incident-automation-api'

export async function saveSiteIncidentPolicyAction(
  projectId: string,
  siteId: string,
  formData: FormData,
): Promise<void> {
  const enabled = formData.get('enabled') === 'on'
  const threshold = Number(formData.get('failure_threshold') ?? 3)
  const severity = String(formData.get('severity') ?? 'MAJOR').toUpperCase() as SiteIncidentPolicyView['severity']
  if (!Number.isInteger(threshold) || threshold < 2 || threshold > 10) {
    throw new Error('Failure threshold must be between 2 and 10')
  }
  if (!['MINOR', 'MAJOR', 'CRITICAL'].includes(severity)) {
    throw new Error('Invalid Incident severity')
  }
  await saveSiteIncidentPolicy(projectId, siteId, enabled, threshold, severity)
  revalidatePath(`/projects/${projectId}`)
}
