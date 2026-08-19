import type { CollectionBeforeValidateHook, CollectionConfig } from 'payload'
import {
  createProjectDocument,
  editProjectDocuments,
  manageProjectDocuments,
  readProjectDocuments,
  relationshipID,
} from '@/access/projectAccess'
import { resolveProjectScope } from '@/lib/projectScope'

const KEY_PATTERN = /^[a-z][a-z0-9_]*$/

const validateDataModel: CollectionBeforeValidateHook = async ({ data, operation, originalDoc, req }) => {
  if (!data) return data
  if (operation === 'update' && originalDoc) {
    data.project = originalDoc.project
    data.organizationId = originalDoc.organizationId
    data.argusProjectId = originalDoc.argusProjectId
  }

  const project = data.project ?? originalDoc?.project
  const scope = await resolveProjectScope(req, project)
  const slug = String(data.slug ?? originalDoc?.slug ?? '').trim().toLowerCase()
  if (!KEY_PATTERN.test(slug)) {
    throw new Error('Model slug must start with a lowercase letter and contain only a-z, 0-9 and underscore')
  }

  const duplicate = await req.payload.find({
    collection: 'data-models',
    depth: 0,
    limit: 1,
    overrideAccess: true,
    pagination: false,
    where: {
      and: [
        { project: { equals: scope.projectID } },
        { slug: { equals: slug } },
        ...(operation === 'update' && originalDoc?.id
          ? [{ id: { not_equals: originalDoc.id } }]
          : []),
      ],
    },
  })
  if (duplicate.docs.length > 0) {
    throw new Error(`Model slug '${slug}' already exists in this project`)
  }

  const fields = Array.isArray(data.fields) ? data.fields : (originalDoc?.fields ?? [])
  const keys = new Set<string>()
  for (const rawField of fields as Array<Record<string, unknown>>) {
    const key = String(rawField.key ?? '').trim().toLowerCase()
    if (!KEY_PATTERN.test(key)) {
      throw new Error(`Invalid field key '${key}'`)
    }
    if (keys.has(key)) {
      throw new Error(`Duplicate field key '${key}'`)
    }
    keys.add(key)
    rawField.key = key

    const type = String(rawField.type ?? '')
    const targetModelID = relationshipID(rawField.targetModel)
    if (type === 'relationship') {
      if (targetModelID === null) {
        throw new Error(`Relationship field '${key}' requires a target model`)
      }
      const targetModel = await req.payload.findByID({
        collection: 'data-models',
        id: targetModelID,
        depth: 0,
        overrideAccess: true,
      }) as { project?: unknown }
      if (relationshipID(targetModel.project) !== scope.projectID) {
        throw new Error(`Relationship field '${key}' cannot target a model in another project`)
      }
    } else {
      rawField.targetModel = null
      rawField.hasMany = false
    }
  }

  data.project = scope.projectID
  data.organizationId = scope.organizationId
  data.argusProjectId = scope.argusProjectId
  data.slug = slug
  data.fields = fields
  data.schemaVersion = operation === 'update'
    ? Number(originalDoc?.schemaVersion ?? 1) + 1
    : 1
  return data
}

export const DataModels: CollectionConfig = {
  slug: 'data-models',
  admin: {
    useAsTitle: 'name',
    group: 'App Data',
    defaultColumns: ['name', 'slug', 'kind', 'schemaVersion', 'status'],
  },
  access: {
    create: createProjectDocument('editor'),
    read: readProjectDocuments,
    update: editProjectDocuments,
    delete: manageProjectDocuments,
  },
  hooks: {
    beforeValidate: [validateDataModel],
  },
  fields: [
    { name: 'organizationId', type: 'text', required: true, index: true, admin: { readOnly: true } },
    { name: 'argusProjectId', type: 'text', required: true, index: true, admin: { readOnly: true } },
    {
      name: 'project',
      type: 'relationship',
      relationTo: 'project-spaces',
      required: true,
      index: true,
      admin: { description: 'Immutable after creation.' },
    },
    { name: 'name', type: 'text', required: true, maxLength: 160 },
    {
      name: 'slug',
      type: 'text',
      required: true,
      index: true,
      maxLength: 120,
      admin: {
        description: 'Stable API identifier inside the project, for example products or release_notes.',
      },
    },
    { name: 'description', type: 'textarea', maxLength: 4000 },
    {
      name: 'kind',
      type: 'select',
      required: true,
      defaultValue: 'data',
      options: [
        { label: 'Application data', value: 'data' },
        { label: 'Content', value: 'content' },
      ],
      admin: {
        description: 'Content models can later be surfaced by the visual CMS without changing the data substrate.',
      },
    },
    { name: 'schemaVersion', type: 'number', required: true, defaultValue: 1, admin: { readOnly: true } },
    {
      name: 'status',
      type: 'select',
      required: true,
      defaultValue: 'active',
      options: [
        { label: 'Active', value: 'active' },
        { label: 'Archived', value: 'archived' },
      ],
    },
    {
      name: 'fields',
      type: 'array',
      required: true,
      minRows: 1,
      fields: [
        { name: 'key', type: 'text', required: true, maxLength: 120 },
        { name: 'label', type: 'text', required: true, maxLength: 160 },
        {
          name: 'type',
          type: 'select',
          required: true,
          options: [
            { label: 'Text', value: 'text' },
            { label: 'Long text', value: 'textarea' },
            { label: 'Number', value: 'number' },
            { label: 'Boolean', value: 'boolean' },
            { label: 'Date', value: 'date' },
            { label: 'Date & time', value: 'datetime' },
            { label: 'JSON', value: 'json' },
            { label: 'Relationship', value: 'relationship' },
          ],
        },
        { name: 'required', type: 'checkbox', defaultValue: false },
        {
          name: 'hasMany',
          type: 'checkbox',
          defaultValue: false,
          admin: { condition: (_, siblingData) => siblingData?.type === 'relationship' },
        },
        {
          name: 'targetModel',
          type: 'relationship',
          relationTo: 'data-models',
          admin: { condition: (_, siblingData) => siblingData?.type === 'relationship' },
        },
        {
          name: 'settings',
          type: 'json',
          admin: { description: 'Reserved typed settings such as min/max, enum values or UI hints.' },
        },
      ],
    },
  ],
}
