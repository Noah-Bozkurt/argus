'use server'

import { revalidatePath } from 'next/cache'
import {
  addIncidentNote,
  createIncident,
  updateIncidentStatus,
  type IncidentSeverity,
  type IncidentStatus,
} from '../../../lib/incidents-api'

function text(formData: FormData, name: string): string {
  return String(formData.get(name) ?? '').trim()
}

function parseNode(value: string): { resourceType: string; resourceId: string } {
  const separator = value.indexOf(':')
  if (separator <= 0 || separator === value.length - 1) {
    throw new Error('Invalid incident source')
  }
  return {
    resourceType: value.slice(0, separator),
    resourceId: value.slice(separator + 1),
  }
}

export async function createIncidentAction(projectId: string, formData: FormData) {
  const source = parseNode(text(formData, 'source'))
  await createIncident(projectId, {
    title: text(formData, 'title'),
    summary: text(formData, 'summary'),
    severity: text(formData, 'severity') as IncidentSeverity,
    source_type: source.resourceType,
    source_id: source.resourceId,
  })
  revalidatePath(`/projects/${projectId}`)
}

export async function updateIncidentStatusAction(
  projectId: string,
  incidentId: string,
  status: IncidentStatus,
) {
  await updateIncidentStatus(projectId, incidentId, status)
  revalidatePath(`/projects/${projectId}`)
  revalidatePath(`/projects/${projectId}/incidents/${incidentId}`)
}

export async function addIncidentNoteAction(projectId: string, incidentId: string, formData: FormData) {
  await addIncidentNote(projectId, incidentId, text(formData, 'message'))
  revalidatePath(`/projects/${projectId}/incidents/${incidentId}`)
}
