'use server'

import { revalidatePath } from 'next/cache'

import { createContentModel, createProjectForm, deleteContentModel, deleteContentRecord, deleteFormSubmission, deleteMedia, saveContentRecord, setContentModelStatus, setContentRecordStatus, updateContentModel, updateFormSubmissionStatus, updateMedia, updateProjectFormStatus, uploadMedia, type ContentBlock, type ContentField, type ContentModel, type FormField, type FormSubmission, type ProjectForm } from '../../../../lib/content-api'

function text(formData: FormData, name: string): string {
  return String(formData.get(name) ?? '').trim()
}

function modelFields(formData: FormData): ContentField[] {
  const fields: ContentField[] = []
  for (let index = 0; index < 50; index += 1) {
    const key = text(formData, `field_${index}_key`).toLowerCase()
    if (!key) continue
    fields.push({
      key,
      label: text(formData, `field_${index}_label`),
      type: text(formData, `field_${index}_type`) as ContentField['type'],
      required: formData.get(`field_${index}_required`) === 'on',
      target_model_id: text(formData, `field_${index}_target_model_id`) || null,
      has_many: formData.get(`field_${index}_has_many`) === 'on',
    })
  }
  if (fields.length === 0) throw new Error('A content type needs at least one field')
  return fields
}

function modelInput(formData: FormData) {
  return {
    name: text(formData, 'name'),
    slug: text(formData, 'slug').toLowerCase(),
    description: text(formData, 'description'),
    public_read: formData.get('public_read') === 'on',
    content_role: text(formData, 'content_role') as ContentModel['content_role'],
    allowed_component_ids: formData.getAll('allowed_component_ids').map(String),
    fields: modelFields(formData),
  }
}

export async function createContentModelAction(projectId: string, formData: FormData) {
  await createContentModel(projectId, modelInput(formData))
  revalidatePath(`/projects/${projectId}/content`)
}

export async function updateContentModelAction(projectId: string, modelId: string, formData: FormData) {
  await updateContentModel(projectId, modelId, modelInput(formData))
  revalidatePath(`/projects/${projectId}/content`)
}

export async function setContentModelStatusAction(projectId: string, modelId: string, status: 'active' | 'archived') {
  await setContentModelStatus(projectId, modelId, status)
  revalidatePath(`/projects/${projectId}/content`)
}

export async function deleteContentModelAction(projectId: string, modelId: string, formData: FormData) {
  if (formData.get('confirm_delete') !== 'on') throw new Error('Confirm content type deletion')
  await deleteContentModel(projectId, modelId)
  revalidatePath(`/projects/${projectId}/content`)
}

function fieldValue(formData: FormData, field: ContentField): unknown {
  if (field.type === 'media') {
    const selected = formData.getAll(`value_${field.key}`).map(String).filter(Boolean)
    return field.has_many ? selected : (selected[0] ?? '')
  }
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
    values: Object.fromEntries(fields.filter((field) => field.type !== 'relationship').map((field) => [field.key, fieldValue(formData, field)])),
    relationships: Object.fromEntries(fields.filter((field) => field.type === 'relationship').map((field) => [field.key, formData.getAll(`relation_${field.key}`).map(String).filter(Boolean)])),
    layout,
    publish: text(formData, 'intent') === 'publish',
  })
  revalidatePath(`/projects/${projectId}/content`)
}

export async function setContentRecordStatusAction(projectId: string, recordId: string, status: 'active' | 'archived') {
  await setContentRecordStatus(projectId, recordId, status)
  revalidatePath(`/projects/${projectId}/content`)
}

export async function deleteContentRecordAction(projectId: string, recordId: string, formData: FormData) {
  if (formData.get('confirm_delete') !== 'on') throw new Error('Confirm record deletion')
  await deleteContentRecord(projectId, recordId)
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
