'use server'

import { revalidatePath } from 'next/cache'

import { createContentModel, saveContentRecord, type ContentField } from '../../../../lib/content-api'

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
  await saveContentRecord(projectId, {
    model_id: text(formData, 'model_id'),
    record_id: text(formData, 'record_id') || undefined,
    values: Object.fromEntries(fields.map((field) => [field.key, fieldValue(formData, field)])),
    publish: text(formData, 'intent') === 'publish',
  })
  revalidatePath(`/projects/${projectId}/content`)
}
