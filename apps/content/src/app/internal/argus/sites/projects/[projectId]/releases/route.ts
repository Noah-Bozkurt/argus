import { siteAccess, siteError } from '@/lib/site-access'
import { createRelease } from '@/lib/content-release'
export async function POST(request: Request, { params }: { params: Promise<{ projectId: string }> }) {
  try {
    const { projectId } = await params
    const { payload, project } = await siteAccess(request, projectId, 'editor')
    if (project.status !== 'active') throw new Error('PERMISSION_DENIED')
    const body = await request.json()
    if (typeof body.requestId !== 'string' || !Array.isArray(body.recordIds)) throw new Error('INVALID_REQUEST')
    const sites = await payload.find({ collection: 'site-connections', overrideAccess: true, limit: 1, where: { project: { equals: project.id } } })
    if (!sites.docs[0]?.hook) throw new Error('SITE_NOT_CONNECTED')
    const release = await createRelease(payload, project.id, projectId, body.requestId, body.recordIds)
    return Response.json({ id: release.id, status: release.status })
  } catch (error) { return siteError(error) }
}
