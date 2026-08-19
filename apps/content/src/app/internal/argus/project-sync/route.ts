import config from '@payload-config'
import { timingSafeEqual } from 'crypto'
import { NextResponse } from 'next/server'
import { getPayload } from 'payload'

import { isUUID } from '@/lib/projectScope'

type ProjectSyncBody = {
  organization_id?: string
  project_id?: string
  name?: string
  client_id?: string | null
  status?: 'active' | 'paused' | 'archived'
}

function authorized(request: Request): boolean {
  const expected = process.env.ARGUS_CONTENT_SYNC_TOKEN ?? ''
  const supplied = request.headers.get('authorization')?.replace(/^Bearer\s+/i, '') ?? ''
  if (expected.length < 32 || supplied.length !== expected.length) return false
  return timingSafeEqual(Buffer.from(supplied), Buffer.from(expected))
}

export async function POST(request: Request) {
  if (!authorized(request)) {
    return NextResponse.json({ code: 'PERMISSION_DENIED' }, { status: 401 })
  }

  const body = await request.json().catch(() => null) as ProjectSyncBody | null
  if (
    !body ||
    typeof body.organization_id !== 'string' ||
    !isUUID(body.organization_id) ||
    typeof body.project_id !== 'string' ||
    !isUUID(body.project_id) ||
    typeof body.name !== 'string' ||
    body.name.trim().length === 0 ||
    body.name.trim().length > 160 ||
    (body.client_id !== null && body.client_id !== undefined && !isUUID(body.client_id)) ||
    !['active', 'paused', 'archived'].includes(body.status ?? 'active')
  ) {
    return NextResponse.json({ code: 'INVALID_REQUEST' }, { status: 400 })
  }

  const payload = await getPayload({ config })
  const existing = await payload.find({
    collection: 'project-spaces',
    depth: 0,
    limit: 1,
    overrideAccess: true,
    pagination: false,
    where: {
      argusProjectId: { equals: body.project_id },
    },
  })
  const current = existing.docs[0] as {
    id: string | number
    organizationId?: string
  } | undefined

  if (current?.organizationId && current.organizationId !== body.organization_id) {
    return NextResponse.json({ code: 'PROJECT_SCOPE_CONFLICT' }, { status: 409 })
  }

  const data = {
    argusProjectId: body.project_id,
    organizationId: body.organization_id,
    name: body.name.trim(),
    clientId: body.client_id ?? null,
    status: body.status ?? 'active',
  }

  const project = current
    ? await payload.update({
        collection: 'project-spaces',
        id: current.id,
        data,
        depth: 0,
        overrideAccess: true,
      })
    : await payload.create({
        collection: 'project-spaces',
        data,
        depth: 0,
        overrideAccess: true,
      })

  return NextResponse.json({
    id: project.id,
    argus_project_id: body.project_id,
    status: project.status,
  })
}
