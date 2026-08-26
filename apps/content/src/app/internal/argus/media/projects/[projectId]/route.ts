import config from '@payload-config'
import { NextResponse } from 'next/server'
import { getPayload, type Payload } from 'payload'

import { internalIdentity } from '@/lib/argusCmsContract'
import { authorizeInternalProject } from '@/lib/internalProjectAccess'
import { MAX_MEDIA_BYTES, normalizeMediaMetadata, validMediaFile } from '@/lib/argusMediaContract'
import { isUUID } from '@/lib/projectScope'

type Project = { id: string; organizationId?: string; status?: string }

async function projectFor(payload: Payload, projectId: string, organizationId: string): Promise<Project | null> {
  const result = await payload.find({
    collection: 'project-spaces', depth: 0, limit: 1, overrideAccess: true, pagination: false,
    where: { and: [{ argusProjectId: { equals: projectId } }, { organizationId: { equals: organizationId } }] },
  })
  return (result.docs[0] as Project | undefined) ?? null
}

function mediaView(document: Record<string, unknown>) {
  const sizes = document.sizes && typeof document.sizes === 'object' ? document.sizes : {}
  return {
    id: String(document.id), filename: document.filename ?? '', mime_type: document.mimeType ?? '',
    filesize: document.filesize ?? 0, width: document.width ?? null, height: document.height ?? null,
    alt: document.alt ?? '', caption: document.caption ?? '', public_read: document.publicRead === true,
    url: document.url ?? null, sizes, updated_at: document.updatedAt ?? null,
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
  if (!await authorizeInternalProject(payload, identity, project, 'viewer')) return NextResponse.json({ code: 'PERMISSION_DENIED' }, { status: 403 })
  const media = await payload.find({
    collection: 'media', depth: 0, limit: 100, sort: '-updatedAt', overrideAccess: true, pagination: false,
    where: { project: { equals: project.id } },
  })
  return NextResponse.json({ media: media.docs.map((document) => mediaView(document as unknown as Record<string, unknown>)) })
}

export async function POST(request: Request, { params }: { params: Promise<{ projectId: string }> }) {
  const identity = internalIdentity(request)
  const { projectId } = await params
  if (!identity) return NextResponse.json({ code: 'PERMISSION_DENIED' }, { status: 401 })
  if (!isUUID(projectId)) return NextResponse.json({ code: 'NOT_FOUND' }, { status: 404 })
  const declaredLength = Number(request.headers.get('content-length') ?? 0)
  if (!Number.isFinite(declaredLength) || declaredLength <= 0) return NextResponse.json({ code: 'LENGTH_REQUIRED' }, { status: 411 })
  if (declaredLength > MAX_MEDIA_BYTES + 64 * 1024) return NextResponse.json({ code: 'UPLOAD_TOO_LARGE' }, { status: 413 })
  const payload = await getPayload({ config })
  const project = await projectFor(payload, projectId, identity.organizationId)
  if (!project || project.status !== 'active') return NextResponse.json({ code: 'NOT_FOUND' }, { status: 404 })
  const actor = await authorizeInternalProject(payload, identity, project, 'editor')
  if (!actor) return NextResponse.json({ code: 'PERMISSION_DENIED' }, { status: 403 })
  const form = await request.formData().catch(() => null)
  const file = form?.get('file')
  const metadata = normalizeMediaMetadata({ alt: form?.get('alt'), caption: form?.get('caption'), public_read: form?.get('public_read') })
  if (!(file instanceof File) || !validMediaFile(file) || !metadata) return NextResponse.json({ code: 'INVALID_MEDIA' }, { status: 400 })
  try {
    const created = await payload.create({
      collection: 'media', depth: 0, overrideAccess: true, user: actor as any,
      data: { organizationId: identity.organizationId, argusProjectId: projectId, project: project.id, ...metadata },
      file: { data: Buffer.from(await file.arrayBuffer()), mimetype: file.type, name: file.name, size: file.size },
    })
    return NextResponse.json({ media: mediaView(created as unknown as Record<string, unknown>) }, { status: 201 })
  } catch (error) {
    console.error('Argus media upload failed', { projectId, userId: identity.userId, workspaceUserId: identity.workspaceUserId, error })
    return NextResponse.json({ code: 'MEDIA_UPLOAD_FAILED' }, { status: 409 })
  }
}

export async function PATCH(request: Request, { params }: { params: Promise<{ projectId: string }> }) {
  const identity = internalIdentity(request)
  const { projectId } = await params
  if (!identity) return NextResponse.json({ code: 'PERMISSION_DENIED' }, { status: 401 })
  if (!isUUID(projectId)) return NextResponse.json({ code: 'NOT_FOUND' }, { status: 404 })
  const body = await request.json().catch(() => null) as Record<string, unknown> | null
  const mediaId = typeof body?.media_id === 'string' ? body.media_id : ''
  const metadata = normalizeMediaMetadata({ alt: body?.alt, caption: body?.caption, public_read: body?.public_read })
  if (!isUUID(mediaId) || !metadata) return NextResponse.json({ code: 'INVALID_MEDIA' }, { status: 400 })
  const payload = await getPayload({ config })
  const project = await projectFor(payload, projectId, identity.organizationId)
  if (!project || project.status !== 'active') return NextResponse.json({ code: 'NOT_FOUND' }, { status: 404 })
  const actor = await authorizeInternalProject(payload, identity, project, 'editor')
  if (!actor) return NextResponse.json({ code: 'PERMISSION_DENIED' }, { status: 403 })
  const existing = await payload.find({
    collection: 'media', depth: 0, limit: 1, overrideAccess: true, pagination: false,
    where: { and: [{ id: { equals: mediaId } }, { project: { equals: project.id } }] },
  })
  if (existing.docs.length !== 1) return NextResponse.json({ code: 'NOT_FOUND' }, { status: 404 })
  const updated = await payload.update({ collection: 'media', id: mediaId, depth: 0, overrideAccess: true, user: actor as any, data: metadata })
  return NextResponse.json({ media: mediaView(updated as unknown as Record<string, unknown>) })
}

export async function DELETE(request: Request, { params }: { params: Promise<{ projectId: string }> }) {
  const identity = internalIdentity(request)
  const { projectId } = await params
  if (!identity) return NextResponse.json({ code: 'PERMISSION_DENIED' }, { status: 401 })
  if (!isUUID(projectId)) return NextResponse.json({ code: 'NOT_FOUND' }, { status: 404 })
  const mediaId = new URL(request.url).searchParams.get('media_id') ?? ''
  if (!isUUID(mediaId)) return NextResponse.json({ code: 'NOT_FOUND' }, { status: 404 })
  const payload = await getPayload({ config })
  const project = await projectFor(payload, projectId, identity.organizationId)
  if (!project) return NextResponse.json({ code: 'NOT_FOUND' }, { status: 404 })
  const actor = await authorizeInternalProject(payload, identity, project, 'manager')
  if (!actor) return NextResponse.json({ code: 'PERMISSION_DENIED' }, { status: 403 })
  const existing = await payload.find({
    collection: 'media', depth: 0, limit: 1, overrideAccess: true, pagination: false,
    where: { and: [{ id: { equals: mediaId } }, { project: { equals: project.id } }] },
  })
  if (existing.docs.length !== 1) return NextResponse.json({ code: 'NOT_FOUND' }, { status: 404 })
  await payload.delete({ collection: 'media', id: mediaId, overrideAccess: true, user: actor as any })
  return new NextResponse(null, { status: 204 })
}
