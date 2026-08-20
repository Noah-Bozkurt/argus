import config from '@payload-config'
import { NextResponse } from 'next/server'
import { getPayload, type Payload } from 'payload'

import { internalIdentity, normalizeModelInput, type CmsFieldInput, type ContentRole, validateValues } from '@/lib/argusCmsContract'
import { isUUID } from '@/lib/projectScope'

type Project = { id: string; organizationId?: string; status?: string }
type Model = {
  id: string
  kind?: 'data' | 'content'
  name?: string
  slug?: string
  description?: string | null
  publicRead?: boolean | null
  contentRole?: ContentRole | null
  allowedComponents?: unknown[] | null
  schemaVersion?: number
  status?: string
  fields?: Array<CmsFieldInput & { id?: string | null; targetModel?: unknown }>
}

async function projectFor(payload: Payload, projectId: string, organizationId: string): Promise<Project | null> {
  const result = await payload.find({
    collection: 'project-spaces', depth: 0, limit: 1, overrideAccess: true, pagination: false,
    where: { and: [{ argusProjectId: { equals: projectId } }, { organizationId: { equals: organizationId } }] },
  })
  return (result.docs[0] as Project | undefined) ?? null
}

function routeKind(request: Request): 'data' | 'content' {
  return new URL(request.url).pathname.includes('/internal/argus/data/') ? 'data' : 'content'
}

async function modelFor(payload: Payload, project: Project, modelId: string, kind: 'data' | 'content'): Promise<Model | null> {
  if (!isUUID(modelId)) return null
  const result = await payload.find({
    collection: 'data-models', depth: 0, limit: 1, overrideAccess: true, pagination: false,
    where: { and: [{ id: { equals: modelId } }, { project: { equals: project.id } }, { kind: { equals: kind } }] },
  })
  return (result.docs[0] as Model | undefined) ?? null
}

function modelView(model: Model) {
  return {
    id: model.id, name: model.name ?? '', slug: model.slug ?? '', description: model.description ?? '', kind: model.kind ?? 'content',
    public_read: model.publicRead === true, schema_version: model.schemaVersion ?? 1,
    content_role: model.contentRole ?? 'collection',
    allowed_component_ids: (model.allowedComponents ?? []).map((value) => typeof value === 'object' && value && 'id' in value ? String(value.id) : String(value)),
    status: model.status ?? 'active',
    fields: (model.fields ?? []).map(({ key, label, type, required, targetModel, hasMany }) => ({ key, label, type, required: required === true,
      target_model_id: typeof targetModel === 'object' && targetModel && 'id' in targetModel ? String((targetModel as { id: unknown }).id) : targetModel ?? null, has_many: hasMany === true })),
  }
}

export async function GET(request: Request, { params }: { params: Promise<{ projectId: string }> }) {
  const identity = internalIdentity(request)
  const { projectId } = await params
  if (!identity) return NextResponse.json({ code: 'PERMISSION_DENIED' }, { status: 401 })
  if (!isUUID(projectId)) return NextResponse.json({ code: 'NOT_FOUND' }, { status: 404 })
  const payload = await getPayload({ config })
  const kind = routeKind(request)
  const project = await projectFor(payload, projectId, identity.organizationId)
  if (!project) return NextResponse.json({ code: 'NOT_FOUND' }, { status: 404 })

  const models = await payload.find({
    collection: 'data-models', depth: 0, limit: 100, overrideAccess: true, pagination: false,
    sort: 'name', where: { and: [{ project: { equals: project.id } }, { kind: { equals: kind } }] },
  })
  const modelIds = models.docs.filter((model) => (model as Model).contentRole !== 'component').map((model) => String(model.id))
  const records = modelIds.length === 0 ? { docs: [] } : await payload.find({
    collection: 'data-records', depth: 0, draft: true, limit: 500, overrideAccess: true, pagination: false,
    sort: '-updatedAt', where: { and: [{ project: { equals: project.id } }, { model: { in: modelIds } }] },
  })
  const recordIds = records.docs.map((record) => String(record.id))
  const relations = recordIds.length === 0 ? { docs: [] } : await payload.find({
    collection: 'data-relations', depth: 0, limit: 1000, overrideAccess: true, pagination: false,
    where: { and: [{ project: { equals: project.id } }, { sourceRecord: { in: recordIds } }] },
  })
  return NextResponse.json({
    project_status: project.status ?? 'active',
    models: models.docs.map((model) => modelView(model as Model)),
    records: records.docs.map((record) => {
      const doc = record as { id: string | number; model?: unknown; values?: unknown; layout?: unknown; _status?: string; status?: string; publishedAt?: string | null; updatedAt?: string }
      const modelId = typeof doc.model === 'object' && doc.model && 'id' in doc.model ? String(doc.model.id) : String(doc.model ?? '')
      return { id: doc.id, model_id: modelId, values: doc.values ?? {}, layout: Array.isArray(doc.layout) ? doc.layout : [], editorial_status: doc._status ?? 'draft', lifecycle_status: doc.status ?? 'active', published_at: doc.publishedAt ?? null, updated_at: doc.updatedAt ?? null }
    }),
    relations: relations.docs.map((relation) => {
      const doc = relation as { id: string; sourceRecord?: unknown; targetRecord?: unknown; fieldKey?: string }
      const idOf = (value: unknown) => typeof value === 'object' && value && 'id' in value ? String(value.id) : String(value ?? '')
      return { id: doc.id, source_record_id: idOf(doc.sourceRecord), target_record_id: idOf(doc.targetRecord), field_key: doc.fieldKey ?? '' }
    }),
  })
}

