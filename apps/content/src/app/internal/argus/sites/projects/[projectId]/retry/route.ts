import { siteAccess, siteError } from '@/lib/site-access'
import { withSiteLock } from '@/lib/content-release'
import { processSite } from '@/lib/site-publishing'
import { isUUID } from '@/lib/projectScope'
export async function POST(request: Request, { params }: { params: Promise<{ projectId: string }> }) {
 try {
  const { projectId } = await params
  const { payload, project } = await siteAccess(request, projectId, 'editor')
  if (project.status !== 'active') throw new Error('PERMISSION_DENIED')
  const body = await request.json()
  if (body.operation === 'reconcile') {
   await processSite(payload, project.id, projectId)
   return Response.json({ checked: true })
  }
  if (!isUUID(body.releaseId) || !isUUID(body.requestId)) throw new Error('INVALID_REQUEST')
  return await withSiteLock(payload, project.id, async () => {
   const existing = await payload.find({ collection: 'content-releases', overrideAccess: true, limit: 1, where: { and: [{ project: { equals: project.id } }, { requestId: { equals: body.requestId } }] } })
   if (existing.docs[0]) return Response.json({ id: existing.docs[0].id })
   const releases = await payload.find({ collection: 'content-releases', overrideAccess: true, limit: 1, where: { and: [{ project: { equals: project.id } }, { id: { equals: body.releaseId } }, { status: { equals: 'failed' } }] } })
   if (!releases.docs[0]) throw new Error('INVALID_REQUEST')
   const release = await payload.create({ collection: 'content-releases', overrideAccess: true, data: { project: project.id, requestId: body.requestId, snapshot: releases.docs[0].snapshot, status: 'queued' } })
   return Response.json({ id: release.id })
  })
 } catch (error) { return siteError(error) }
}
