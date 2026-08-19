import type { CollectionConfig } from 'payload'

export const WorkspaceUsers: CollectionConfig = {
  slug: 'workspace-users',
  auth: true,
  admin: {
    useAsTitle: 'email',
    group: 'Argus',
  },
  access: {
    create: ({ req }) => (req.user as { role?: string } | null)?.role === 'admin',
    read: ({ req }) => {
      const user = req.user as {
        id?: string | number
        organizationId?: string
        role?: string
      } | null
      if (!user?.id || !user.organizationId) return false
      if (user.role === 'admin') {
        return { organizationId: { equals: user.organizationId } }
      }
      return { id: { equals: user.id } }
    },
    update: ({ req }) => {
      const user = req.user as {
        id?: string | number
        organizationId?: string
        role?: string
      } | null
      if (!user?.id || !user.organizationId) return false
      if (user.role === 'admin') {
        return { organizationId: { equals: user.organizationId } }
      }
      return { id: { equals: user.id } }
    },
    delete: ({ req }) => {
      const user = req.user as { organizationId?: string; role?: string } | null
      if (user?.role !== 'admin' || !user.organizationId) return false
      return { organizationId: { equals: user.organizationId } }
    },
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
        description: 'Argus organization UUID. This is the tenant boundary for content data.',
      },
      validate: (value: unknown) =>
        typeof value === 'string' && /^[0-9a-f-]{36}$/i.test(value)
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
