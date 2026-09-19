import config from '@payload-config'
import { getPayload } from 'payload'
import { internalIdentity } from './argusCmsContract'
import { authorizeInternalProject, type InternalProjectRole } from './internalProjectAccess'
import { isUUID } from './projectScope'

export async function siteAccess(request: Request, projectId: string, role: InternalProjectRole = 'viewer') {
  const identity = internalIdentity(request)
  if (!identity || !isUUID(projectId)) throw new Error('PERMISSION_DENIED')
  const payload = await getPayload({ config })
  const projects = await payload.find({ collection: 'project-spaces', depth: 0, limit: 1, overrideAccess: true,
    where: { and: [{ argusProjectId: { equals: projectId } }, { organizationId: { equals: identity.organizationId } }] } })
  const project = projects.docs[0]
  if (!project) throw new Error('PROJECT_NOT_READY')
  const user = await authorizeInternalProject(payload, identity, project, role)
  if (!user) throw new Error('PERMISSION_DENIED')
  return { payload, project, user }
}
export function siteError(error: unknown): Response {
  const code = error instanceof Error ? error.message : 'SITE_REQUEST_FAILED'
  const safe = ['PERMISSION_DENIED', 'PROJECT_NOT_READY', 'INVALID_URL', 'INVALID_TARGET', 'SITE_ENCRYPTION_KEY_NOT_CONFIGURED', 'CLOUDFLARE_REQUEST_FAILED', 'SITE_NOT_CONNECTED', 'INVALID_REQUEST', 'PUBLIC_CONTENT_REQUIRED', 'RELEASE_IN_PROGRESS']
  return Response.json({ code: safe.includes(code) ? code : 'SITE_REQUEST_FAILED' }, { status: code === 'PERMISSION_DENIED' ? 403 : code === 'PROJECT_NOT_READY' ? 409 : 400 })
}
