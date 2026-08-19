import type { CollectionBeforeValidateHook, CollectionConfig } from 'payload'
import {
  createProjectDocument,
  editProjectDocuments,
  manageProjectDocuments,
  readProjectDocuments,
  relationshipID,
} from '@/access/projectAccess'
import { resolveProjectScope } from '@/lib/projectScope'

type ModelField = {
  key?: string
  type?: string
  required?: boolean
}

function isPlainObject(value: unknown): value is Record<string, unknown> {
  return Boolean(value && typeof value === 'object' && !Array.isArray(value))
}

function validScalar(type: string, value: unknown): boolean {
  switch (type) {
    case 'text':
    case 'textarea':
      return typeof value === 'string'
    case 'number':
      return typeof value === 'number' && Number.isFinite(value)
    case 'boolean':
      return typeof value === 'boolean'
    case 'date':
    case 'datetime':
      return typeof value === 'string' && !Number.isNaN(Date.parse(value))
    case 'json':
      return value !== undefined
    default:
      return false
  }
}

const validateRecord: CollectionBeforeValidateHook = async ({ data, operation, originalDoc, req }) => {
  if (!data) return data
  if (operation === 'update' && originalDoc) {
    data.project = originalDoc.project
    data.model = originalDoc.model
    data.organizationId = originalDoc.organizationId
    data.argusProjectId = originalDoc.argusProjectId
    data.createdBy = originalDoc.createdBy
  }

  const project = data.project ?? originalDoc?.project
  const modelValue = data.model ?? originalDoc?.model
  const modelID = relationshipID(modelValue)
  if (modelID === null) throw new Error('A data model is required')

  const scope = await resolveProjectScope(req, project)
  const model = await req.payload.findByID({
    collection: 'data-models',
    id: modelID,
    depth: 0,
    overrideAccess: true,
  }) as {
    project?: unknown
    schemaVersion?: number
    status?: string
    kind?: string
    fields?: ModelField[]
  }
  if (relationshipID(model.project) !== scope.projectID) {
    throw new Error('Record model must belong to the same project')
  }
  if (model.status === 'archived') {
    throw new Error('Cannot write records for an archived model')
  }

  const values = data.values ?? originalDoc?.values ?? {}
  if (!isPlainObject(values)) throw new Error('Record values must be a JSON object')
  const fields = model.fields ?? []
  const scalarFields = new Map(
    fields
      .filter((field) => field.type !== 'relationship' && field.key)
      .map((field) => [field.key as string, field]),
  )
  const relationshipKeys = new Set(
    fields
      .filter((field) => field.type === 'relationship' && field.key)
      .map((field) => field.key as string),
  )

  for (const key of Object.keys(values)) {
    if (relationshipKeys.has(key)) {
      throw new Error(`Relationship field '${key}' must be stored in data-relations`)
    }
    const field = scalarFields.get(key)
    if (!field) throw new Error(`Unknown field '${key}' for this model`)
    if (values[key] !== null && !validScalar(String(field.type), values[key])) {
      throw new Error(`Field '${key}' does not match type '${field.type}'`)
    }
  }
  for (const [key, field] of scalarFields) {
    if (field.required && (values[key] === undefined || values[key] === null || values[key] === '')) {
      throw new Error(`Required field '${key}' is missing`)
    }
  }

  data.project = scope.projectID
  data.model = modelID
  data.organizationId = scope.organizationId
  data.argusProjectId = scope.argusProjectId
  data.schemaVersion = Number(model.schemaVersion ?? 1)

  if (model.kind !== 'content') {
    data._status = 'published'
  } else if (data._status === 'published' && originalDoc?._status !== 'published') {
    data.publishedAt = new Date().toISOString()
  }

  if (operation === 'create' && req.user) {
    data.createdBy = req.user.id
  }
  return data
}

export const DataRecords: CollectionConfig = {
  slug: 'data-records',
  admin: {
    group: 'App Data',
    defaultColumns: ['model', '_status', 'publishedAt', 'status', 'schemaVersion', 'updatedAt'],
    description: 'Application data publishes immediately. Content models support Payload drafts and publication history.',
  },
  access: {
    create: createProjectDocument('editor'),
    read: readProjectDocuments,
    update: editProjectDocuments,
    delete: manageProjectDocuments,
  },
  hooks: {
    beforeValidate: [validateRecord],
  },
  versions: {
    drafts: true,
    maxPerDoc: 50,
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
    {
      name: 'model',
      type: 'relationship',
      relationTo: 'data-models',
      required: true,
      index: true,
      admin: { description: 'Immutable after creation.' },
    },
    { name: 'schemaVersion', type: 'number', required: true, admin: { readOnly: true } },
    {
      name: 'values',
      type: 'json',
      required: true,
      defaultValue: {},
      admin: {
        description: 'Scalar values validated against the selected model. Relationships are stored separately.',
      },
    },
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
      name: 'publishedAt',
      type: 'date',
      admin: {
        position: 'sidebar',
        readOnly: true,
        description: 'Set when a content record is published for the first time after its latest draft state.',
      },
    },
    {
      name: 'createdBy',
      type: 'relationship',
      relationTo: 'workspace-users',
      admin: { readOnly: true },
    },
  ],
}
