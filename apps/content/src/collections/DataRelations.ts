import type { CollectionBeforeValidateHook, CollectionConfig } from 'payload'
import {
  createProjectDocument,
  editProjectDocuments,
  readProjectDocuments,
  relationshipID,
} from '@/access/projectAccess'
import { resolveProjectScope } from '@/lib/projectScope'

type RelationField = {
  key?: string
  type?: string
  hasMany?: boolean
  targetModel?: unknown
}

const validateRelation: CollectionBeforeValidateHook = async ({ data, req }) => {
  if (!data) return data
  const project = data.project
  const sourceRecordID = relationshipID(data.sourceRecord)
  const targetRecordID = relationshipID(data.targetRecord)
  const fieldKey = String(data.fieldKey ?? '').trim().toLowerCase()
  if (sourceRecordID === null || targetRecordID === null || !fieldKey) {
    throw new Error('Source record, target record and field key are required')
  }

  const scope = await resolveProjectScope(req, project)
  const [sourceRecord, targetRecord] = await Promise.all([
    req.payload.findByID({
      collection: 'data-records',
      id: sourceRecordID,
      depth: 0,
      overrideAccess: true,
    }),
    req.payload.findByID({
      collection: 'data-records',
      id: targetRecordID,
      depth: 0,
      overrideAccess: true,
    }),
  ]) as Array<{ project?: unknown; model?: unknown }>

  const sourceModelID = relationshipID(sourceRecord.model)
  const targetModelID = relationshipID(targetRecord.model)
  if (
    relationshipID(sourceRecord.project) !== scope.projectID ||
    relationshipID(targetRecord.project) !== scope.projectID ||
    sourceModelID === null ||
    targetModelID === null
  ) {
    throw new Error('Relationships cannot cross project boundaries')
  }

  const sourceModel = await req.payload.findByID({
    collection: 'data-models',
    id: sourceModelID,
    depth: 0,
    overrideAccess: true,
  }) as { fields?: RelationField[] }
  const field = (sourceModel.fields ?? []).find(
    (candidate) => candidate.key === fieldKey && candidate.type === 'relationship',
  )
  if (!field) {
    throw new Error(`'${fieldKey}' is not a relationship field on the source model`)
  }
  if (relationshipID(field.targetModel) !== targetModelID) {
    throw new Error(`Relationship '${fieldKey}' points to a different target model`)
  }

  const duplicate = await req.payload.find({
    collection: 'data-relations',
    depth: 0,
    limit: 1,
    overrideAccess: true,
    pagination: false,
    where: {
      and: [
        { sourceRecord: { equals: sourceRecordID } },
        { fieldKey: { equals: fieldKey } },
        { targetRecord: { equals: targetRecordID } },
      ],
    },
  })
  if (duplicate.docs.length > 0) {
    throw new Error(`Relationship '${fieldKey}' already contains this target`)
  }

  if (!field.hasMany) {
    const existing = await req.payload.find({
      collection: 'data-relations',
      depth: 0,
      limit: 1,
      overrideAccess: true,
      pagination: false,
      where: {
        and: [
          { sourceRecord: { equals: sourceRecordID } },
          { fieldKey: { equals: fieldKey } },
        ],
      },
    })
    if (existing.docs.length > 0) {
      throw new Error(`Relationship '${fieldKey}' accepts only one target`)
    }
  }

  data.project = scope.projectID
  data.organizationId = scope.organizationId
  data.argusProjectId = scope.argusProjectId
  data.sourceModel = sourceModelID
  data.targetModel = targetModelID
  data.sourceRecord = sourceRecordID
  data.targetRecord = targetRecordID
  data.fieldKey = fieldKey
  return data
}

export const DataRelations: CollectionConfig = {
  slug: 'data-relations',
  admin: {
    group: 'App Data',
    defaultColumns: ['sourceModel', 'fieldKey', 'targetModel', 'updatedAt'],
    description: 'Relationship endpoints are immutable. Delete and recreate an edge to change it.',
  },
  access: {
    create: createProjectDocument('editor'),
    read: readProjectDocuments,
    update: () => false,
    delete: editProjectDocuments,
  },
  hooks: {
    beforeValidate: [validateRelation],
  },
  fields: [
    { name: 'organizationId', type: 'text', required: true, index: true, admin: { readOnly: true } },
    { name: 'argusProjectId', type: 'text', required: true, index: true, admin: { readOnly: true } },
    { name: 'project', type: 'relationship', relationTo: 'project-spaces', required: true, index: true },
    { name: 'sourceModel', type: 'relationship', relationTo: 'data-models', required: true, index: true, admin: { readOnly: true } },
    { name: 'sourceRecord', type: 'relationship', relationTo: 'data-records', required: true, index: true },
    { name: 'fieldKey', type: 'text', required: true, index: true, maxLength: 120 },
    { name: 'targetModel', type: 'relationship', relationTo: 'data-models', required: true, index: true, admin: { readOnly: true } },
    { name: 'targetRecord', type: 'relationship', relationTo: 'data-records', required: true, index: true },
  ],
}
