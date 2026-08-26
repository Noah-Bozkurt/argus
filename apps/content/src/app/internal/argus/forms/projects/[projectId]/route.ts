import config from '@payload-config'
import { NextResponse } from 'next/server'
import { getPayload, type Payload } from 'payload'

import { internalIdentity } from '@/lib/argusCmsContract'
import { normalizeFormInput, type FormField } from '@/lib/argusFormsContract'
import { authorizeInternalProject } from '@/lib/internalProjectAccess'
import { isUUID } from '@/lib/projectScope'

type Project = { id: string; organizationId?: string; status?: string }

async function projectFor(payload: Payload, projectId: string, organizationId: string): Promise<Project | null> {
  const result = await payload.find({
    collection: 'project-spaces', depth: 0, limit: 1, overrideAccess: true, pagination: false,
    where: { and: [{ argusProjectId: { equals: projectId } }, { organizationId: { equals: organizationId } }] },
  })
  return (result.docs[0] as Project | undefined) ?? null
}

function formFields(value: unknown): FormField[] {
  return (Array.isArray(value) ? value : []).map((item) => {
    const field = item as Record<string, unknown>
    return {
      key: String(field.key ?? ''), label: String(field.label ?? ''), type: String(field.type ?? '') as FormField['type'],
      required: field.required === true,
      options: (Array.isArray(field.options) ? field.options : []).map((option) => typeof option === 'object' && option && 'value' in option ? String(option.value) : ''),
    }
  })
}

function formView(document: Record<string, unknown>) {
  return {
    id: String(document.id), name: document.name ?? '', slug: document.slug ?? '', description: document.description ?? '',
    success_message: document.successMessage ?? '', status: document.status ?? 'draft', fields: formFields(document.fields),
    updated_at: document.updatedAt ?? null,
  }
}

function submissionView(document: Record<string, unknown>) {
  const form = document.form
  return {
    id: String(document.id), form_id: typeof form === 'object' && form && 'id' in form ? String(form.id) : String(form ?? ''),
    values: document.values ?? {}, status: document.status ?? 'new', submitted_at: document.submittedAt ?? null,
  }
}

export async function GET(request: Request, { params }: { params: Promise<{ projectId: string }> }) {
  const identity = internalIdentity(request)
  const { projectId } = await params
  if (!identity) return NextResponse.json({ code: 'PERMISSION_DENIED' }, { status: 401 })
  if (!isUUID(projectId)) return NextResponse.json({ code: 'NOT_FOUND' }, { status: 404 })
  const payload = await getPayload({ config })
  const project = await projectFor(payload, projectId, identity.organizationId)
  if (!project) return NextResponse.json({ code: 'NOT_FOUND' }, { status: 404 })
  if (!await authorizeInternalProject(payload, identity, project, 'viewer')) return NextResponse.json({ code: 'PERMISSION_DENIED' }, { status: 403 })
  const forms = await payload.find({
    collection: 'form-definitions', depth: 0, limit: 100, sort: 'name', overrideAccess: true, pagination: false,
    where: { project: { equals: project.id } },
  })
  const formIds = forms.docs.map((form) => String(form.id))
  const requestedPage = Number.parseInt(new URL(request.url).searchParams.get('submission_page') ?? '1', 10)
  const submissionPage = Number.isFinite(requestedPage) ? Math.max(1, requestedPage) : 1
  const submissions = formIds.length === 0 ? { docs: [], page: 1, totalPages: 0, totalDocs: 0, hasNextPage: false, hasPrevPage: false } : await payload.find({
    collection: 'form-submissions', depth: 0, limit: 100, page: submissionPage, sort: '-submittedAt', overrideAccess: true,
    where: { and: [{ project: { equals: project.id } }, { form: { in: formIds } }] },
  })
  return NextResponse.json({
    forms: forms.docs.map((form) => formView(form as unknown as Record<string, unknown>)),
    submissions: submissions.docs.map((submission) => submissionView(submission as unknown as Record<string, unknown>)),
    submission_pagination: {
      page: submissions.page ?? 1, total_pages: submissions.totalPages ?? 0, total_docs: submissions.totalDocs ?? 0,
      has_next_page: submissions.hasNextPage ?? false, has_prev_page: submissions.hasPrevPage ?? false,
    },
  })
}

