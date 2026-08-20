import config from '@payload-config'
import { NextResponse } from 'next/server'
import { getPayload, type Payload } from 'payload'

import {
  FORM_RATE_LIMIT, FORM_RATE_WINDOW_MS, FORM_SLUG_PATTERN, readBoundedJson,
  submissionSourceHash, type FormField, validateSubmission,
} from '@/lib/argusFormsContract'
import { isUUID } from '@/lib/projectScope'

function corsHeaders(): Record<string, string> {
  return {
    'access-control-allow-origin': '*',
    'access-control-allow-methods': 'GET, POST, OPTIONS',
    'access-control-allow-headers': 'content-type',
  }
}

export function OPTIONS() {
  return new NextResponse(null, { status: 204, headers: corsHeaders() })
}

type PublicForm = {
  id: string
  project: string
  name: string
  slug: string
  description: string
  successMessage: string
  fields: FormField[]
}

async function resolveForm(payload: Payload, projectId: string, formSlug: string): Promise<PublicForm | null> {
  const projects = await payload.find({
    collection: 'project-spaces', depth: 0, limit: 1, overrideAccess: true, pagination: false,
    where: { and: [{ argusProjectId: { equals: projectId } }, { status: { equals: 'active' } }] },
  })
  const project = projects.docs[0]
  if (!project) return null
  const forms = await payload.find({
    collection: 'form-definitions', depth: 0, limit: 1, overrideAccess: true, pagination: false,
    where: { and: [{ project: { equals: project.id } }, { slug: { equals: formSlug } }, { status: { equals: 'published' } }] },
  })
  const form = forms.docs[0] as unknown as Record<string, unknown> | undefined
  if (!form) return null
  const fields = (Array.isArray(form.fields) ? form.fields : []).map((item) => {
    const field = item as Record<string, unknown>
    return {
      key: String(field.key ?? ''), label: String(field.label ?? ''), type: String(field.type ?? '') as FormField['type'],
      required: field.required === true,
      options: (Array.isArray(field.options) ? field.options : []).map((option) => typeof option === 'object' && option && 'value' in option ? String(option.value) : ''),
    }
  })
  return {
    id: String(form.id), project: String(project.id), name: String(form.name ?? ''), slug: String(form.slug ?? ''),
    description: String(form.description ?? ''), successMessage: String(form.successMessage ?? ''), fields,
  }
}

function publicView(form: PublicForm) {
  return {
    name: form.name, slug: form.slug, description: form.description,
    fields: form.fields.map(({ key, label, type, required, options }) => ({ key, label, type, required, options })),
  }
}

function postgresErrorCode(error: unknown): string | null {
  let current: unknown = error
  for (let depth = 0; depth < 5 && current && typeof current === 'object'; depth += 1) {
    if ('code' in current && typeof current.code === 'string') return current.code
    current = 'cause' in current ? current.cause : null
  }
  return null
}

export async function GET(_request: Request, { params }: { params: Promise<{ projectId: string; formSlug: string }> }) {
  const { projectId, formSlug } = await params
  if (!isUUID(projectId) || !FORM_SLUG_PATTERN.test(formSlug)) return NextResponse.json({ code: 'NOT_FOUND' }, { status: 404, headers: corsHeaders() })
  const form = await resolveForm(await getPayload({ config }), projectId, formSlug)
  if (!form) return NextResponse.json({ code: 'NOT_FOUND' }, { status: 404, headers: corsHeaders() })
  return NextResponse.json({ form: publicView(form) }, { headers: { ...corsHeaders(), 'cache-control': 'public, max-age=60, stale-while-revalidate=300' } })
}

export async function POST(request: Request, { params }: { params: Promise<{ projectId: string; formSlug: string }> }) {
  const { projectId, formSlug } = await params
  if (!isUUID(projectId) || !FORM_SLUG_PATTERN.test(formSlug)) return NextResponse.json({ code: 'NOT_FOUND' }, { status: 404, headers: corsHeaders() })
  const body = await readBoundedJson(request) as Record<string, unknown> | null
  if (!body || typeof body !== 'object' || Array.isArray(body)) return NextResponse.json({ code: 'INVALID_SUBMISSION' }, { status: 400, headers: corsHeaders() })
  if (typeof body._company === 'string' && body._company.trim()) {
    return NextResponse.json({ accepted: true }, { status: 202, headers: { ...corsHeaders(), 'cache-control': 'no-store' } })
  }
  const payload = await getPayload({ config })
  const form = await resolveForm(payload, projectId, formSlug)
  if (!form) return NextResponse.json({ code: 'NOT_FOUND' }, { status: 404, headers: corsHeaders() })
  const values = validateSubmission(form.fields, body.values)
  if (!values) return NextResponse.json({ code: 'INVALID_SUBMISSION' }, { status: 400, headers: corsHeaders() })
  const sourceHash = submissionSourceHash(
    process.env.PAYLOAD_SECRET ?? '', form.id,
    request.headers.get('x-forwarded-for'), request.headers.get('x-real-ip'),
  )
  const rateWindow = String(Math.floor(Date.now() / FORM_RATE_WINDOW_MS))
  let recent
  try {
    recent = await payload.count({
      collection: 'form-submissions', overrideAccess: true,
      where: { and: [{ form: { equals: form.id } }, { sourceHash: { equals: sourceHash } }, { rateWindow: { equals: rateWindow } }] },
    })
  } catch (error) {
    console.error('Public form rate lookup failed', { projectId, formSlug, error })
    return NextResponse.json({ code: 'SUBMISSION_UNAVAILABLE' }, { status: 503, headers: { ...corsHeaders(), 'cache-control': 'no-store', 'retry-after': '60' } })
  }
  if (recent.totalDocs >= FORM_RATE_LIMIT) {
    return NextResponse.json({ code: 'RATE_LIMITED' }, { status: 429, headers: { ...corsHeaders(), 'cache-control': 'no-store', 'retry-after': String(Math.ceil(FORM_RATE_WINDOW_MS / 1000)) } })
  }
  let submission
  try {
    submission = await payload.create({
      collection: 'form-submissions', depth: 0, overrideAccess: true,
      data: {
        organizationId: '', argusProjectId: projectId, project: form.project, form: form.id,
        values, status: 'new', sourceHash, rateWindow, rateKey: `${sourceHash}:${rateWindow}:${recent.totalDocs}`,
        submittedAt: new Date().toISOString(),
      },
    })
  } catch (error) {
    if (postgresErrorCode(error) === '23505') {
      return NextResponse.json({ code: 'RATE_LIMITED' }, { status: 429, headers: { ...corsHeaders(), 'cache-control': 'no-store', 'retry-after': String(Math.ceil(FORM_RATE_WINDOW_MS / 1000)) } })
    }
    console.error('Public form submission persistence failed', { projectId, formSlug, error })
    return NextResponse.json({ code: 'SUBMISSION_UNAVAILABLE' }, { status: 503, headers: { ...corsHeaders(), 'cache-control': 'no-store', 'retry-after': '60' } })
  }
  return NextResponse.json(
    { accepted: true, submission_id: submission.id, success_message: form.successMessage },
    { status: 201, headers: { ...corsHeaders(), 'cache-control': 'no-store' } },
  )
}
