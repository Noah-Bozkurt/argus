'use server'

import { revalidatePath } from 'next/cache'

import { createContentModel, createProjectForm, deleteFormSubmission, deleteMedia, saveContentRecord, updateFormSubmissionStatus, updateMedia, updateProjectFormStatus, uploadMedia, type ContentBlock, type ContentField, type ContentModel, type FormField, type FormSubmission, type ProjectForm } from '../../../../lib/content-api'

function text(formData: FormData, name: string): string {
  return String(formData.get(name) ?? '').trim()
}

export async function createContentModelAction(projectId: string, formData: FormData) {
  const fields: ContentField[] = []
  for (let index = 0; index < 5; index += 1) {
    const key = text(formData, `field_${index}_key`).toLowerCase()
    if (!key) continue
    fields.push({
      key,
      label: text(formData, `field_${index}_label`),
      type: text(formData, `field_${index}_type`) as ContentField['type'],
      required: formData.get(`field_${index}_required`) === 'on',
    })
  }
  await createContentModel(projectId, {
    name: text(formData, 'name'),
    slug: text(formData, 'slug').toLowerCase(),
    description: text(formData, 'description'),
    public_read: formData.get('public_read') === 'on',
    content_role: text(formData, 'content_role') as ContentModel['content_role'],
    allowed_component_ids: formData.getAll('allowed_component_ids').map(String),
    fields,
  })
  revalidatePath(`/projects/${projectId}/content`)
}

function fieldValue(formData: FormData, field: ContentField): unknown {
  if (field.type === 'boolean') return formData.get(`value_${field.key}`) === 'on'
  const value = text(formData, `value_${field.key}`)
  if (!value) return value
  if (field.type === 'number') return Number(value)
  if (field.type === 'json') {
    try { return JSON.parse(value) } catch { return value }
  }
  return value
}

export async function saveContentRecordAction(projectId: string, fields: ContentField[], formData: FormData) {
  let layout: ContentBlock[] = []
  try {
    const parsed = JSON.parse(text(formData, 'layout') || '[]')
    if (Array.isArray(parsed)) layout = parsed
  } catch {
    throw new Error('Page layout is not valid JSON')
  }
  await saveContentRecord(projectId, {
    model_id: text(formData, 'model_id'),
    record_id: text(formData, 'record_id') || undefined,
    values: Object.fromEntries(fields.map((field) => [field.key, fieldValue(formData, field)])),
    layout,
    publish: text(formData, 'intent') === 'publish',
  })
  revalidatePath(`/projects/${projectId}/content`)
}

export async function uploadMediaAction(projectId: string, formData: FormData) {
  const file = formData.get('file')
  if (!(file instanceof File) || file.size === 0 || file.size > 10 * 1024 * 1024) throw new Error('Choose an image no larger than 10 MiB')
  await uploadMedia(projectId, formData)
  revalidatePath(`/projects/${projectId}/content`)
}

export async function deleteMediaAction(projectId: string, mediaId: string, formData: FormData) {
  if (formData.get('confirm_delete') !== 'on') throw new Error('Confirm media deletion')
  await deleteMedia(projectId, mediaId)
  revalidatePath(`/projects/${projectId}/content`)
}

export async function updateMediaAction(projectId: string, mediaId: string, formData: FormData) {
  await updateMedia(projectId, {
    media_id: mediaId, alt: text(formData, 'alt'), caption: text(formData, 'caption'),
    public_read: formData.get('public_read') === 'on',
  })
  revalidatePath(`/projects/${projectId}/content`)
}

export async function createProjectFormAction(projectId: string, formData: FormData) {
  const fields: FormField[] = []
  for (let index = 0; index < 30; index += 1) {
    const key = text(formData, `form_field_${index}_key`).toLowerCase()
    if (!key) continue
    fields.push({
      key, label: text(formData, `form_field_${index}_label`),
      type: text(formData, `form_field_${index}_type`) as FormField['type'],
      required: formData.get(`form_field_${index}_required`) === 'on',
      options: text(formData, `form_field_${index}_options`).split(',').map((value) => value.trim()).filter(Boolean),
    })
  }
  await createProjectForm(projectId, {
    name: text(formData, 'form_name'), slug: text(formData, 'form_slug').toLowerCase(),
    description: text(formData, 'form_description'), success_message: text(formData, 'form_success_message'),
    published: formData.get('form_published') === 'on', fields,
  })
  revalidatePath(`/projects/${projectId}/content`)
}

export async function updateProjectFormStatusAction(projectId: string, formId: string, status: ProjectForm['status']) {
  await updateProjectFormStatus(projectId, formId, status)
  revalidatePath(`/projects/${projectId}/content`)
}

export async function updateFormSubmissionStatusAction(projectId: string, submissionId: string, status: FormSubmission['status']) {
  await updateFormSubmissionStatus(projectId, submissionId, status)
  revalidatePath(`/projects/${projectId}/content`)
}

export async function deleteFormSubmissionAction(projectId: string, submissionId: string, formData: FormData) {
  if (formData.get('confirm_delete') !== 'on') throw new Error('Confirm submission deletion')
  await deleteFormSubmission(projectId, submissionId)
  revalidatePath(`/projects/${projectId}/content`)
}
