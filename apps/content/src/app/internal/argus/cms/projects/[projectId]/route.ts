import config from '@payload-config'
import { NextResponse } from 'next/server'
import { getPayload, type Payload } from 'payload'

import { internalIdentity, normalizeModelInput, type CmsFieldInput, type ContentRole, validateValues } from '@/lib/argusCmsContract'
import { authorizeInternalProject } from '@/lib/internalProjectAccess'
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

type WorkspaceUser = { id: string | number }

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

function recordView(record: Record<string, unknown>) {
  const model = record.model
  const modelId = typeof model === 'object' && model && 'id' in model ? String(model.id) : String(model ?? '')
  return {
    id: String(record.id), model_id: modelId, values: record.values ?? {}, layout: Array.isArray(record.layout) ? record.layout : [],
    editorial_status: record._status ?? 'draft', lifecycle_status: record.status ?? 'active',
    published_at: record.publishedAt ?? null, updated_at: record.updatedAt ?? null,
  }
}

async function requireRole(payload: Payload, identity: NonNullable<ReturnType<typeof internalIdentity>>, project: Project, role: 'viewer' | 'editor' | 'manager') {
  return authorizeInternalProject(payload, identity, project, role)
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
  if (!await requireRole(payload, identity, project, 'viewer')) return NextResponse.json({ code: 'PERMISSION_DENIED' }, { status: 403 })

  const models = await payload.find({
    collection: 'data-models', depth: 0, limit: 100, overrideAccess: true, pagination: false,
    sort: 'name', where: { and: [{ project: { equals: project.id } }, { kind: { equals: kind } }] },
  })
  const modelIds = models.docs.filter((model) => (model as Model).contentRole !== 'component').map((model) => String(model.id))
  const requestedPage = Number.parseInt(new URL(request.url).searchParams.get('record_page') ?? '1', 10)
  const recordPage = Number.isFinite(requestedPage) ? Math.max(1, requestedPage) : 1
  const records = modelIds.length === 0
    ? { docs: [], page: 1, totalPages: 0, totalDocs: 0, hasNextPage: false, hasPrevPage: false }
    : await payload.find({
        collection: 'data-records', depth: 0, draft: true, limit: 100, page: recordPage, overrideAccess: true,
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
    records: records.docs.map((record) => recordView(record as unknown as Record<string, unknown>)),
    relations: relations.docs.map((relation) => {
      const doc = relation as { id: string; sourceRecord?: unknown; targetRecord?: unknown; fieldKey?: string }
      const idOf = (value: unknown) => typeof value === 'object' && value && 'id' in value ? String(value.id) : String(value ?? '')
      return { id: doc.id, source_record_id: idOf(doc.sourceRecord), target_record_id: idOf(doc.targetRecord), field_key: doc.fieldKey ?? '' }
    }),
    pagination: { records: { page: records.page ?? 1, total_pages: records.totalPages ?? 0, total_docs: records.totalDocs ?? 0, has_next_page: records.hasNextPage ?? false, has_prev_page: records.hasPrevPage ?? false } },
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
    if (body.operation === 'create_model' || body.operation === 'update_model') {
      const actor = await requireRole(payload, identity, project, 'editor')
      if (!actor) return NextResponse.json({ code: 'PERMISSION_DENIED' }, { status: 403 })
      const normalized = normalizeModelInput(body.model)
      if (!normalized) return NextResponse.json({ code: 'INVALID_REQUEST' }, { status: 400 })
      if (kind === 'data' && normalized.contentRole !== 'collection') return NextResponse.json({ code: 'INVALID_REQUEST' }, { status: 400 })

      if (body.operation === 'update_model') {
        const modelId = typeof body.model_id === 'string' ? body.model_id : ''
        const existing = await modelFor(payload, project, modelId, kind)
        if (!existing) return NextResponse.json({ code: 'NOT_FOUND' }, { status: 404 })
        if (normalized.slug !== existing.slug || normalized.contentRole !== (existing.contentRole ?? 'collection')) {
          return NextResponse.json({ code: 'IMMUTABLE_MODEL_SHAPE' }, { status: 409 })
        }
        const model = await payload.update({
          collection: 'data-models', id: modelId, depth: 0, overrideAccess: true,
          data: { name: normalized.name, description: normalized.description, allowedComponents: kind === 'data' ? [] : normalized.allowedComponentIds,
            publicRead: kind === 'content' && normalized.publicRead, fields: normalized.fields },
          user: actor as any,
        })
        return NextResponse.json({ model: modelView(model as Model) })
      }

      const model = await payload.create({
        collection: 'data-models', depth: 0, draft: false, overrideAccess: true,
        data: { organizationId: identity.organizationId, argusProjectId: projectId, project: project.id, name: normalized.name, slug: normalized.slug, description: normalized.description, kind, contentRole: kind === 'data' ? 'collection' : normalized.contentRole, allowedComponents: kind === 'data' ? [] : normalized.allowedComponentIds, publicRead: kind === 'content' && normalized.publicRead, schemaVersion: 1, status: 'active', fields: normalized.fields },
        user: actor as any,
      })
      return NextResponse.json({ model: modelView(model as Model) }, { status: 201 })
    }

    if (body.operation === 'set_model_status' || body.operation === 'delete_model') {
      const actor = await requireRole(payload, identity, project, 'manager')
      if (!actor) return NextResponse.json({ code: 'PERMISSION_DENIED' }, { status: 403 })
      const modelId = typeof body.model_id === 'string' ? body.model_id : ''
      const model = await modelFor(payload, project, modelId, kind)
      if (!model) return NextResponse.json({ code: 'NOT_FOUND' }, { status: 404 })
      if (body.operation === 'set_model_status') {
        const status = body.status === 'active' || body.status === 'archived' ? body.status : null
        if (!status) return NextResponse.json({ code: 'INVALID_REQUEST' }, { status: 400 })
        const updated = await payload.update({ collection: 'data-models', id: model.id, depth: 0, overrideAccess: true, data: { status }, user: actor as any })
        return NextResponse.json({ model: modelView(updated as Model) })
      }
      const records = await payload.find({ collection: 'data-records', depth: 0, draft: true, limit: 1, overrideAccess: true, pagination: false,
        where: { and: [{ project: { equals: project.id } }, { model: { equals: model.id } }] } })
      if (records.docs.length > 0) return NextResponse.json({ code: 'MODEL_NOT_EMPTY' }, { status: 409 })
      await payload.delete({ collection: 'data-models', id: model.id, overrideAccess: true, user: actor as any })
      return new NextResponse(null, { status: 204 })
    }

    if (body.operation === 'set_record_status' || body.operation === 'delete_record') {
      const requiredRole = body.operation === 'delete_record' ? 'manager' : 'editor'
      const actor = await requireRole(payload, identity, project, requiredRole)
      if (!actor) return NextResponse.json({ code: 'PERMISSION_DENIED' }, { status: 403 })
      const recordId = typeof body.record_id === 'string' ? body.record_id : ''
      if (!isUUID(recordId)) return NextResponse.json({ code: 'NOT_FOUND' }, { status: 404 })
      const existing = await payload.find({ collection: 'data-records', depth: 0, draft: true, limit: 1, overrideAccess: true, pagination: false,
        where: { and: [{ id: { equals: recordId } }, { project: { equals: project.id } }] } })
      if (existing.docs.length !== 1) return NextResponse.json({ code: 'NOT_FOUND' }, { status: 404 })
      if (body.operation === 'set_record_status') {
        const status = body.status === 'active' || body.status === 'archived' ? body.status : null
        if (!status) return NextResponse.json({ code: 'INVALID_REQUEST' }, { status: 400 })
        const updated = await payload.update({ collection: 'data-records', id: recordId, depth: 0, draft: true, overrideAccess: true, data: { status }, user: actor as any })
        return NextResponse.json({ record: recordView(updated as unknown as Record<string, unknown>) })
      }
      const transactionID = await payload.db.beginTransaction()
      try {
        const req = { transactionID, user: actor } as any
        const relations = await payload.find({ collection: 'data-relations', depth: 0, limit: 1000, overrideAccess: true, pagination: false, req,
          where: { and: [{ project: { equals: project.id } }, { or: [{ sourceRecord: { equals: recordId } }, { targetRecord: { equals: recordId } }] }] } })
        for (const relation of relations.docs) await payload.delete({ collection: 'data-relations', id: relation.id, overrideAccess: true, req })
        await payload.delete({ collection: 'data-records', id: recordId, overrideAccess: true, req })
        await payload.db.commitTransaction(transactionID)
      } catch (error) {
        await payload.db.rollbackTransaction(transactionID)
        throw error
      }
      return new NextResponse(null, { status: 204 })
    }

    if (body.operation === 'save_record') {
      const actor = await requireRole(payload, identity, project, 'editor') as WorkspaceUser | null
      if (!actor) return NextResponse.json({ code: 'PERMISSION_DENIED' }, { status: 403 })
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
        const existing = await payload.find({ collection: 'data-records', depth: 0, draft: true, limit: 1, overrideAccess: true, pagination: false,
          where: { and: [{ id: { equals: recordId } }, { project: { equals: project.id } }, { model: { equals: model.id } }] } })
        if (existing.docs.length !== 1) return NextResponse.json({ code: 'NOT_FOUND' }, { status: 404 })
      }

      const transactionID = await payload.db.beginTransaction()
      try {
        const req = { transactionID, user: actor } as any
        const saved = recordId
          ? publish
            ? await payload.update({ collection: 'data-records', id: recordId, depth: 0, draft: false, overrideAccess: true, data, req })
            : await payload.update({ collection: 'data-records', id: recordId, depth: 0, draft: true, overrideAccess: true, data, req })
          : publish
            ? await payload.create({ collection: 'data-records', depth: 0, draft: false, overrideAccess: true, data, req })
            : await payload.create({ collection: 'data-records', depth: 0, draft: true, overrideAccess: true, data, req })
        const record = saved as unknown as Record<string, unknown>
        const savedId = String(record.id)
        const existingRelations = await payload.find({ collection: 'data-relations', depth: 0, limit: 1000, overrideAccess: true, pagination: false, req,
          where: { and: [{ project: { equals: project.id } }, { sourceRecord: { equals: savedId } }] } })
        for (const relation of existingRelations.docs) await payload.delete({ collection: 'data-relations', id: relation.id, overrideAccess: true, req })
        for (const [fieldKey, targetIds] of relationTargets) for (const targetRecord of targetIds) await payload.create({
          collection: 'data-relations', depth: 0, overrideAccess: true, req,
          data: { organizationId: identity.organizationId, argusProjectId: projectId, project: project.id,
            sourceModel: model.id, sourceRecord: savedId, targetModel: fields.find((field) => field.key === fieldKey)?.targetModel ?? '', targetRecord, fieldKey },
        })
        await payload.db.commitTransaction(transactionID)
        return NextResponse.json({ record: recordView({ ...record, model: model.id }) }, { status: recordId ? 200 : 201 })
      } catch (error) {
        await payload.db.rollbackTransaction(transactionID)
        throw error
      }
    }
  } catch (error) {
    console.error('Argus CMS operation failed', { operation: body.operation, projectId, userId: identity.userId, workspaceUserId: identity.workspaceUserId, error })
    return NextResponse.json({ code: 'CONTENT_OPERATION_FAILED' }, { status: 409 })
  }
  return NextResponse.json({ code: 'INVALID_REQUEST' }, { status: 400 })
}
