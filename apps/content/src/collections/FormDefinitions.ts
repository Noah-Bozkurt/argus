import type { CollectionBeforeValidateHook, CollectionConfig } from 'payload'

import { createProjectDocument, editProjectDocuments, manageProjectDocuments, readProjectDocuments } from '@/access/projectAccess'
import { FORM_FIELD_TYPES, FORM_SLUG_PATTERN } from '@/lib/argusFormsContract'
import { resolveProjectScope } from '@/lib/projectScope'

const validateForm: CollectionBeforeValidateHook = async ({ data, operation, originalDoc, req }) => {
  if (!data) return data
  if (operation === 'update' && originalDoc) {
    data.project = originalDoc.project
    data.organizationId = originalDoc.organizationId
    data.argusProjectId = originalDoc.argusProjectId
    data.slug = originalDoc.slug
  }
  const scope = await resolveProjectScope(req, data.project ?? originalDoc?.project)
  const slug = String(data.slug ?? originalDoc?.slug ?? '').trim().toLowerCase()
  if (!FORM_SLUG_PATTERN.test(slug)) throw new Error('Form slug is invalid')
  const duplicate = await req.payload.find({
    collection: 'form-definitions', depth: 0, limit: 1, overrideAccess: true, pagination: false,
    where: { and: [{ project: { equals: scope.projectID } }, { slug: { equals: slug } }, ...(operation === 'update' && originalDoc?.id ? [{ id: { not_equals: originalDoc.id } }] : [])] },
  })
  if (duplicate.docs.length > 0) throw new Error(`Form slug '${slug}' already exists in this project`)
  const fields = (Array.isArray(data.fields) ? data.fields : originalDoc?.fields ?? []) as Array<Record<string, unknown>>
  if (fields.length < 1 || fields.length > 30) throw new Error('Forms require between 1 and 30 fields')
  const keys = new Set<string>()
  for (const field of fields) {
    const key = String(field.key ?? '').trim().toLowerCase()
    const type = String(field.type ?? '')
    if (!FORM_SLUG_PATTERN.test(key) || keys.has(key) || !FORM_FIELD_TYPES.includes(type as (typeof FORM_FIELD_TYPES)[number])) throw new Error(`Invalid or duplicate form field '${key}'`)
    keys.add(key)
    field.key = key
    const options = Array.isArray(field.options) ? field.options.map((item) => typeof item === 'object' && item && 'value' in item ? String(item.value).trim() : String(item).trim()).filter(Boolean) : []
    if (type === 'select' && (options.length < 1 || options.length > 50 || new Set(options).size !== options.length)) throw new Error(`Select field '${key}' requires unique options`)
    field.options = type === 'select' ? options.map((value) => ({ value })) : []
  }
  data.project = scope.projectID
  data.organizationId = scope.organizationId
  data.argusProjectId = scope.argusProjectId
  data.slug = slug
  data.fields = fields
  return data
}

export const FormDefinitions: CollectionConfig = {
  slug: 'form-definitions',
  admin: { group: 'Content', useAsTitle: 'name', defaultColumns: ['name', 'slug', 'status', 'project', 'updatedAt'] },
  access: {
    create: createProjectDocument('editor'), read: readProjectDocuments,
    update: editProjectDocuments, delete: manageProjectDocuments,
  },
  hooks: { beforeValidate: [validateForm] },
  fields: [
    { name: 'organizationId', type: 'text', required: true, index: true, admin: { readOnly: true } },
    { name: 'argusProjectId', type: 'text', required: true, index: true, admin: { readOnly: true } },
    { name: 'project', type: 'relationship', relationTo: 'project-spaces', required: true, index: true, admin: { description: 'Immutable project ownership.' } },
    { name: 'name', type: 'text', required: true, maxLength: 160 },
    { name: 'slug', type: 'text', required: true, maxLength: 120, index: true, admin: { description: 'Immutable public form identifier.' } },
    { name: 'description', type: 'textarea', maxLength: 2000 },
    { name: 'successMessage', type: 'text', required: true, maxLength: 500 },
    {
      name: 'status', type: 'select', required: true, defaultValue: 'draft',
      options: [{ label: 'Draft', value: 'draft' }, { label: 'Published', value: 'published' }, { label: 'Archived', value: 'archived' }],
    },
    {
      name: 'fields', type: 'array', required: true, minRows: 1, maxRows: 30,
      fields: [
        { name: 'key', type: 'text', required: true, maxLength: 120 },
        { name: 'label', type: 'text', required: true, maxLength: 160 },
        { name: 'type', type: 'select', required: true, options: FORM_FIELD_TYPES.map((value) => ({ label: value, value })) },
        { name: 'required', type: 'checkbox', defaultValue: false },
        { name: 'options', type: 'array', maxRows: 50, fields: [{ name: 'value', type: 'text', required: true, maxLength: 160 }] },
      ],
    },
  ],
}
