import { createHmac } from 'crypto'

export const FORM_SLUG_PATTERN = /^[a-z][a-z0-9_]{0,119}$/
export const FORM_FIELD_TYPES = ['text', 'email', 'textarea', 'number', 'boolean', 'select'] as const
export const MAX_FORM_BODY_BYTES = 64 * 1024
export const FORM_RATE_LIMIT = 10
export const FORM_RATE_WINDOW_MS = 10 * 60 * 1000
export const MAX_FORM_EXPORT_ROWS = 10_000

export type FormField = {
  key: string
  label: string
  type: (typeof FORM_FIELD_TYPES)[number]
  required: boolean
  options: string[]
}

export function normalizeFormInput(input: unknown): {
  name: string
  slug: string
  description: string
  successMessage: string
  published: boolean
  fields: FormField[]
} | null {
  if (!input || typeof input !== 'object') return null
  const raw = input as Record<string, unknown>
  const name = typeof raw.name === 'string' ? raw.name.trim() : ''
  const slug = typeof raw.slug === 'string' ? raw.slug.trim().toLowerCase() : ''
  const description = typeof raw.description === 'string' ? raw.description.trim() : ''
  const successMessage = typeof raw.success_message === 'string' ? raw.success_message.trim() : ''
  if (!name || name.length > 160 || !FORM_SLUG_PATTERN.test(slug) || description.length > 2000 || !successMessage || successMessage.length > 500) return null
  if (!Array.isArray(raw.fields) || raw.fields.length < 1 || raw.fields.length > 30) return null
  const keys = new Set<string>()
  const fields: FormField[] = []
  for (const item of raw.fields) {
    if (!item || typeof item !== 'object') return null
    const field = item as Record<string, unknown>
    const key = typeof field.key === 'string' ? field.key.trim().toLowerCase() : ''
    const label = typeof field.label === 'string' ? field.label.trim() : ''
    const type = typeof field.type === 'string' ? field.type : ''
    if (!FORM_SLUG_PATTERN.test(key) || !label || label.length > 160 || !FORM_FIELD_TYPES.includes(type as FormField['type']) || keys.has(key)) return null
    keys.add(key)
    const options = Array.isArray(field.options)
      ? field.options.map((option) => typeof option === 'string' ? option.trim() : '').filter(Boolean)
      : []
    if (options.some((option) => option.length > 160) || new Set(options).size !== options.length || (type === 'select' && (options.length < 1 || options.length > 50))) return null
    fields.push({ key, label, type: type as FormField['type'], required: field.required === true, options: type === 'select' ? options : [] })
  }
  return { name, slug, description, successMessage, published: raw.published === true, fields }
}

const EMAIL_PATTERN = /^[^\s@]+@[^\s@]+\.[^\s@]+$/

export function validateSubmission(fields: FormField[], input: unknown): Record<string, unknown> | null {
  if (!input || typeof input !== 'object' || Array.isArray(input)) return null
  const values = input as Record<string, unknown>
  const known = new Map(fields.map((field) => [field.key, field]))
  if (Object.keys(values).some((key) => !known.has(key))) return null
  for (const field of fields) {
    const value = values[field.key]
    if (field.required && (value === undefined || value === null || value === '' || value === false)) return null
    if (value === undefined || value === null || value === '') continue
    const valid = field.type === 'number'
      ? typeof value === 'number' && Number.isFinite(value)
      : field.type === 'boolean'
        ? typeof value === 'boolean'
        : typeof value === 'string' && value.length <= (field.type === 'textarea' ? 4000 : 500)
          && (field.type !== 'email' || EMAIL_PATTERN.test(value))
          && (field.type !== 'select' || field.options.includes(value))
    if (!valid) return null
  }
  return Object.fromEntries(fields.filter((field) => values[field.key] !== undefined).map((field) => [field.key, values[field.key]]))
}

export function submissionSourceHash(secret: string, formId: string, forwardedFor: string | null, realIp: string | null): string {
  const source = (forwardedFor?.split(',')[0] ?? realIp ?? 'unknown').trim().slice(0, 200) || 'unknown'
  return createHmac('sha256', secret).update(`${formId}\0${source}`).digest('hex')
}

export async function readBoundedJson(request: Request): Promise<unknown | null> {
  const declared = Number(request.headers.get('content-length') ?? 0)
  if (Number.isFinite(declared) && declared > MAX_FORM_BODY_BYTES) return null
  if (!request.body) return null
  const reader = request.body.getReader()
  const chunks: Uint8Array[] = []
  let size = 0
  while (true) {
    const { done, value } = await reader.read()
    if (done) break
    size += value.byteLength
    if (size > MAX_FORM_BODY_BYTES) {
      await reader.cancel()
      return null
    }
    chunks.push(value)
  }
  try {
    return JSON.parse(new TextDecoder().decode(Buffer.concat(chunks)))
  } catch {
    return null
  }
}

export function csvCell(value: unknown): string {
  let text = value === undefined || value === null ? '' : typeof value === 'object' ? JSON.stringify(value) : String(value)
  if (/^[\s]*[=+\-@]/.test(text) || /^[\t\r\n]/.test(text)) text = `'${text}`
  return `"${text.replaceAll('"', '""')}"`
}

export function formSubmissionsCsv(fields: FormField[], submissions: Array<{ id: string; status: string; submittedAt?: string | null; values?: unknown }>): string {
  const header = ['submission_id', 'status', 'submitted_at', ...fields.map((field) => field.key)]
  const rows = submissions.map((submission) => {
    const values = submission.values && typeof submission.values === 'object' && !Array.isArray(submission.values) ? submission.values as Record<string, unknown> : {}
    return [submission.id, submission.status, submission.submittedAt ?? '', ...fields.map((field) => values[field.key])]
  })
  return `\uFEFF${[header, ...rows].map((row) => row.map(csvCell).join(',')).join('\r\n')}\r\n`
}
