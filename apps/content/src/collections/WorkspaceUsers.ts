import type { CollectionBeforeValidateHook, CollectionConfig, Where } from 'payload'

const UUID_PATTERN = /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i

type WorkspaceRole = 'owner' | 'admin' | 'member' | 'client'
type WorkspaceUser = {
  id?: string | number
  organizationId?: string | null
  argusUserId?: string | null
  role?: WorkspaceRole | null
}

function canManageUsers(user: WorkspaceUser | null): boolean {
  return Boolean(user && (user.role === 'owner' || user.role === 'admin') && user.organizationId)
}

const protectTenantFields: CollectionBeforeValidateHook = async ({ data, operation, originalDoc, req }) => {
  if (!data) return data
  const actor = req.user as WorkspaceUser | null

  if (operation === 'create') {
    const existing = await req.payload.find({
      collection: 'workspace-users',
      depth: 0,
      limit: 1,
      overrideAccess: true,
      pagination: false,
    })
    const bootstrap = existing.docs.length === 0 && !actor
    if (bootstrap) {
      if (typeof data.organizationId !== 'string' || !UUID_PATTERN.test(data.organizationId)) {
        throw new Error('The first workspace user requires a valid organization UUID')
      }
      data.role = 'owner'
      return data
    }

    if (!canManageUsers(actor)) {
      throw new Error('Only organization owners and admins can create workspace users')
    }
    if (actor?.role === 'admin' && data.role === 'owner') {
      throw new Error('Only an owner can create another owner')
    }
    data.organizationId = actor?.organizationId
    if (!data.role) data.role = 'member'
    return data
  }

  if (originalDoc) {
    data.organizationId = originalDoc.organizationId
    data.argusUserId = originalDoc.argusUserId

    if (actor?.role !== 'owner') {
      if (actor?.role !== 'admin') {
        data.role = originalDoc.role
      } else if (originalDoc.role === 'owner' || data.role === 'owner') {
        data.role = originalDoc.role
      }
    }
  }
  return data
}

export const WorkspaceUsers: CollectionConfig = {
  slug: 'workspace-users',
  auth: {
    tokenExpiration: 8 * 60 * 60,
    maxLoginAttempts: 5,
    lockTime: 10 * 60 * 1000,
    useSessions: true,
    cookies: {
      secure: (process.env.PAYLOAD_PUBLIC_URL ?? '').startsWith('https://'),
      sameSite: 'Lax',
      domain: process.env.ARGUS_AUTH_COOKIE_DOMAIN || undefined,
    },
  },
  admin: {
    useAsTitle: 'email',
    group: 'Argus',
  },
  access: {
    create: async ({ req }) => {
      const actor = req.user as WorkspaceUser | null
      if (canManageUsers(actor)) return true
      if (actor) return false
      const existing = await req.payload.find({
        collection: 'workspace-users',
        depth: 0,
        limit: 1,
        overrideAccess: true,
        pagination: false,
      })
      return existing.docs.length === 0
    },
    read: ({ req }) => {
      const user = req.user as WorkspaceUser | null
      if (!user?.id || !user.organizationId) return false
      if (user.role === 'owner' || user.role === 'admin') {
        return { organizationId: { equals: user.organizationId } } as Where
      }
      return { id: { equals: user.id } } as Where
    },
    update: ({ req }) => {
      const user = req.user as WorkspaceUser | null
      if (!user?.id || !user.organizationId) return false
      if (user.role === 'owner' || user.role === 'admin') {
        return { organizationId: { equals: user.organizationId } } as Where
      }
      return { id: { equals: user.id } } as Where
    },
    delete: ({ req }) => {
      const user = req.user as WorkspaceUser | null
      if (!user?.organizationId) return false
      if (user.role === 'owner') {
        return { organizationId: { equals: user.organizationId } } as Where
      }
      if (user.role === 'admin') {
        return {
          and: [
            { organizationId: { equals: user.organizationId } },
            { role: { not_equals: 'owner' } },
          ],
        } as Where
      }
      return false
    },
  },
  hooks: {
    beforeValidate: [protectTenantFields],
  },
  fields: [
    {
      name: 'displayName',
      type: 'text',
      required: true,
      maxLength: 160,
    },
    {
      name: 'organizationId',
      type: 'text',
      required: true,
      index: true,
      saveToJWT: true,
      admin: {
        readOnly: true,
        description: 'Argus organization UUID. This is the tenant boundary for application and content data.',
      },
      validate: (value: unknown) =>
        typeof value === 'string' && UUID_PATTERN.test(value)
          ? true
          : 'organizationId must be a UUID',
    },
    {
      name: 'argusUserId',
      type: 'text',
      index: true,
      saveToJWT: true,
      admin: {
        readOnly: true,
        description: 'Control-plane user UUID. Required for operator access; client-only accounts may leave this empty.',
      },
      validate: (value: unknown) =>
        value === null || value === undefined || value === '' || (typeof value === 'string' && UUID_PATTERN.test(value))
          ? true
          : 'argusUserId must be a UUID',
    },
    {
      name: 'role',
      type: 'select',
      required: true,
      defaultValue: 'member',
      saveToJWT: true,
      options: [
        { label: 'Owner', value: 'owner' },
        { label: 'Administrator', value: 'admin' },
        { label: 'Member', value: 'member' },
        { label: 'Client', value: 'client' },
      ],
    },
  ],
}
