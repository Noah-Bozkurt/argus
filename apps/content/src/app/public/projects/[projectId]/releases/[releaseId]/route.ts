import config from '@payload-config'
import { getPayload } from 'payload'
import { isUUID } from '@/lib/projectScope'
export async function GET(_request: Request, { params }: { params: Promise<{ projectId: string; releaseId: string }> }) {
  const { projectId, releaseId } = await params
  if (!isUUID(projectId) || (releaseId !== 'build' && !isUUID(releaseId))) return new Response(null, { status: 404 })
  const payload = await getPayload({ config })
  const projects = await payload.find({ collection: 'project-spaces', depth: 0, limit: 1, overrideAccess: true, where: { and: [{ argusProjectId: { equals: projectId } }, { status: { equals: 'active' } }] } })
  const project = projects.docs[0]
  if (!project) return new Response(null, { status: 404 })
  // One active build per site: resolve once at build startup, then use its immutable ID.
  const releases = await payload.find({ collection: 'content-releases', depth: 0, limit: 1, sort: '-createdAt', overrideAccess: true,
    where: { and: [{ project: { equals: project.id } }, ...(releaseId === 'build' ? [{ status: { in: ['triggering', 'building', 'unknown', 'deployed'] } }] : [{ id: { equals: releaseId } }])] } })
  const release = releases.docs[0]
  if (!release) return new Response(null, { status: 404 })
  return Response.json({ id: release.id, snapshot: release.snapshot }, { headers: { 'cache-control': 'no-store', 'access-control-allow-origin': '*' } })
}
