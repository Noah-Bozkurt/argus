import type { CollectionBeforeValidateHook, CollectionConfig } from 'payload'
import {
  createProjectSpace,
  manageProjectSpaces,
  readProjectSpaces,
  userOrganizationID,
} from '@/access/projectAccess'
import { isUUID } from '@/lib/projectScope'

const normalizeProjectScope: CollectionBeforeValidateHook = ({ data, operation, originalDoc, req }) => {
  if (!data) return data
  const organizationId = userOrganizationID(req)
  if (operation === 'create') {
    if (organizationId) {
      data.organizationId = organizationId
    } else if (typeof data.organizationId !== 'string' || !isUUID(data.organizationId)) {
      throw new Error('Project creation requires a valid organization UUID')
    }
  } else if (originalDoc) {
    data.organizationId = originalDoc.organizationId
    data.argusProjectId = originalDoc.argusProjectId
  }
  return data
}

export const ProjectSpaces: CollectionConfig = {
  slug: 'project-spaces',
  admin: {
    useAsTitle: 'name',
    group: 'App Data',
    defaultColumns: ['name', 'argusProjectId', 'status', 'clientId'],
  },
  access: {
    create: createProjectSpace,
    read: readProjectSpaces,
    update: manageProjectSpaces,
    delete: manageProjectSpaces,
  },
  hooks: {
    beforeValidate: [normalizeProjectScope],
  },
  fields: [
    {
      name: 'argusProjectId',
      label: 'Argus project UUID',
      type: 'text',
      required: true,
      unique: true,
      index: true,
      validate: (value: unknown) =>
        typeof value === 'string' && isUUID(value) ? true : 'argusProjectId must be a UUID',
    },
    {
      name: 'organizationId',
      type: 'text',
      required: true,
      index: true,
      admin: {
        readOnly: true,
        description: 'Argus organization UUID. Server-side project sync may stamp this with overrideAccess.',
      },
      validate: (value: unknown) =>
        typeof value === 'string' && isUUID(value) ? true : 'organizationId must be a UUID',
    },
    {
      name: 'name',
      type: 'text',
      required: true,
      maxLength: 160,
    },
    {
      name: 'clientId',
      type: 'text',
      index: true,
      admin: {
        description: 'Optional client UUID/reference. Personal projects leave this empty.',
      },
      validate: (value: unknown) =>
        value === null || value === undefined || value === '' || (typeof value === 'string' && isUUID(value))
          ? true
          : 'clientId must be a UUID',
    },
    {
      name: 'status',
      type: 'select',
      required: true,
      defaultValue: 'active',
      options: [
        { label: 'Active', value: 'active' },
        { label: 'Paused', value: 'paused' },
        { label: 'Archived', value: 'archived' },
      ],
    },
  ],
}
