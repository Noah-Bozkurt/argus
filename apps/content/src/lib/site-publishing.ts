import type { Payload } from 'payload'
import { withSiteLock } from './content-release'
import { accountPath, cloudflare, targetName } from './cloudflare-sites'
import { unseal } from './site-security'
import { publicSiteJSON } from './public-site-fetch'

export async function processSite(payload: Payload, project: string, projectId: string) {
  return withSiteLock(payload, project, async () => {
    const sites = await payload.find({ collection: 'site-connections', depth: 0, limit: 1, overrideAccess: true, where: { project: { equals: project } } })
    const site = sites.docs[0]
    if (!site?.hook) return
    const active = await payload.find({ collection: 'content-releases', depth: 0, limit: 1, sort: 'createdAt', overrideAccess: true,
      where: { and: [{ project: { equals: project } }, { status: { in: ['queued', 'triggering', 'building', 'unknown'] } }] } })
    const release = active.docs[0]
    if (!release) return
    const update = (data: { status?: 'triggering' | 'building' | 'deployed' | 'failed' | 'unknown'; providerId?: string; startedAt?: string; completedAt?: string; errorCode?: string }) => payload.update({ collection: 'content-releases', id: release.id, overrideAccess: true, data })
    if (release.status === 'queued') {
      await update({ status: 'triggering', startedAt: new Date().toISOString() })
      try {
        const hook = new URL(unseal(site.hook, projectId))
        if (hook.origin !== 'https://api.cloudflare.com' || !/^\/client\/v4\/(pages\/webhooks|workers\/builds)\/deploy_hooks\/[a-z0-9-]+$/i.test(hook.pathname)) throw new Error('INVALID_HOOK')
        const response = await fetch(hook, { method: 'POST', redirect: 'error', signal: AbortSignal.timeout(10000) })
        if (!response.ok) throw new Error('BUILD_TRIGGER_UNCONFIRMED')
        const result = await response.json() as { result?: { id?: string; build_uuid?: string }; id?: string }
        const id = result.result?.build_uuid ?? result.result?.id ?? result.id
        await update({ status: id ? 'building' : 'unknown', ...(id ? { providerId: id } : {}), ...(!id ? { errorCode: 'BUILD_ID_UNAVAILABLE' } : {}) })
      } catch { await update({ status: 'unknown', errorCode: 'BUILD_TRIGGER_UNCONFIRMED' }) }
      return
    }
    // An interrupted trigger is never blindly replayed: first verify the site.
    try {
      const marker = await publicSiteJSON(`${site.siteURL}/argus-release.json`) as { releaseId?: string; projectId?: string }
      if (marker.releaseId === release.id && marker.projectId === projectId) {
        await update({ status: 'deployed', completedAt: new Date().toISOString(), errorCode: '' })
        return
      }
    } catch { /* The last live site may not have the integration yet. */ }
    if (release.status === 'triggering') { await update({ status: 'unknown', errorCode: 'BUILD_TRIGGER_UNCONFIRMED' }); return }
    if (!release.providerId) return
    try {
      const token = unseal(site.credential, projectId)
      const path = accountPath(site.accountId)
      const result = site.provider === 'pages'
        ? await cloudflare<{ latest_stage?: { status?: string } }>(token, `${path}/pages/projects/${targetName(site.target)}/deployments/${encodeURIComponent(release.providerId)}`)
        : await cloudflare<{ status?: string }>(token, `${path}/builds/builds/${encodeURIComponent(release.providerId)}`)
      const status = 'latest_stage' in result ? result.latest_stage?.status : (result as { status?: string }).status
      if (['failure', 'failed', 'canceled', 'cancelled'].includes(status ?? '')) await update({ status: 'failed', errorCode: 'CLOUDFLARE_BUILD_FAILED', completedAt: new Date().toISOString() })
      else if (['success', 'succeeded'].includes(status ?? '') && Date.now() - Date.parse(release.startedAt ?? release.createdAt) > 10 * 60 * 1000) await update({ status: 'unknown', errorCode: 'LIVE_RELEASE_NOT_VERIFIED' })
    } catch { await update({ status: 'unknown', errorCode: 'BUILD_STATUS_UNAVAILABLE' }) }
  })
}
