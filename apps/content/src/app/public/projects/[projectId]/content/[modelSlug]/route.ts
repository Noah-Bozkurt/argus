import config from '@payload-config'
import { NextResponse } from 'next/server'
import { getPayload } from 'payload'

import { isUUID } from '@/lib/projectScope'

const MODEL_SLUG_PATTERN = /^[a-z][a-z0-9_]*$/
const MAX_LIMIT = 100

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

  const payload = await getPayload({ config })
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
    slug?: string
    schemaVersion?: number
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

  const response = NextResponse.json(
    {
      model: {
        slug: modelSlug,
        schema_version: model.schemaVersion ?? null,
      },
      records: records.docs.map((record) => {
        const doc = record as {
          id: string | number
          values?: unknown
          publishedAt?: string | null
          updatedAt?: string
        }
        return {
          id: doc.id,
          values: doc.values ?? {},
          published_at: doc.publishedAt ?? null,
          updated_at: doc.updatedAt ?? null,
        }
      }),
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
  return response
}