export async function POST(request: Request, { params }: { params: Promise<{ projectId: string }> }) {
  const identity = internalIdentity(request)
  const { projectId } = await params
  if (!identity) return NextResponse.json({ code: 'PERMISSION_DENIED' }, { status: 401 })
  if (!isUUID(projectId)) return NextResponse.json({ code: 'NOT_FOUND' }, { status: 404 })
  const body = await request.json().catch(() => null) as Record<string, unknown> | null
  if (!body || typeof body.operation !== 'string') return NextResponse.json({ code: 'INVALID_REQUEST' }, { status: 400 })
  const payload = await getPayload({ config })
  const project = await projectFor(payload, projectId, identity.organizationId)
  if (!project) return NextResponse.json({ code: 'NOT_FOUND' }, { status: 404 })
  try {
    if (body.operation === 'create_form') {
      if (project.status !== 'active') return NextResponse.json({ code: 'NOT_FOUND' }, { status: 404 })
      const actor = await authorizeInternalProject(payload, identity, project, 'editor')
      if (!actor) return NextResponse.json({ code: 'PERMISSION_DENIED' }, { status: 403 })
      const normalized = normalizeFormInput(body.form)
      if (!normalized) return NextResponse.json({ code: 'INVALID_FORM' }, { status: 400 })
      const created = await payload.create({
        collection: 'form-definitions', depth: 0, overrideAccess: true, user: actor as any,
        data: {
          organizationId: identity.organizationId, argusProjectId: projectId, project: project.id,
          name: normalized.name, slug: normalized.slug, description: normalized.description,
          successMessage: normalized.successMessage, status: normalized.published ? 'published' : 'draft',
          fields: normalized.fields.map((field) => ({ ...field, options: field.options.map((value) => ({ value })) })),
        },
      })
      return NextResponse.json({ form: formView(created as unknown as Record<string, unknown>) }, { status: 201 })
    }
    if (body.operation === 'update_form_status') {
      const actor = await authorizeInternalProject(payload, identity, project, 'editor')
      if (!actor) return NextResponse.json({ code: 'PERMISSION_DENIED' }, { status: 403 })
      const formId = typeof body.form_id === 'string' ? body.form_id : ''
      const status = typeof body.status === 'string' ? body.status : ''
      if (!isUUID(formId) || !['draft', 'published', 'archived'].includes(status) || (status === 'published' && project.status !== 'active')) return NextResponse.json({ code: 'INVALID_REQUEST' }, { status: 400 })
      const forms = await payload.find({ collection: 'form-definitions', depth: 0, limit: 1, overrideAccess: true, pagination: false, where: { and: [{ id: { equals: formId } }, { project: { equals: project.id } }] } })
      if (forms.docs.length !== 1) return NextResponse.json({ code: 'NOT_FOUND' }, { status: 404 })
      const updated = await payload.update({ collection: 'form-definitions', id: formId, depth: 0, overrideAccess: true, user: actor as any, data: { status: status as 'draft' | 'published' | 'archived' } })
      return NextResponse.json({ form: formView(updated as unknown as Record<string, unknown>) })
    }
    if (body.operation === 'update_submission_status') {
      const actor = await authorizeInternalProject(payload, identity, project, 'editor')
      if (!actor) return NextResponse.json({ code: 'PERMISSION_DENIED' }, { status: 403 })
      const submissionId = typeof body.submission_id === 'string' ? body.submission_id : ''
      const status = typeof body.status === 'string' ? body.status : ''
      if (!isUUID(submissionId) || !['new', 'reviewed', 'spam', 'archived'].includes(status)) return NextResponse.json({ code: 'INVALID_REQUEST' }, { status: 400 })
      const submissions = await payload.find({ collection: 'form-submissions', depth: 0, limit: 1, overrideAccess: true, pagination: false, where: { and: [{ id: { equals: submissionId } }, { project: { equals: project.id } }] } })
      if (submissions.docs.length !== 1) return NextResponse.json({ code: 'NOT_FOUND' }, { status: 404 })
      const updated = await payload.update({ collection: 'form-submissions', id: submissionId, depth: 0, overrideAccess: true, user: actor as any, data: { status: status as 'new' | 'reviewed' | 'spam' | 'archived' } })
      return NextResponse.json({ submission: submissionView(updated as unknown as Record<string, unknown>) })
    }
    if (body.operation === 'delete_submission') {
      const actor = await authorizeInternalProject(payload, identity, project, 'manager')
      if (!actor) return NextResponse.json({ code: 'PERMISSION_DENIED' }, { status: 403 })
      const submissionId = typeof body.submission_id === 'string' ? body.submission_id : ''
      if (!isUUID(submissionId)) return NextResponse.json({ code: 'INVALID_REQUEST' }, { status: 400 })
      const submissions = await payload.find({ collection: 'form-submissions', depth: 0, limit: 1, overrideAccess: true, pagination: false, where: { and: [{ id: { equals: submissionId } }, { project: { equals: project.id } }] } })
      if (submissions.docs.length !== 1) return NextResponse.json({ code: 'NOT_FOUND' }, { status: 404 })
      await payload.delete({ collection: 'form-submissions', id: submissionId, overrideAccess: true, user: actor as any })
      return new NextResponse(null, { status: 204 })
    }
  } catch (error) {
    console.error('Argus form operation failed', { operation: body.operation, projectId, userId: identity.userId, workspaceUserId: identity.workspaceUserId, error })
    return NextResponse.json({ code: 'FORM_OPERATION_FAILED' }, { status: 409 })
  }
  return NextResponse.json({ code: 'INVALID_REQUEST' }, { status: 400 })
}
