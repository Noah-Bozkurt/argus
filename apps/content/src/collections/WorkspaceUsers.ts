import type { CollectionBeforeValidateHook, CollectionConfig, Where } from 'payload'

const UUID_PATTERN = /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i

type WorkspaceUser = {
  id?: string | number
  organizationId?: string | null
  role?: 'admin' | 'member' | null
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
      data.role = 'admin'
      return data
    }

    if (!actor || actor.role !== 'admin' || !actor.organizationId) {
      throw new Error('Only organization admins can create workspace users')
    }
    data.organizationId = actor.organizationId
    return data
  }

  if (originalDoc) {
    data.organizationId = originalDoc.organizationId
    if (actor?.role !== 'admin') {
      data.role = originalDoc.role
      data.argusUserId = originalDoc.argusUserId
    }
  }
  return data
}

export const WorkspaceUsers: CollectionConfig = {
  slug: 'workspace-users',
  auth: true,
  admin: {
    useAsTitle: 'email',
    group: 'Argus',
  },
  access: {
    create: async ({ req }) => {
      const actor = req.user as WorkspaceUser | null
      if (actor?.role === 'admin' && actor.organizationId) return true
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
      if (user.role === 'admin') {
        return { organizationId: { equals: user.organizationId } } as Where
      }
      return { id: { equals: user.id } } as Where
    },
    update: ({ req }) => {
      const user = req.user as WorkspaceUser | null
      if (!user?.id || !user.organizationId) return false
      if (user.role === 'admin') {
        return { organizationId: { equals: user.organizationId } } as Where
      }
      return { id: { equals: user.id } } as Where
    },
    delete: ({ req }) => {
      const user = req.user as WorkspaceUser | null
      if (user?.role !== 'admin' || !user.organizationId) return false
      return { organizationId: { equals: user.organizationId } } as Where
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
      admin: {
        readOnly: true,
        description: 'Argus organization UUID. This is the tenant boundary for content data.',
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
      admin: {
        description: 'Optional link to the Argus control-plane user UUID.',
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
      options: [
        { label: 'Organization admin', value: 'admin' },
        { label: 'Member', value: 'member' },
      ],
    },
  ],
}
