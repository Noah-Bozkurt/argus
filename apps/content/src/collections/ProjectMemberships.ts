import type { CollectionBeforeValidateHook, CollectionConfig, Where } from 'payload'
import {
  createProjectDocument,
  manageProjectDocuments,
  readProjectDocuments,
  relationshipID,
} from '@/access/projectAccess'
import { resolveProjectScope } from '@/lib/projectScope'

const validateMembership: CollectionBeforeValidateHook = async ({ data, operation, originalDoc, req }) => {
  if (!data) return data

  if (operation === 'update' && originalDoc) {
    data.project = originalDoc.project
    data.user = originalDoc.user
    data.organizationId = originalDoc.organizationId
  }

  const project = data.project ?? originalDoc?.project
  const user = data.user ?? originalDoc?.user
  const projectID = relationshipID(project)
  const userID = relationshipID(user)
  if (projectID === null || userID === null) {
    throw new Error('Project and user are required')
  }

  const scope = await resolveProjectScope(req, projectID)
  const targetUser = await req.payload.findByID({
    collection: 'workspace-users',
    id: userID,
    depth: 0,
    overrideAccess: true,
  }) as { organizationId?: string }
  if (targetUser.organizationId !== scope.organizationId) {
    throw new Error('Project membership cannot cross organization boundaries')
  }

  const duplicate = await req.payload.find({
    collection: 'project-memberships',
    depth: 0,
    limit: 1,
    overrideAccess: true,
    pagination: false,
    where: {
      and: [
        { project: { equals: projectID } },
        { user: { equals: userID } },
        ...(operation === 'update' && originalDoc?.id
          ? [{ id: { not_equals: originalDoc.id } }]
          : []),
      ],
    },
  })
  if (duplicate.docs.length > 0) {
    throw new Error('This user already has a role in the project')
  }

  data.project = projectID
  data.user = userID
  data.organizationId = scope.organizationId
  return data
}

export const ProjectMemberships: CollectionConfig = {
  slug: 'project-memberships',
  admin: {
    group: 'App Data',
    defaultColumns: ['project', 'user', 'role'],
  },
  access: {
    create: createProjectDocument('manager'),
    read: async (args) => {
      const user = args.req.user as { id?: string | number; role?: string } | null
      if (user?.role === 'client' && user.id) {
        return { user: { equals: user.id } } as Where
      }
      return readProjectDocuments(args)
    },
    update: manageProjectDocuments,
    delete: manageProjectDocuments,
  },
  hooks: {
    beforeValidate: [validateMembership],
  },
  fields: [
    {
      name: 'organizationId',
      type: 'text',
      required: true,
      index: true,
      admin: { readOnly: true },
    },
    {
      name: 'project',
      type: 'relationship',
      relationTo: 'project-spaces',
      required: true,
      index: true,
      admin: {
        description: 'Immutable after creation.',
      },
    },
    {
      name: 'user',
      type: 'relationship',
      relationTo: 'workspace-users',
      required: true,
      index: true,
      admin: {
        description: 'Immutable after creation.',
      },
    },
    {
      name: 'role',
      type: 'select',
      required: true,
      defaultValue: 'viewer',
      options: [
        { label: 'Manager', value: 'manager' },
        { label: 'Editor', value: 'editor' },
        { label: 'Viewer', value: 'viewer' },
      ],
    },
  ],
}
