import type { CollectionBeforeValidateHook, CollectionConfig, PayloadRequest } from 'payload'
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
  hasMany?: boolean
}

type PageBlock = { id?: unknown; component?: unknown; values?: unknown }
const UUID_PATTERN = /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i

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

function validateScalarValues(fields: ModelField[], values: unknown): Record<string, unknown> {
  if (!isPlainObject(values)) throw new Error('Record values must be a JSON object')
  const scalarFields = new Map(fields.filter((field) => !['relationship', 'media'].includes(String(field.type)) && field.key).map((field) => [field.key as string, field]))
  const mediaFields = new Map(fields.filter((field) => field.type === 'media' && field.key).map((field) => [field.key as string, field]))
  const relationshipKeys = new Set(fields.filter((field) => field.type === 'relationship' && field.key).map((field) => field.key as string))
  for (const key of Object.keys(values)) {
    if (relationshipKeys.has(key)) throw new Error(`Relationship field '${key}' must be stored in data-relations`)
    if (mediaFields.has(key)) continue
    const field = scalarFields.get(key)
    if (!field) throw new Error(`Unknown field '${key}' for this model`)
    if (values[key] !== null && !validScalar(String(field.type), values[key])) throw new Error(`Field '${key}' does not match type '${field.type}'`)
  }
  for (const [key, field] of scalarFields) {
    if (field.required && (values[key] === undefined || values[key] === null || values[key] === '')) throw new Error(`Required field '${key}' is missing`)
  }
  return values
}

async function validateMediaValues(req: PayloadRequest, projectID: string | number, fields: ModelField[], values: Record<string, unknown>) {
  let totalReferences = 0
  for (const field of fields.filter((candidate) => candidate.type === 'media' && candidate.key)) {
    const key = field.key as string
    const value = values[key]
    if (field.required && (value === undefined || value === null || value === '' || (Array.isArray(value) && value.length === 0))) throw new Error(`Required field '${key}' is missing`)
    if (value === undefined || value === null || value === '') continue
    const ids = field.hasMany ? (Array.isArray(value) ? value : []) : [value]
    if (ids.length === 0 || ids.length > 50 || ids.some((id) => typeof id !== 'string' || !UUID_PATTERN.test(id)) || new Set(ids).size !== ids.length) throw new Error(`Field '${key}' does not contain valid media references`)
    totalReferences += ids.length
    if (totalReferences > 100) throw new Error('A record or component block can reference at most 100 media assets')
    for (const id of ids as string[]) {
      const media = await req.payload.find({
        collection: 'media', depth: 0, limit: 1, overrideAccess: true, pagination: false, req,
        where: { and: [{ id: { equals: id } }, { project: { equals: projectID } }] },
      })
      if (media.docs.length !== 1) throw new Error(`Media reference '${key}' must belong to the same project`)
    }
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
    req,
  }) as {
    project?: unknown
    schemaVersion?: number
    status?: string
    kind?: string
    contentRole?: string
    allowedComponents?: unknown[]
    fields?: ModelField[]
  }
  if (relationshipID(model.project) !== scope.projectID) {
    throw new Error('Record model must belong to the same project')
  }
  if (model.status === 'archived') {
    throw new Error('Cannot write records for an archived model')
  }
  if (model.contentRole === 'component') throw new Error('Component schemas are embedded in pages and cannot have standalone records')

  const values = data.values ?? originalDoc?.values ?? {}
  const fields = model.fields ?? []
  const validatedValues = validateScalarValues(fields, values)
  await validateMediaValues(req, scope.projectID, fields, validatedValues)

  const layout = data.layout ?? originalDoc?.layout ?? []
  if (model.contentRole === 'page') {
    if (!Array.isArray(layout) || layout.length > 100) throw new Error('Page layout must contain at most 100 blocks')
    const componentIDs = (model.allowedComponents ?? []).map(relationshipID).filter((id): id is string | number => id !== null)
    const components = await Promise.all(componentIDs.map((id) => req.payload.findByID({
      collection: 'data-models', id, depth: 0, overrideAccess: true, req,
    }))) as Array<{ slug?: string; status?: string; kind?: string; contentRole?: string; project?: unknown; fields?: ModelField[] }>
    const bySlug = new Map(components.filter((component) => component.status !== 'archived' && component.kind === 'content' && component.contentRole === 'component' && relationshipID(component.project) === scope.projectID && component.slug).map((component) => [component.slug as string, component]))
    data.layout = await Promise.all((layout as PageBlock[]).map(async (block) => {
      if (!isPlainObject(block) || typeof block.id !== 'string' || !UUID_PATTERN.test(block.id) || typeof block.component !== 'string') throw new Error('Page block identity is invalid')
      const component = bySlug.get(block.component)
      if (!component) throw new Error(`Component '${block.component}' is not allowed by this page schema`)
      const blockValues = validateScalarValues(component.fields ?? [], block.values ?? {})
      await validateMediaValues(req, scope.projectID, component.fields ?? [], blockValues)
      return { id: block.id, component: block.component, values: blockValues }
    }))
  } else {
    data.layout = []
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
      name: 'layout',
      type: 'json',
      required: true,
      defaultValue: [],
      admin: { description: 'Validated component blocks for page content. Empty for normal collection records.' },
    },
    {
      name: 'status',
      type: 'select',
      enumName: 'enum_data_records_lifecycle_status',
      required: true,
      defaultValue: 'active',
      options: [
        { label: 'Active', value: 'active' },
        { label: 'Archived', value: 'archived' },
      ],
      admin: {
        description: 'Argus record lifecycle. Kept separate from Payload’s internal draft _status.',
      },
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
