import { siteAccess, siteError } from '@/lib/site-access'
import { previewToken } from '@/lib/site-security'
import { isUUID } from '@/lib/projectScope'
export async function POST(request: Request, { params }: { params: Promise<{ projectId: string }> }) {
  try {
    const { projectId } = await params
    const { payload, project, user } = await siteAccess(request, projectId, 'editor')
    const { recordId } = await request.json()
    if (!isUUID(recordId)) throw new Error('INVALID_REQUEST')
    const records = await payload.find({ collection: 'data-records', depth: 0, limit: 1, draft: true, overrideAccess: true, where: { and: [{ id: { equals: recordId } }, { project: { equals: project.id } }] } })
    if (!records.docs.length) throw new Error('PERMISSION_DENIED')
    const sites = await payload.find({ collection: 'site-connections', depth: 0, limit: 1, overrideAccess: true, where: { project: { equals: project.id } } })
    const site = sites.docs[0]
    if (!site?.previewURL) throw new Error('SITE_NOT_CONNECTED')
    const origin = new URL(site.previewURL).origin
    return Response.json({ url: site.previewURL, token: previewToken({ project: projectId, record: recordId, user: String(user.id), origin }) }, { headers: { 'cache-control': 'no-store' } })
  } catch (error) { return siteError(error) }
}
