import type { Payload, Where } from 'payload'

import type { ArgusCmsIdentity } from './argusCmsContract'

export type InternalProject = {
  id: string | number
  organizationId?: string
  status?: string
}

export type InternalProjectRole = 'viewer' | 'editor' | 'manager'

type WorkspaceUser = {
  id: string | number
  organizationId?: string | null
  argusUserId?: string | null
  role?: 'owner' | 'admin' | 'member' | 'client' | null
}

const roleRank: Record<InternalProjectRole, number> = {
  viewer: 1,
  editor: 2,
  manager: 3,
}

export async function internalWorkspaceUser(
  payload: Payload,
  identity: ArgusCmsIdentity,
): Promise<WorkspaceUser | null> {
  const candidates: Where[] = [{ argusUserId: { equals: identity.userId } }]
  if (identity.workspaceUserId) candidates.unshift({ id: { equals: identity.workspaceUserId } })
  const users = await payload.find({
    collection: 'workspace-users',
    depth: 0,
    limit: 1,
    overrideAccess: true,
    pagination: false,
    where: {
      and: [
        { organizationId: { equals: identity.organizationId } },
        { or: candidates },
      ],
    },
  })
  return (users.docs[0] as WorkspaceUser | undefined) ?? null
}

export async function authorizeInternalProject(
  payload: Payload,
  identity: ArgusCmsIdentity,
  project: InternalProject,
  minimumRole: InternalProjectRole,
): Promise<WorkspaceUser | null> {
  if (project.organizationId !== identity.organizationId) return null
  const user = await internalWorkspaceUser(payload, identity)
  if (!user || user.organizationId !== identity.organizationId) return null
  if (user.role === 'owner' || user.role === 'admin') return user

  const memberships = await payload.find({
    collection: 'project-memberships',
    depth: 0,
    limit: 1,
    overrideAccess: true,
    pagination: false,
    where: {
      and: [
        { project: { equals: project.id } },
        { user: { equals: user.id } },
      ],
    },
  })
  const membership = memberships.docs[0] as { role?: InternalProjectRole } | undefined
  return membership?.role && roleRank[membership.role] >= roleRank[minimumRole] ? user : null
}
