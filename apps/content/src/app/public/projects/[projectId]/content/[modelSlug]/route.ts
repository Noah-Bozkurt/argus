import config from '@payload-config'
import { NextResponse } from 'next/server'
import { getPayload, type Payload } from 'payload'

import { isUUID } from '@/lib/projectScope'

const MODEL_SLUG_PATTERN = /^[a-z][a-z0-9_]*$/
const MAX_LIMIT = 100
const MAX_EXPANDED_RELATIONS = 100

type PublicField = { key?: string; type?: string; hasMany?: boolean }
type PublicMedia = { id: string; alt: string; caption: string; filename: string; mime_type: string; width: number | null; height: number | null; url: string | null; sizes: unknown }

async function publicMediaValues(payload: Payload, projectId: string | number, fields: PublicField[], input: unknown, cache: Map<string, PublicMedia | null>): Promise<Record<string, unknown>> {
  const values = input && typeof input === 'object' && !Array.isArray(input) ? { ...input as Record<string, unknown> } : {}
  const mediaFields = fields.filter((candidate) => candidate.type === 'media' && candidate.key)
  const references = mediaFields.flatMap((field) => {
    const raw = values[field.key as string]
    return field.hasMany ? (Array.isArray(raw) ? raw.map(String) : []) : raw ? [String(raw)] : []
  }).slice(0, 100)
  const missing = [...new Set(references.filter((id) => !cache.has(id)))]
  if (missing.length > 0) {
    const result = await payload.find({ collection: 'media', depth: 0, limit: 100, overrideAccess: true, pagination: false,
      where: { and: [{ id: { in: missing } }, { project: { equals: projectId } }, { publicRead: { equals: true } }] } })
    for (const id of missing) cache.set(id, null)
    for (const raw of result.docs) {
      const asset = raw as { id: string; alt?: string; caption?: string | null; filename?: string; mimeType?: string; width?: number | null; height?: number | null; url?: string | null; sizes?: unknown }
      cache.set(String(asset.id), { id: String(asset.id), alt: asset.alt ?? '', caption: asset.caption ?? '', filename: asset.filename ?? '', mime_type: asset.mimeType ?? '', width: asset.width ?? null, height: asset.height ?? null, url: asset.url ?? null, sizes: asset.sizes ?? {} })
    }
  }
  for (const field of mediaFields) {
    const key = field.key as string
    const raw = values[key]
    const ids = field.hasMany ? (Array.isArray(raw) ? raw.map(String) : []) : raw ? [String(raw)] : []
    const assets = ids.slice(0, 50).map((id) => cache.get(id)).filter((asset): asset is PublicMedia => Boolean(asset))
    values[key] = field.hasMany ? assets : (assets[0] ?? null)
  }
  return values
}

function corsHeaders(): Record<string, string> {
  return {
    'access-control-allow-origin': '*',
    'access-control-allow-methods': 'GET, OPTIONS',
    'access-control-allow-headers': 'content-type',
  }
}

export function OPTIONS() {
  return new NextResponse(null, {
    status: 204,
    headers: corsHeaders(),
  })
}

