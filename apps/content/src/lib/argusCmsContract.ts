import { timingSafeEqual } from 'crypto'

export const MODEL_SLUG_PATTERN = /^[a-z][a-z0-9_]{0,119}$/
export const FIELD_KEY_PATTERN = /^[a-z][a-z0-9_]{0,119}$/
export const FIELD_TYPES = ['text', 'textarea', 'number', 'boolean', 'date', 'datetime', 'json'] as const
const UUID_PATTERN = /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i

export type CmsFieldInput = {
  key: string
  label: string
  type: (typeof FIELD_TYPES)[number]
  required: boolean
}

export type ArgusCmsIdentity = {
  organizationId: string
  userId: string
}

export function internalIdentity(request: Request): ArgusCmsIdentity | null {
  const expected = process.env.ARGUS_CONTENT_SYNC_TOKEN ?? ''
  const supplied = request.headers.get('authorization')?.replace(/^Bearer\s+/i, '') ?? ''
  if (expected.length < 32 || supplied.length !== expected.length) return null
  if (!timingSafeEqual(Buffer.from(supplied), Buffer.from(expected))) return null

  const organizationId = request.headers.get('x-argus-org-id') ?? ''
  const userId = request.headers.get('x-argus-user-id') ?? ''
  return UUID_PATTERN.test(organizationId) && UUID_PATTERN.test(userId) ? { organizationId, userId } : null
}

export function normalizeModelInput(input: unknown): {
  name: string
  slug: string
  description: string
  publicRead: boolean
  fields: CmsFieldInput[]
} | null {
  if (!input || typeof input !== 'object') return null
  const raw = input as Record<string, unknown>
  const name = typeof raw.name === 'string' ? raw.name.trim() : ''
  const slug = typeof raw.slug === 'string' ? raw.slug.trim().toLowerCase() : ''
  const description = typeof raw.description === 'string' ? raw.description.trim() : ''
  if (!name || name.length > 160 || !MODEL_SLUG_PATTERN.test(slug) || description.length > 4000) return null
  if (!Array.isArray(raw.fields) || raw.fields.length < 1 || raw.fields.length > 50) return null

  const seen = new Set<string>()
  const fields: CmsFieldInput[] = []
  for (const item of raw.fields) {
    if (!item || typeof item !== 'object') return null
    const field = item as Record<string, unknown>
    const key = typeof field.key === 'string' ? field.key.trim().toLowerCase() : ''
    const label = typeof field.label === 'string' ? field.label.trim() : ''
    const type = typeof field.type === 'string' ? field.type : ''
    if (!FIELD_KEY_PATTERN.test(key) || !label || label.length > 160 || !FIELD_TYPES.includes(type as CmsFieldInput['type']) || seen.has(key)) return null
    seen.add(key)
    fields.push({ key, label, type: type as CmsFieldInput['type'], required: field.required === true })
  }
  return { name, slug, description, publicRead: raw.public_read === true, fields }
}

export function validateValues(fields: CmsFieldInput[], input: unknown): Record<string, unknown> | null {
  if (!input || typeof input !== 'object' || Array.isArray(input)) return null
  const values = input as Record<string, unknown>
  const allowed = new Map(fields.map((field) => [field.key, field]))
  if (Object.keys(values).some((key) => !allowed.has(key))) return null

  for (const field of fields) {
    const value = values[field.key]
    if (field.required && (value === undefined || value === null || value === '')) return null
    if (value === undefined || value === null || value === '') continue
    const valid = field.type === 'number'
      ? typeof value === 'number' && Number.isFinite(value)
      : field.type === 'boolean'
        ? typeof value === 'boolean'
        : field.type === 'json'
          ? true
          : typeof value === 'string' && (!['date', 'datetime'].includes(field.type) || !Number.isNaN(Date.parse(value)))
    if (!valid) return null
  }
  return values
}
