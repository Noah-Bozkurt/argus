import config from '@payload-config'
import { NextResponse } from 'next/server'
import { getPayload, type Payload } from 'payload'

import { internalIdentity, normalizeModelInput, type CmsFieldInput, validateValues } from '@/lib/argusCmsContract'
import { isUUID } from '@/lib/projectScope'

type Project = { id: string; organizationId?: string; status?: string }
type Model = {
  id: string
  name?: string
  slug?: string
  description?: string | null
  publicRead?: boolean | null
  schemaVersion?: number
  status?: string
  fields?: Array<CmsFieldInput & { id?: string | null }>
}

async function projectFor(payload: Payload, projectId: string, organizationId: string): Promise<Project | null> {
  const result = await payload.find({
    collection: 'project-spaces', depth: 0, limit: 1, overrideAccess: true, pagination: false,
    where: { and: [{ argusProjectId: { equals: projectId } }, { organizationId: { equals: organizationId } }] },
  })
  return (result.docs[0] as Project | undefined) ?? null
}

async function modelFor(payload: Payload, project: Project, modelId: string): Promise<Model | null> {
  if (!isUUID(modelId)) return null
  const result = await payload.find({
    collection: 'data-models', depth: 0, limit: 1, overrideAccess: true, pagination: false,
    where: { and: [{ id: { equals: modelId } }, { project: { equals: project.id } }, { kind: { equals: 'content' } }] },
  })
  return (result.docs[0] as Model | undefined) ?? null
}

function modelView(model: Model) {
  return {
    id: model.id, name: model.name ?? '', slug: model.slug ?? '', description: model.description ?? '',
    public_read: model.publicRead === true, schema_version: model.schemaVersion ?? 1,
    status: model.status ?? 'active',
    fields: (model.fields ?? []).map(({ key, label, type, required }) => ({ key, label, type, required: required === true })),
  }
}

export async function GET(request: Request, { params }: { params: Promise<{ projectId: string }> }) {
  const identity = internalIdentity(request)
  const { projectId } = await params
  if (!identity) return NextResponse.json({ code: 'PERMISSION_DENIED' }, { status: 401 })
  if (!isUUID(projectId)) return NextResponse.json({ code: 'NOT_FOUND' }, { status: 404 })
  const payload = await getPayload({ config })
  const project = await projectFor(payload, projectId, identity.organizationId)
  if (!project) return NextResponse.json({ code: 'NOT_FOUND' }, { status: 404 })

  const models = await payload.find({
    collection: 'data-models', depth: 0, limit: 100, overrideAccess: true, pagination: false,
    sort: 'name', where: { and: [{ project: { equals: project.id } }, { kind: { equals: 'content' } }] },
  })
  const modelIds = models.docs.map((model) => String(model.id))
  const records = modelIds.length === 0 ? { docs: [] } : await payload.find({
    collection: 'data-records', depth: 0, draft: true, limit: 500, overrideAccess: true, pagination: false,
    sort: '-updatedAt', where: { and: [{ project: { equals: project.id } }, { model: { in: modelIds } }] },
  })
  return NextResponse.json({
    project_status: project.status ?? 'active',
    models: models.docs.map((model) => modelView(model as Model)),
    records: records.docs.map((record) => {
      const doc = record as { id: string | number; model?: unknown; values?: unknown; _status?: string; status?: string; publishedAt?: string | null; updatedAt?: string }
      const modelId = typeof doc.model === 'object' && doc.model && 'id' in doc.model ? String(doc.model.id) : String(doc.model ?? '')
      return { id: doc.id, model_id: modelId, values: doc.values ?? {}, editorial_status: doc._status ?? 'draft', lifecycle_status: doc.status ?? 'active', published_at: doc.publishedAt ?? null, updated_at: doc.updatedAt ?? null }
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
  const project = await projectFor(payload, projectId, identity.organizationId)
  if (!project || project.status !== 'active') return NextResponse.json({ code: 'NOT_FOUND' }, { status: 404 })

  try {
    if (body.operation === 'create_model') {
      const normalized = normalizeModelInput(body.model)
      if (!normalized) return NextResponse.json({ code: 'INVALID_REQUEST' }, { status: 400 })
      const model = await payload.create({
        collection: 'data-models', depth: 0, draft: false, overrideAccess: true,
        data: { organizationId: identity.organizationId, argusProjectId: projectId, project: project.id, name: normalized.name, slug: normalized.slug, description: normalized.description, kind: 'content', publicRead: normalized.publicRead, schemaVersion: 1, status: 'active', fields: normalized.fields },
      })
      return NextResponse.json({ model: modelView(model as Model) }, { status: 201 })
    }

    if (body.operation === 'save_record') {
      const modelId = typeof body.model_id === 'string' ? body.model_id : ''
      const model = await modelFor(payload, project, modelId)
      if (!model || model.status !== 'active') return NextResponse.json({ code: 'NOT_FOUND' }, { status: 404 })
      const fields = (model.fields ?? []).map(({ key, label, type, required }) => ({ key, label, type, required: required === true }))
      const values = validateValues(fields, body.values)
      if (!values) return NextResponse.json({ code: 'INVALID_RECORD' }, { status: 400 })
      const publish = body.publish === true
      const data = { organizationId: identity.organizationId, argusProjectId: projectId, project: project.id, model: model.id, schemaVersion: model.schemaVersion ?? 1, values, status: 'active' as const, _status: publish ? 'published' as const : 'draft' as const }
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
      const record = saved as { id: string; values?: unknown; _status?: string; publishedAt?: string | null; updatedAt?: string }
      return NextResponse.json({ record: { id: record.id, model_id: model.id, values: record.values ?? {}, editorial_status: record._status ?? (publish ? 'published' : 'draft'), published_at: record.publishedAt ?? null, updated_at: record.updatedAt } }, { status: recordId ? 200 : 201 })
    }
  } catch (error) {
    console.error('Argus CMS operation failed', { operation: body.operation, projectId, userId: identity.userId, error })
    return NextResponse.json({ code: 'CONTENT_OPERATION_FAILED' }, { status: 409 })
  }
  return NextResponse.json({ code: 'INVALID_REQUEST' }, { status: 400 })
}
