import config from '@payload-config'
import { getPayload } from 'payload'
import { timingSafeEqual } from 'node:crypto'
import { isUUID } from '@/lib/projectScope'
import { processSite } from '@/lib/site-publishing'
export async function POST(request: Request) {
  const expected = Buffer.from(process.env.ARGUS_CONTENT_SYNC_TOKEN ?? '')
  const supplied = Buffer.from(request.headers.get('authorization')?.replace(/^Bearer /, '') ?? '')
  if (expected.length < 32 || supplied.length !== expected.length || !timingSafeEqual(expected, supplied)) return new Response(null, { status: 403 })
  const body = await request.json()
  if (!isUUID(body.organization_id)) return new Response(null, { status: 400 })
  const payload = await getPayload({ config })
  // Rotate through pending releases rather than scanning every project. A bounded
  // concurrent batch stays within the worker request timeout even for slow sites.
  const pending = await payload.find({ collection: 'content-releases', depth: 1, limit: 100,
    sort: 'updatedAt', overrideAccess: true, where: { and: [
      { 'project.organizationId': { equals: body.organization_id } },
      { 'project.status': { equals: 'active' } },
      { status: { in: ['queued', 'triggering', 'building', 'unknown'] } },
    ] } })
  const projects = new Map<string, string>()
  const selected = [] as typeof pending.docs
  for (const release of pending.docs) {
    const project = release.project
    if (typeof project === 'string' || projects.has(project.id)) continue
    projects.set(project.id, project.argusProjectId)
    selected.push(release)
    if (projects.size === 4) break
  }
  await Promise.all(selected.map(async release => {
    // Touch the candidate so unknown builds cannot monopolize future batches.
    await payload.update({ collection: 'content-releases', id: release.id, overrideAccess: true, data: {} })
    const project = release.project
    if (typeof project !== 'string') await processSite(payload, project.id, project.argusProjectId)
  }))
  return Response.json({ processed: projects.size })
}
