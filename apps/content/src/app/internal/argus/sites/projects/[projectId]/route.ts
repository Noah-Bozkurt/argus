import { withSiteLock } from '@/lib/content-release'
import { siteAccess, siteError } from '@/lib/site-access'
import { discoverTargets, createHook, accountPath, targetName } from '@/lib/cloudflare-sites'
import { httpsURL, seal } from '@/lib/site-security'

export async function GET(request: Request, { params }: { params: Promise<{ projectId: string }> }) {
  try {
    const { projectId } = await params
    const { payload, project, user } = await siteAccess(request, projectId)
    const result = await payload.find({ collection: 'site-connections', overrideAccess: true, limit: 1, depth: 0, where: { project: { equals: project.id } } })
    const site = result.docs[0]
    const releases = await payload.find({ collection: 'content-releases', overrideAccess: true, limit: 20, depth: 0, sort: '-createdAt', where: { project: { equals: project.id } } })
    return Response.json({ can_connect: ['owner', 'admin'].includes(user.role ?? ''), site: site ? { siteURL: site.siteURL, previewURL: site.previewURL, provider: site.provider, accountId: site.accountId, target: site.target, branch: site.branch, components: site.components, connected: Boolean(site.hook) } : null,
      releases: releases.docs.map(release => ({ id: release.id, status: release.status, createdAt: release.createdAt, errorCode: release.errorCode })) })
  } catch (error) { return siteError(error) }
}
export async function POST(request: Request, { params }: { params: Promise<{ projectId: string }> }) {
  try {
    const { projectId } = await params
    const { payload, project, user } = await siteAccess(request, projectId, 'manager')
    if (!['owner', 'admin'].includes(user.role ?? '')) throw new Error('PERMISSION_DENIED')
    const body = await request.json()
    if (typeof body.token !== 'string' || body.token.length < 20 || body.token.length > 1024 || typeof body.accountId !== 'string') throw new Error('INVALID_REQUEST')
    accountPath(body.accountId)
    if (body.operation === 'discover') return Response.json({ targets: await discoverTargets(body.token, body.accountId) })
    if (body.operation !== 'connect' || !['pages', 'workers'].includes(body.provider) || typeof body.target !== 'string' || typeof body.branch !== 'string') throw new Error('INVALID_REQUEST')
    targetName(body.target)
    const siteURL = httpsURL(body.siteURL)
    const previewURL = body.previewURL ? httpsURL(body.previewURL) : null
    // Encrypt before any provider mutation so an absent key cannot orphan a hook.
    const credential = seal(body.token, projectId)
    return await withSiteLock(payload, project.id, async () => {
    const existing = await payload.find({ collection: 'site-connections', overrideAccess: true, limit: 1, depth: 0, where: { project: { equals: project.id } } })
    const active = await payload.find({ collection: 'content-releases', overrideAccess: true, limit: 1, where: { and: [{ project: { equals: project.id } }, { status: { in: ['queued', 'triggering', 'building', 'unknown'] } }] } })
    if (active.docs.length) throw new Error('RELEASE_IN_PROGRESS')
    const hookURL = await createHook(body.token, body.accountId, { provider: body.provider, name: body.target, branch: body.branch }, projectId)
    const data = { project: project.id, siteURL, previewURL, provider: body.provider as 'pages' | 'workers', accountId: body.accountId, target: body.target, branch: body.branch, credential, hook: seal(hookURL, projectId) }
    if (existing.docs[0]) await payload.update({ collection: 'site-connections', id: existing.docs[0].id, overrideAccess: true, data })
    else await payload.create({ collection: 'site-connections', overrideAccess: true, data })
    return Response.json({ connected: true })
    })
  } catch (error) { return siteError(error) }
}
