import type { CollectionConfig } from 'payload'

export const ContentReleases: CollectionConfig = {
  slug: 'content-releases',
  admin: { hidden: true },
  access: { create: () => false, read: () => false, update: () => false, delete: () => false },
  fields: [
    { name: 'project', type: 'relationship', relationTo: 'project-spaces', required: true, index: true },
    { name: 'requestId', type: 'text', required: true, unique: true },
    { name: 'snapshot', type: 'json', required: true },
    { name: 'status', type: 'select', required: true, defaultValue: 'queued', options: ['queued', 'triggering', 'building', 'deployed', 'failed', 'unknown'] },
    { name: 'providerId', type: 'text' },
    { name: 'errorCode', type: 'text' },
    { name: 'startedAt', type: 'date' },
    { name: 'completedAt', type: 'date' },
  ],
}
