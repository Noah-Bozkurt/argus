import type { PayloadRequest } from 'payload'
import { relationshipID } from '@/access/projectAccess'

const UUID_PATTERN = /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i

export function isUUID(value: string): boolean {
  return UUID_PATTERN.test(value)
}

export async function resolveProjectScope(req: PayloadRequest, projectValue: unknown) {
  const projectID = relationshipID(projectValue)
  if (projectID === null) {
    throw new Error('A project space is required')
  }
  const project = await req.payload.findByID({
    collection: 'project-spaces',
    id: projectID,
    depth: 0,
    overrideAccess: true,
    req,
  }) as {
    id: string | number
    argusProjectId?: string
    organizationId?: string
  }
  if (!project.organizationId || !project.argusProjectId) {
    throw new Error('Project space is missing Argus scope metadata')
  }
  return {
    projectID: project.id,
    argusProjectId: project.argusProjectId,
    organizationId: project.organizationId,
  }
}