export async function POST(request: Request, { params }: { params: Promise<{ projectId: string }> }) {
  const identity = internalIdentity(request)
  const { projectId } = await params
  if (!identity) return NextResponse.json({ code: 'PERMISSION_DENIED' }, { status: 401 })
  if (!isUUID(projectId)) return NextResponse.json({ code: 'NOT_FOUND' }, { status: 404 })
  const body = await request.json().catch(() => null) as Record<string, unknown> | null
  if (!body || typeof body.operation !== 'string') return NextResponse.json({ code: 'INVALID_REQUEST' }, { status: 400 })
  const payload = await getPayload({ config })
  const kind = routeKind(request)
  const project = await projectFor(payload, projectId, identity.organizationId)
  if (!project || project.status !== 'active') return NextResponse.json({ code: 'NOT_FOUND' }, { status: 404 })

  try {
    if (body.operation === 'create_model') {
      const normalized = normalizeModelInput(body.model)
      if (!normalized) return NextResponse.json({ code: 'INVALID_REQUEST' }, { status: 400 })
      if (kind === 'data' && normalized.contentRole !== 'collection') return NextResponse.json({ code: 'INVALID_REQUEST' }, { status: 400 })
      const model = await payload.create({
        collection: 'data-models', depth: 0, draft: false, overrideAccess: true,
        data: { organizationId: identity.organizationId, argusProjectId: projectId, project: project.id, name: normalized.name, slug: normalized.slug, description: normalized.description, kind, contentRole: kind === 'data' ? 'collection' : normalized.contentRole, allowedComponents: kind === 'data' ? [] : normalized.allowedComponentIds, publicRead: kind === 'content' && normalized.publicRead, schemaVersion: 1, status: 'active', fields: normalized.fields },
      })
      return NextResponse.json({ model: modelView(model as Model) }, { status: 201 })
    }

    if (body.operation === 'save_record') {
      const modelId = typeof body.model_id === 'string' ? body.model_id : ''
      const model = await modelFor(payload, project, modelId, kind)
      if (!model || model.status !== 'active') return NextResponse.json({ code: 'NOT_FOUND' }, { status: 404 })
      const fields = (model.fields ?? []).map(({ key, label, type, required, targetModel, hasMany }) => ({ key, label, type, required: required === true,
        targetModel: typeof targetModel === 'object' && targetModel && 'id' in targetModel ? String((targetModel as { id: unknown }).id) : String(targetModel ?? ''), hasMany }))
      const values = validateValues(fields, body.values)
      if (!values) return NextResponse.json({ code: 'INVALID_RECORD' }, { status: 400 })
      const requestedRelations = body.relationships && typeof body.relationships === 'object' && !Array.isArray(body.relationships)
        ? body.relationships as Record<string, unknown> : {}
      const relationTargets = new Map<string, string[]>()
      for (const field of fields.filter((candidate) => candidate.type === 'relationship')) {
        const raw = requestedRelations[field.key]
        const ids = (Array.isArray(raw) ? raw : raw ? [raw] : []).filter((id): id is string => typeof id === 'string' && isUUID(id))
        if (ids.length !== (Array.isArray(raw) ? raw.length : raw ? 1 : 0) || (!field.hasMany && ids.length > 1) || (field.required && ids.length === 0) || new Set(ids).size !== ids.length) {
          return NextResponse.json({ code: 'INVALID_RELATIONSHIPS' }, { status: 400 })
        }
        for (const targetId of ids) {
          const target = await payload.find({ collection: 'data-records', depth: 0, draft: true, limit: 1, overrideAccess: true, pagination: false,
            where: { and: [{ id: { equals: targetId } }, { project: { equals: project.id } }, { model: { equals: field.targetModel } }, { status: { equals: 'active' } }] } })
          if (target.docs.length !== 1) return NextResponse.json({ code: 'INVALID_RELATIONSHIPS' }, { status: 400 })
        }
        relationTargets.set(field.key, ids)
      }
      const publish = kind === 'data' || body.publish === true
      const layout = Array.isArray(body.layout) ? body.layout : []
      const data = { organizationId: identity.organizationId, argusProjectId: projectId, project: project.id, model: model.id, schemaVersion: model.schemaVersion ?? 1, values, layout, status: 'active' as const, _status: publish ? 'published' as const : 'draft' as const }
      const recordId = typeof body.record_id === 'string' ? body.record_id : ''
      if (recordId) {
        if (!isUUID(recordId)) return NextResponse.json({ code: 'NOT_FOUND' }, { status: 404 })
        const existing = await payload.find({
          collection: 'data-records', depth: 0, draft: true, limit: 1, overrideAccess: true, pagination: false,
          where: { and: [{ id: { equals: recordId } }, { project: { equals: project.id } }, { model: { equals: model.id } }] },
        })
        if (existing.docs.length !== 1) return NextResponse.json({ code: 'NOT_FOUND' }, { status: 404 })
      }
      const saved = recordId
        ? publish
          ? await payload.update({ collection: 'data-records', id: recordId, depth: 0, draft: false, overrideAccess: true, data })
          : await payload.update({ collection: 'data-records', id: recordId, depth: 0, draft: true, overrideAccess: true, data })
        : publish
          ? await payload.create({ collection: 'data-records', depth: 0, draft: false, overrideAccess: true, data })
          : await payload.create({ collection: 'data-records', depth: 0, draft: true, overrideAccess: true, data })
      const record = saved as { id: string; values?: unknown; layout?: unknown; _status?: string; publishedAt?: string | null; updatedAt?: string }
      const existingRelations = await payload.find({ collection: 'data-relations', depth: 0, limit: 1000, overrideAccess: true, pagination: false,
        where: { and: [{ project: { equals: project.id } }, { sourceRecord: { equals: record.id } }] } })
      for (const relation of existingRelations.docs) await payload.delete({ collection: 'data-relations', id: relation.id, overrideAccess: true })
      for (const [fieldKey, targetIds] of relationTargets) for (const targetRecord of targetIds) await payload.create({
        collection: 'data-relations', depth: 0, overrideAccess: true,
        data: { organizationId: identity.organizationId, argusProjectId: projectId, project: project.id,
          sourceModel: model.id, sourceRecord: record.id, targetModel: fields.find((field) => field.key === fieldKey)?.targetModel ?? '', targetRecord, fieldKey },
      })
      return NextResponse.json({ record: { id: record.id, model_id: model.id, values: record.values ?? {}, layout: Array.isArray(record.layout) ? record.layout : [], editorial_status: record._status ?? (publish ? 'published' : 'draft'), published_at: record.publishedAt ?? null, updated_at: record.updatedAt } }, { status: recordId ? 200 : 201 })
    }
  } catch (error) {
    console.error('Argus CMS operation failed', { operation: body.operation, projectId, userId: identity.userId, error })
    return NextResponse.json({ code: 'CONTENT_OPERATION_FAILED' }, { status: 409 })
  }
  return NextResponse.json({ code: 'INVALID_REQUEST' }, { status: 400 })
}
