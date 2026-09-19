import config from '@payload-config'
import { getPayload } from 'payload'
import { verifyPreview } from '@/lib/site-security'
import { authorizeInternalProject } from '@/lib/internalProjectAccess'
export async function POST(request: Request) {
  const token = request.headers.get('authorization')?.replace(/^Bearer /, '') ?? ''
  const grant = verifyPreview(token)
  if (!grant) return new Response(null, { status: 403 })
  const payload = await getPayload({ config })
  const projects = await payload.find({ collection: 'project-spaces', depth: 0, limit: 1, overrideAccess: true, where: { and: [{ argusProjectId: { equals: grant.project } }, { status: { equals: 'active' } }] } })
  const project = projects.docs[0]
  if (!project) return new Response(null, { status: 403 })
  const user = await payload.findByID({ collection: 'workspace-users', id: grant.user, overrideAccess: true }).catch(() => null)
  if (!user?.argusUserId || !await authorizeInternalProject(payload, { organizationId: project.organizationId, userId: user.argusUserId, workspaceUserId: user.id }, project, 'editor')) return new Response(null, { status: 403 })
  const sites = await payload.find({ collection: 'site-connections', depth: 0, limit: 1, overrideAccess: true, where: { project: { equals: project.id } } })
  if (!sites.docs[0]?.previewURL || new URL(sites.docs[0].previewURL).origin !== grant.origin) return new Response(null, { status: 403 })
  const records = await payload.find({ collection: 'data-records', depth: 0, draft: true, limit: 1, overrideAccess: true, where: { and: [{ id: { equals: grant.record } }, { project: { equals: project.id } }] } })
  const record = records.docs[0]
  if (!record) return new Response(null, { status: 404 })
  const model = await payload.findByID({ collection: 'data-models', id: String(record.model), depth: 0, overrideAccess: true })
  const allowed = await payload.find({ collection: 'data-models', depth: 0, limit: 100, overrideAccess: true, where: { and: [{ project: { equals: project.id } }, { id: { in: (model.allowedComponents ?? []).map(String) } }, { contentRole: { equals: 'component' } }, { status: { equals: 'active' } }] } })
  return Response.json({ allowed_components: allowed.docs.map(component => component.slug), projectId: grant.project, record: { id: record.id, values: record.values, layout: record.layout, model: record.model } }, { headers: { 'cache-control': 'private, no-store', 'referrer-policy': 'no-referrer' } })
}
