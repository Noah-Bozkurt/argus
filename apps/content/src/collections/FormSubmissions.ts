import type { CollectionBeforeValidateHook, CollectionConfig } from 'payload'

import { editProjectDocuments, manageProjectDocuments, readProjectDocuments, relationshipID } from '@/access/projectAccess'
import { resolveProjectScope } from '@/lib/projectScope'

const scopeSubmission: CollectionBeforeValidateHook = async ({ data, operation, originalDoc, req }) => {
  if (!data) return data
  if (operation === 'update' && originalDoc) {
    data.project = originalDoc.project
    data.form = originalDoc.form
    data.organizationId = originalDoc.organizationId
    data.argusProjectId = originalDoc.argusProjectId
    data.values = originalDoc.values
    data.sourceHash = originalDoc.sourceHash
    data.rateWindow = originalDoc.rateWindow
    data.rateKey = originalDoc.rateKey
    data.submittedAt = originalDoc.submittedAt
  }
  const scope = await resolveProjectScope(req, data.project ?? originalDoc?.project)
  const formId = relationshipID(data.form ?? originalDoc?.form)
  if (formId === null) throw new Error('Submission requires a form')
  const form = await req.payload.findByID({ collection: 'form-definitions', id: formId, depth: 0, overrideAccess: true }) as { project?: unknown }
  if (relationshipID(form.project) !== scope.projectID) throw new Error('Submission form must belong to the same project')
  data.project = scope.projectID
  data.form = formId
  data.organizationId = scope.organizationId
  data.argusProjectId = scope.argusProjectId
  if (operation === 'create') data.submittedAt = new Date().toISOString()
  return data
}

export const FormSubmissions: CollectionConfig = {
  slug: 'form-submissions',
  admin: { group: 'Content', defaultColumns: ['form', 'status', 'submittedAt', 'project'] },
  access: {
    create: () => false, read: readProjectDocuments,
    update: editProjectDocuments, delete: manageProjectDocuments,
  },
  hooks: { beforeValidate: [scopeSubmission] },
  fields: [
    { name: 'organizationId', type: 'text', required: true, index: true, admin: { readOnly: true } },
    { name: 'argusProjectId', type: 'text', required: true, index: true, admin: { readOnly: true } },
    { name: 'project', type: 'relationship', relationTo: 'project-spaces', required: true, index: true, admin: { readOnly: true } },
    { name: 'form', type: 'relationship', relationTo: 'form-definitions', required: true, index: true, admin: { readOnly: true } },
    { name: 'values', type: 'json', required: true, admin: { readOnly: true } },
    {
      name: 'status', type: 'select', required: true, defaultValue: 'new',
      options: [{ label: 'New', value: 'new' }, { label: 'Reviewed', value: 'reviewed' }, { label: 'Spam', value: 'spam' }, { label: 'Archived', value: 'archived' }],
    },
    { name: 'sourceHash', type: 'text', required: true, index: true, admin: { hidden: true } },
    { name: 'rateWindow', type: 'text', required: true, index: true, admin: { hidden: true } },
    { name: 'rateKey', type: 'text', required: true, unique: true, index: true, admin: { hidden: true } },
    { name: 'submittedAt', type: 'date', required: true, index: true, admin: { readOnly: true } },
  ],
}
