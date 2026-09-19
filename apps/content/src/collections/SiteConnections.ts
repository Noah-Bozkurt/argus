import type { CollectionConfig } from 'payload'

// Only scoped internal handlers may access connection secrets or change a target.
export const SiteConnections: CollectionConfig = {
  slug: 'site-connections',
  admin: { hidden: true },
  access: { create: () => false, read: () => false, update: () => false, delete: () => false },
  fields: [
    { name: 'project', type: 'relationship', relationTo: 'project-spaces', required: true, unique: true },
    { name: 'siteURL', type: 'text', required: true },
    { name: 'previewURL', type: 'text' },
    { name: 'provider', type: 'select', required: true, options: ['pages', 'workers'] },
    { name: 'accountId', type: 'text', required: true },
    { name: 'target', type: 'text', required: true },
    { name: 'branch', type: 'text', required: true },
    { name: 'credential', type: 'text', required: true },
    { name: 'hook', type: 'text' },
    { name: 'components', type: 'json', defaultValue: [] },
  ],
}
