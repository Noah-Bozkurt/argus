import config from '@payload-config'
import { NextResponse } from 'next/server'
import { getPayload } from 'payload'

import { internalIdentity } from '@/lib/argusCmsContract'
import { formSubmissionsCsv, MAX_FORM_EXPORT_ROWS, type FormField } from '@/lib/argusFormsContract'
import { authorizeInternalProject } from '@/lib/internalProjectAccess'
import { isUUID } from '@/lib/projectScope'

export async function GET(request: Request, { params }: { params: Promise<{ projectId: string; formId: string }> }) {
  const identity = internalIdentity(request)
  const { projectId, formId } = await params
  if (!identity) return NextResponse.json({ code: 'PERMISSION_DENIED' }, { status: 401 })
  if (!isUUID(projectId) || !isUUID(formId)) return NextResponse.json({ code: 'NOT_FOUND' }, { status: 404 })
  const payload = await getPayload({ config })
  const projects = await payload.find({
    collection: 'project-spaces', depth: 0, limit: 1, overrideAccess: true, pagination: false,
    where: { and: [{ argusProjectId: { equals: projectId } }, { organizationId: { equals: identity.organizationId } }] },
  })
  const project = projects.docs[0] as { id: string; organizationId?: string } | undefined
  if (!project) return NextResponse.json({ code: 'NOT_FOUND' }, { status: 404 })
  if (!await authorizeInternalProject(payload, identity, project, 'viewer')) return NextResponse.json({ code: 'PERMISSION_DENIED' }, { status: 403 })

  const forms = await payload.find({
    collection: 'form-definitions', depth: 0, limit: 1, overrideAccess: true, pagination: false,
    where: { and: [{ id: { equals: formId } }, { project: { equals: project.id } }] },
  })
  const form = forms.docs[0] as { id: string; slug?: string; fields?: Array<Record<string, unknown>> } | undefined
  if (!form) return NextResponse.json({ code: 'NOT_FOUND' }, { status: 404 })

  const first = await payload.find({
    collection: 'form-submissions', depth: 0, limit: 500, page: 1, sort: 'submittedAt', overrideAccess: true,
    where: { and: [{ project: { equals: project.id } }, { form: { equals: form.id } }] },
  })
  if (first.totalDocs > MAX_FORM_EXPORT_ROWS) return NextResponse.json({ code: 'EXPORT_TOO_LARGE', max_rows: MAX_FORM_EXPORT_ROWS }, { status: 422 })

  const docs = [...first.docs]
  for (let page = 2; page <= first.totalPages; page += 1) {
    const next = await payload.find({
      collection: 'form-submissions', depth: 0, limit: 500, page, sort: 'submittedAt', overrideAccess: true,
      where: { and: [{ project: { equals: project.id } }, { form: { equals: form.id } }] },
    })
    docs.push(...next.docs)
  }

  const fields: FormField[] = (form.fields ?? []).map((raw) => ({
    key: String(raw.key ?? ''), label: String(raw.label ?? ''), type: String(raw.type ?? 'text') as FormField['type'], required: raw.required === true,
    options: (Array.isArray(raw.options) ? raw.options : []).map((option) => typeof option === 'object' && option && 'value' in option ? String(option.value) : ''),
  }))
  const csv = formSubmissionsCsv(fields, docs.map((doc) => ({ id: String(doc.id), status: String(doc.status ?? ''), submittedAt: doc.submittedAt, values: doc.values })))
  return new NextResponse(csv, {
    headers: {
      'content-type': 'text/csv; charset=utf-8',
      'content-disposition': `attachment; filename="${String(form.slug ?? 'form')}-submissions.csv"`,
      'cache-control': 'private, no-store',
      'x-content-type-options': 'nosniff',
    },
  })
}
