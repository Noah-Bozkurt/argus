'use server'

import { revalidatePath } from 'next/cache'
import {
  addStatusPageComponent,
  createStatusPage,
  deleteStatusPage,
  removeStatusPageComponent,
  updateStatusIncidentPublication,
  updateStatusPage,
} from '../../../lib/status-pages-api'

function text(formData: FormData, name: string): string {
  return String(formData.get(name) ?? '').trim()
}

function parseResource(value: string): { type: 'SITE' | 'SERVICE'; id: string } {
  const separator = value.indexOf(':')
  const type = value.slice(0, separator)
  const id = value.slice(separator + 1)
  if ((type !== 'SITE' && type !== 'SERVICE') || !id) {
    throw new Error('Invalid status component')
  }
  return { type, id }
}

export async function createStatusPageAction(projectId: string, formData: FormData) {
  await createStatusPage(projectId, text(formData, 'name'), text(formData, 'slug'))
  revalidatePath(`/projects/${projectId}`)
}

export async function updateStatusPageAction(projectId: string, pageId: string, formData: FormData) {
  await updateStatusPage(
    projectId,
    pageId,
    text(formData, 'name'),
    text(formData, 'slug'),
    text(formData, 'visibility') === 'PUBLIC' ? 'PUBLIC' : 'INTERNAL',
  )
  revalidatePath(`/projects/${projectId}`)
  revalidatePath(`/status/${text(formData, 'slug')}`)
}

export async function deleteStatusPageAction(projectId: string, pageId: string) {
  await deleteStatusPage(projectId, pageId)
  revalidatePath(`/projects/${projectId}`)
}

export async function addStatusComponentAction(projectId: string, pageId: string, formData: FormData) {
  const resource = parseResource(text(formData, 'resource'))
  await addStatusPageComponent(
    projectId,
    pageId,
    resource.type,
    resource.id,
    text(formData, 'display_name'),
  )
  revalidatePath(`/projects/${projectId}`)
}

export async function removeStatusComponentAction(
  projectId: string,
  pageId: string,
  componentId: string,
) {
  await removeStatusPageComponent(projectId, pageId, componentId)
  revalidatePath(`/projects/${projectId}`)
}

export async function updateStatusIncidentPublicationAction(
  projectId: string,
  pageId: string,
  formData: FormData,
) {
  await updateStatusIncidentPublication(
    projectId,
    pageId,
    text(formData, 'incident_id'),
    text(formData, 'public_title'),
    text(formData, 'public_message'),
    formData.get('is_published') === 'on',
  )
  revalidatePath(`/projects/${projectId}`)
}