export async function GET(
  request: Request,
  { params }: { params: Promise<{ projectId: string; modelSlug: string }> },
) {
  const { projectId, modelSlug } = await params
  if (!isUUID(projectId) || !MODEL_SLUG_PATTERN.test(modelSlug)) {
    return NextResponse.json({ code: 'NOT_FOUND' }, { status: 404, headers: corsHeaders() })
  }

  const url = new URL(request.url)
  const parsedLimit = Number.parseInt(url.searchParams.get('limit') ?? '50', 10)
  const parsedPage = Number.parseInt(url.searchParams.get('page') ?? '1', 10)
  const limit = Number.isFinite(parsedLimit) ? Math.min(Math.max(parsedLimit, 1), MAX_LIMIT) : 50
  const page = Number.isFinite(parsedPage) ? Math.max(parsedPage, 1) : 1
  const expandRelationships = url.searchParams.get('expand') === 'relationships'

  const payload = await getPayload({ config })
  const mediaCache = new Map<string, PublicMedia | null>()
  const componentFieldCache = new Map<string, PublicField[] | null>()
  const projects = await payload.find({
    collection: 'project-spaces',
    depth: 0,
    limit: 1,
    overrideAccess: true,
    pagination: false,
    where: {
      argusProjectId: { equals: projectId },
    },
  })
  const project = projects.docs[0] as { id: string | number; status?: string } | undefined
  if (!project || project.status !== 'active') {
    return NextResponse.json({ code: 'NOT_FOUND' }, { status: 404, headers: corsHeaders() })
  }

  const models = await payload.find({
    collection: 'data-models',
    depth: 0,
    limit: 1,
    overrideAccess: true,
    pagination: false,
    where: {
      and: [
        { project: { equals: project.id } },
        { slug: { equals: modelSlug } },
        { kind: { equals: 'content' } },
        { publicRead: { equals: true } },
        { status: { equals: 'active' } },
      ],
    },
  })
  const model = models.docs[0] as {
    id: string | number
    schemaVersion?: number
    contentRole?: string
    fields?: Array<{ key?: string; type?: string; targetModel?: unknown; hasMany?: boolean }>
  } | undefined
  if (!model) {
    return NextResponse.json({ code: 'NOT_FOUND' }, { status: 404, headers: corsHeaders() })
  }

  const records = await payload.find({
    collection: 'data-records',
    depth: 0,
    draft: false,
    limit,
    overrideAccess: true,
    page,
    where: {
      and: [
        { project: { equals: project.id } },
        { model: { equals: model.id } },
        { status: { equals: 'active' } },
        { _status: { equals: 'published' } },
      ],
    },
  })
  const expanded = new Map<string, Record<string, unknown[]>>()
  if (expandRelationships && records.docs.length > 0) {
    const sourceIds = records.docs.map((record) => String(record.id))
    const edges = await payload.find({ collection: 'data-relations', depth: 0, limit: MAX_EXPANDED_RELATIONS + 1,
      overrideAccess: true, pagination: false, where: { and: [{ project: { equals: project.id } }, { sourceRecord: { in: sourceIds } }] } })
    if (edges.docs.length > MAX_EXPANDED_RELATIONS) return NextResponse.json({ code: 'RELATIONSHIP_EXPANSION_TOO_LARGE' }, { status: 422, headers: corsHeaders() })
    const idOf = (value: unknown) => typeof value === 'object' && value && 'id' in value ? String(value.id) : String(value ?? '')
    for (const edge of edges.docs) {
      const relation = edge as { sourceRecord?: unknown; targetRecord?: unknown; fieldKey?: string }
      const targetId = idOf(relation.targetRecord)
      const targetResult = await payload.find({ collection: 'data-records', depth: 0, draft: false, limit: 1, overrideAccess: true, pagination: false,
        where: { and: [{ id: { equals: targetId } }, { project: { equals: project.id } }, { status: { equals: 'active' } }, { _status: { equals: 'published' } }] } })
      const target = targetResult.docs[0] as { id: string; model?: unknown; values?: unknown; publishedAt?: string | null; updatedAt?: string } | undefined
      if (!target) continue
      const targetModelResult = await payload.find({ collection: 'data-models', depth: 0, limit: 1, overrideAccess: true, pagination: false,
        where: { and: [{ id: { equals: idOf(target.model) } }, { project: { equals: project.id } }, { kind: { equals: 'content' } }, { publicRead: { equals: true } }, { status: { equals: 'active' } }] } })
      const targetModel = targetModelResult.docs[0] as { slug?: string; fields?: PublicField[] } | undefined
      if (!targetModel?.slug) continue
      const sourceId = idOf(relation.sourceRecord)
      const fields = expanded.get(sourceId) ?? {}
      const key = relation.fieldKey ?? ''
      const declaredField = (model.fields ?? []).find((field) => field.key === key && field.type === 'relationship')
      if (!declaredField || idOf(declaredField.targetModel) !== idOf(target.model)) continue
      ;(fields[key] ??= []).push({ id: target.id, model: targetModel.slug, values: await publicMediaValues(payload, project.id, targetModel.fields ?? [], target.values, mediaCache), published_at: target.publishedAt ?? null, updated_at: target.updatedAt ?? null })
      expanded.set(sourceId, fields)
    }
  }

  return NextResponse.json(
    {
      model: {
        slug: modelSlug,
        schema_version: model.schemaVersion ?? null,
      },
      records: await Promise.all(records.docs.map(async (record) => {
        const doc = record as {
          id: string | number
          values?: unknown
          layout?: unknown
          publishedAt?: string | null
          updatedAt?: string
        }
        const layout = model.contentRole === 'page' && Array.isArray(doc.layout) ? await Promise.all(doc.layout.map(async (rawBlock) => {
          const block = rawBlock as { component?: string; values?: unknown }
          const slug = block.component ?? ''
          if (!componentFieldCache.has(slug)) {
            const components = await payload.find({ collection: 'data-models', depth: 0, limit: 1, overrideAccess: true, pagination: false,
              where: { and: [{ project: { equals: project.id } }, { slug: { equals: slug } }, { kind: { equals: 'content' } }, { contentRole: { equals: 'component' } }, { status: { equals: 'active' } }] } })
            const component = components.docs[0] as { fields?: PublicField[] } | undefined
            componentFieldCache.set(slug, component?.fields ?? null)
          }
          const componentFields = componentFieldCache.get(slug)
          return componentFields ? { ...block, values: await publicMediaValues(payload, project.id, componentFields, block.values, mediaCache) } : block
        })) : []
        return {
          id: doc.id,
          values: await publicMediaValues(payload, project.id, model.fields ?? [], doc.values, mediaCache),
          layout,
          ...(expandRelationships ? { relationships: expanded.get(String(doc.id)) ?? {} } : {}),
          published_at: doc.publishedAt ?? null,
          updated_at: doc.updatedAt ?? null,
        }
      })),
      pagination: {
        page: records.page,
        limit: records.limit,
        total_docs: records.totalDocs,
        total_pages: records.totalPages,
        has_next_page: records.hasNextPage,
        has_prev_page: records.hasPrevPage,
      },
    },
    {
      headers: {
        ...corsHeaders(),
        'cache-control': 'public, max-age=60, stale-while-revalidate=300',
      },
    },
  )
}
