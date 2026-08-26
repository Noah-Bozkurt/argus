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

function fieldShape(rawField: Record<string, unknown>) {
  return {
    key: String(rawField.key ?? '').trim().toLowerCase(),
    label: String(rawField.label ?? ''),
    type: String(rawField.type ?? ''),
    required: rawField.required === true,
    hasMany: rawField.hasMany === true,
    targetModel: relationshipID(rawField.targetModel),
    settings: rawField.settings ?? null,
  }
}

function componentShape(values: unknown[]) {
  return values.map(relationshipID).filter((id): id is string | number => id !== null).map(String).sort()
}

const validateDataModel: CollectionBeforeValidateHook = async ({ data, operation, originalDoc, req }) => {
  if (!data) return data
  if (operation === 'update' && originalDoc) {
    data.project = originalDoc.project
    data.organizationId = originalDoc.organizationId
    data.argusProjectId = originalDoc.argusProjectId
    data.slug = originalDoc.slug
    data.kind = originalDoc.kind
    data.contentRole = originalDoc.contentRole
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
      if (type !== 'media') rawField.hasMany = false
    }
  }

  const kind = String(data.kind ?? originalDoc?.kind ?? 'data')
  const contentRole = kind === 'content' ? String(data.contentRole ?? originalDoc?.contentRole ?? 'collection') : 'collection'
  if (!['collection', 'page', 'component'].includes(contentRole)) {
    throw new Error('Invalid content role')
  }
  const allowedComponents = contentRole === 'page' && Array.isArray(data.allowedComponents)
    ? data.allowedComponents.map(relationshipID).filter((id): id is string | number => id !== null)
    : contentRole === 'page' && Array.isArray(originalDoc?.allowedComponents)
      ? originalDoc.allowedComponents.map(relationshipID).filter((id): id is string | number => id !== null)
      : []
  if (new Set(allowedComponents.map(String)).size !== allowedComponents.length) {
    throw new Error('Allowed component schemas must be unique')
  }
  for (const componentID of allowedComponents) {
    const component = await req.payload.findByID({
      collection: 'data-models', id: componentID, depth: 0, overrideAccess: true,
    }) as { project?: unknown; kind?: string; contentRole?: string }
    if (relationshipID(component.project) !== scope.projectID || component.kind !== 'content' || component.contentRole !== 'component') {
      throw new Error('Page schemas can only allow component schemas from the same project')
    }
  }

  const schemaChanged = operation === 'update' && originalDoc
    ? JSON.stringify((fields as Array<Record<string, unknown>>).map(fieldShape)) !== JSON.stringify(((originalDoc.fields ?? []) as Array<Record<string, unknown>>).map(fieldShape))
      || JSON.stringify(componentShape(allowedComponents)) !== JSON.stringify(componentShape(Array.isArray(originalDoc.allowedComponents) ? originalDoc.allowedComponents : []))
    : true

  data.project = scope.projectID
  data.organizationId = scope.organizationId
  data.argusProjectId = scope.argusProjectId
  data.slug = slug
  data.fields = fields
  data.contentRole = contentRole
  data.allowedComponents = allowedComponents
  data.publicRead = kind === 'content' && contentRole !== 'component' && data.publicRead === true
  data.schemaVersion = operation === 'update'
    ? Number(originalDoc?.schemaVersion ?? 1) + (schemaChanged ? 1 : 0)
    : 1
  return data
}

export const DataModels: CollectionConfig = {
  slug: 'data-models',
  admin: {
    useAsTitle: 'name',
    group: 'App Data',
    defaultColumns: ['name', 'slug', 'kind', 'publicRead', 'schemaVersion', 'status'],
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
        description: 'Stable immutable API identifier inside the project, for example products or release_notes.',
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
        description: 'Immutable after creation. Content models support drafts/publication and the public content API.',
      },
    },
    {
      name: 'publicRead',
      type: 'checkbox',
      defaultValue: false,
      admin: {
        condition: (_, siblingData) => siblingData?.kind === 'content',
        description: 'Expose only published active records through the read-only public CMS API.',
      },
    },
    {
      name: 'contentRole',
      type: 'select',
      required: true,
      defaultValue: 'collection',
      options: [
        { label: 'Collection', value: 'collection' },
        { label: 'Page', value: 'page' },
        { label: 'Component schema', value: 'component' },
      ],
      admin: { description: 'Immutable content shape. Page records can contain blocks defined by component schemas.' },
    },
    {
      name: 'allowedComponents',
      type: 'relationship',
      relationTo: 'data-models',
      hasMany: true,
      admin: { condition: (_, siblingData) => siblingData?.kind === 'content' && siblingData?.contentRole === 'page' },
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
            { label: 'Media', value: 'media' },
          ],
        },
        { name: 'required', type: 'checkbox', defaultValue: false },
        {
          name: 'hasMany',
          type: 'checkbox',
          defaultValue: false,
          admin: { condition: (_, siblingData) => ['relationship', 'media'].includes(siblingData?.type) },
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
