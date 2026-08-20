import path from 'path'
import type { Access, CollectionBeforeValidateHook, CollectionConfig } from 'payload'

import { createProjectDocument, editProjectDocuments, manageProjectDocuments, readProjectDocuments } from '@/access/projectAccess'
import { resolveProjectScope } from '@/lib/projectScope'

const scopeMedia: CollectionBeforeValidateHook = async ({ data, operation, originalDoc, req }) => {
  if (!data) return data
  if (operation === 'update' && originalDoc) {
    data.project = originalDoc.project
    data.organizationId = originalDoc.organizationId
    data.argusProjectId = originalDoc.argusProjectId
  }
  const scope = await resolveProjectScope(req, data.project ?? originalDoc?.project)
  data.project = scope.projectID
  data.organizationId = scope.organizationId
  data.argusProjectId = scope.argusProjectId
  return data
}

const readMedia: Access = (args) => args.req.user
  ? readProjectDocuments(args)
  : { and: [{ publicRead: { equals: true } }, { 'project.status': { equals: 'active' } }] }

export const Media: CollectionConfig = {
  slug: 'media',
  admin: {
    group: 'Content',
    useAsTitle: 'alt',
    defaultColumns: ['filename', 'alt', 'project', 'width', 'height', 'updatedAt'],
    description: 'Project-owned images and optimized web variants.',
  },
  access: {
    create: createProjectDocument('editor'),
    read: readMedia,
    update: editProjectDocuments,
    delete: manageProjectDocuments,
  },
  hooks: { beforeValidate: [scopeMedia] },
  upload: {
    staticDir: path.resolve(process.env.ARGUS_MEDIA_DIR ?? path.resolve(process.cwd(), 'media')),
    mimeTypes: ['image/jpeg', 'image/png', 'image/webp', 'image/avif'],
    adminThumbnail: 'thumbnail',
    imageSizes: [
      { name: 'thumbnail', width: 320, height: 320, position: 'centre', withoutEnlargement: true },
      { name: 'medium', width: 960, height: 960, position: 'centre', withoutEnlargement: true },
      { name: 'large', width: 1920, height: 1920, position: 'centre', withoutEnlargement: true },
    ],
  },
  fields: [
    { name: 'organizationId', type: 'text', required: true, index: true, admin: { readOnly: true } },
    { name: 'argusProjectId', type: 'text', required: true, index: true, admin: { readOnly: true } },
    {
      name: 'project', type: 'relationship', relationTo: 'project-spaces', required: true, index: true,
      admin: { description: 'Immutable project ownership.' },
    },
    { name: 'alt', type: 'text', required: true, maxLength: 300 },
    { name: 'caption', type: 'textarea', maxLength: 2000 },
    {
      name: 'publicRead', type: 'checkbox', defaultValue: false,
      admin: { description: 'Allow this file to be delivered anonymously and used by a published site.' },
    },
  ],
}
