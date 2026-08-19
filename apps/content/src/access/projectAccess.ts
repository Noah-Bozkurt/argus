import type { Access, PayloadRequest, Where } from 'payload'

type WorkspaceRole = 'admin' | 'member'
type ProjectRole = 'manager' | 'editor' | 'viewer'

type WorkspaceUser = {
  id: string | number
  organizationId?: string | null
  role?: WorkspaceRole | null
}

const roleRank: Record<ProjectRole, number> = {
  viewer: 1,
  editor: 2,
  manager: 3,
}

export function relationshipID(value: unknown): string | number | null {
  if (typeof value === 'string' || typeof value === 'number') return value
  if (value && typeof value === 'object' && 'id' in value) {
    const id = (value as { id?: unknown }).id
    if (typeof id === 'string' || typeof id === 'number') return id
  }
  return null
}

function workspaceUser(req: PayloadRequest): WorkspaceUser | null {
  return (req.user as WorkspaceUser | null | undefined) ?? null
}

export function isOrganizationAdmin(req: PayloadRequest): boolean {
  return workspaceUser(req)?.role === 'admin'
}

export function userOrganizationID(req: PayloadRequest): string | null {
  return workspaceUser(req)?.organizationId ?? null
}

async function membershipProjectIDs(
  req: PayloadRequest,
  minimumRole: ProjectRole,
): Promise<Array<string | number>> {
  const user = workspaceUser(req)
  if (!user) return []

  const allowedRoles = (Object.keys(roleRank) as ProjectRole[]).filter(
    (role) => roleRank[role] >= roleRank[minimumRole],
  )
  const memberships = await req.payload.find({
    collection: 'project-memberships',
    depth: 0,
    limit: 500,
    overrideAccess: true,
    pagination: false,
    where: {
      and: [
        { user: { equals: user.id } },
        { role: { in: allowedRoles } },
      ],
    },
  })

  return memberships.docs
    .map((membership) => relationshipID((membership as { project?: unknown }).project))
    .filter((id): id is string | number => id !== null)
}

export async function hasProjectRole(
  req: PayloadRequest,
  projectID: string | number,
  minimumRole: ProjectRole,
): Promise<boolean> {
  const user = workspaceUser(req)
  const organizationId = user?.organizationId
  if (!user || !organizationId) return false
  if (user.role === 'admin') {
    const project = await req.payload.findByID({
      collection: 'project-spaces',
      id: projectID,
      depth: 0,
      overrideAccess: true,
    })
    return (project as { organizationId?: string }).organizationId === organizationId
  }

  const memberships = await req.payload.find({
    collection: 'project-memberships',
    depth: 0,
    limit: 1,
    overrideAccess: true,
    pagination: false,
    where: {
      and: [
        { project: { equals: projectID } },
        { user: { equals: user.id } },
      ],
    },
  })
  const membership = memberships.docs[0] as { role?: ProjectRole } | undefined
  return Boolean(membership?.role && roleRank[membership.role] >= roleRank[minimumRole])
}

async function projectDocumentWhere(
  req: PayloadRequest,
  minimumRole: ProjectRole,
): Promise<boolean | Where> {
  const user = workspaceUser(req)
  const organizationId = user?.organizationId
  if (!user || !organizationId) return false
  if (user.role === 'admin') {
    return { organizationId: { equals: organizationId } }
  }
  const projectIDs = await membershipProjectIDs(req, minimumRole)
  if (projectIDs.length === 0) return false
  return {
    and: [
      { organizationId: { equals: organizationId } },
      { project: { in: projectIDs } },
    ],
  }
}

export const readProjectDocuments: Access = ({ req }) => projectDocumentWhere(req, 'viewer')
export const editProjectDocuments: Access = ({ req }) => projectDocumentWhere(req, 'editor')
export const manageProjectDocuments: Access = ({ req }) => projectDocumentWhere(req, 'manager')

export function createProjectDocument(minimumRole: ProjectRole): Access {
  return async ({ data, req }) => {
    const projectID = relationshipID(data?.project)
    return projectID !== null && hasProjectRole(req, projectID, minimumRole)
  }
}

export const readProjectSpaces: Access = async ({ req }) => {
  const user = workspaceUser(req)
  const organizationId = user?.organizationId
  if (!user || !organizationId) return false
  if (user.role === 'admin') {
    return { organizationId: { equals: organizationId } }
  }
  const projectIDs = await membershipProjectIDs(req, 'viewer')
  if (projectIDs.length === 0) return false
  return {
    and: [
      { organizationId: { equals: organizationId } },
      { id: { in: projectIDs } },
    ],
  }
}

export const createProjectSpace: Access = ({ req, data }) => {
  const user = workspaceUser(req)
  return Boolean(
    user?.role === 'admin' &&
      user.organizationId &&
      data?.organizationId === user.organizationId,
  )
}

export const manageProjectSpaces: Access = async ({ req }) => {
  const user = workspaceUser(req)
  const organizationId = user?.organizationId
  if (!user || !organizationId) return false
  if (user.role === 'admin') {
    return { organizationId: { equals: organizationId } }
  }
  const projectIDs = await membershipProjectIDs(req, 'manager')
  if (projectIDs.length === 0) return false
  return {
    and: [
      { organizationId: { equals: organizationId } },
      { id: { in: projectIDs } },
    ],
  }
}
