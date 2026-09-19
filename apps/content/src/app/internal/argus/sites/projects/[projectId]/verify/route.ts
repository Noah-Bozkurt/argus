import { siteAccess, siteError } from '@/lib/site-access'
import { publicSiteJSON } from '@/lib/public-site-fetch'
import { normalizeModelInput } from '@/lib/argusCmsContract'
export async function POST(request: Request, { params }: { params: Promise<{ projectId: string }> }) {
 try {
  const { projectId } = await params
  const { payload, project } = await siteAccess(request, projectId, 'manager')
  const body = await request.json()
  const sites = await payload.find({ collection: 'site-connections', depth: 0, limit: 1, overrideAccess: true, where: { project: { equals: project.id } } })
  const site = sites.docs[0]
  if (!site) throw new Error('SITE_NOT_CONNECTED')
  const manifest = await publicSiteJSON(`${site.siteURL}/argus-components.json`) as { version?: number; components?: unknown[] }
  if (manifest.version !== 1 || !Array.isArray(manifest.components) || manifest.components.length > 100) throw new Error('INVALID_REQUEST')
  const normalized = manifest.components.map(component => normalizeModelInput({ ...(component as object), content_role: 'component' }))
  if (normalized.some(component => !component)) throw new Error('INVALID_REQUEST')
  const models = await payload.find({ collection: 'data-models', depth: 0, pagination: false, overrideAccess: true, where: { project: { equals: project.id } } })
  const mismatched = normalized.filter(component => {
   if (!component) return false
   const model = models.docs.find(model => model.slug === component.slug)
   return model && (model.contentRole !== 'component' || model.kind !== 'content' || component.fields.some(field => !model.fields.some(existing => existing.key === field.key && existing.type === field.type)))
  }).map(component => component!.slug)
  const missing = normalized.filter(component => component && !models.docs.some(model => model.slug === component.slug))
  if (body.register === true) {
   for (const component of missing) {
    if (!component) continue
    await payload.create({ collection: 'data-models', overrideAccess: true, data: { project: project.id, organizationId: project.organizationId, argusProjectId: projectId, name: component.name, slug: component.slug, kind: 'content', contentRole: 'component', schemaVersion: 1, status: 'active', publicRead: false, fields: component.fields } })
   }
  }
  await payload.update({ collection: 'site-connections', id: site.id, overrideAccess: true, data: { components: JSON.parse(JSON.stringify(normalized)) } })
  return Response.json({ manifest: 'connected', mismatched, missing: body.register ? [] : missing.map(component => component!.slug), existing_schemas_preserved: true, preview: site.previewURL ? 'configured; open a saved page to verify rendering' : 'not configured' })
 } catch (error) { return siteError(error) }
}
