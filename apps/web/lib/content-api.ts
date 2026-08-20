export type ContentField = {
  key: string
  label: string
  type: 'text' | 'textarea' | 'number' | 'boolean' | 'date' | 'datetime' | 'json'
  required: boolean
}

export type ContentModel = {
  id: string
  name: string
  slug: string
  description: string
  public_read: boolean
  schema_version: number
  status: string
  fields: ContentField[]
}

export type ContentRecord = {
  id: string
  model_id: string
  values: Record<string, unknown>
  editorial_status: 'draft' | 'published'
  lifecycle_status: string
  published_at: string | null
  updated_at: string | null
}

export type ContentWorkspace = {
  project_status: string
  models: ContentModel[]
  records: ContentRecord[]
}

const contentApi = process.env.ARGUS_CONTENT_URL ?? 'http://content:3000'

function headers(): Record<string, string> {
  const token = process.env.ARGUS_CONTENT_SYNC_TOKEN
  const organizationId = process.env.ARGUS_ORG_ID
  const userId = process.env.ARGUS_USER_ID
  if (!token || !organizationId || !userId) throw new Error('Argus content integration is not configured')
  return { authorization: `Bearer ${token}`, 'x-argus-org-id': organizationId, 'x-argus-user-id': userId }
}

async function request<T>(projectId: string, init: RequestInit = {}): Promise<T> {
  const response = await fetch(`${contentApi}/internal/argus/cms/projects/${projectId}`, {
    ...init,
    cache: 'no-store',
    headers: { ...headers(), ...(init.headers ?? {}) },
  })
  if (!response.ok) throw new Error(`Content service ${response.status}: ${await response.text()}`)
  return response.json()
}

export const getContentWorkspace = (projectId: string): Promise<ContentWorkspace> => request(projectId)

export async function createContentModel(projectId: string, model: {
  name: string
  slug: string
  description: string
  public_read: boolean
  fields: ContentField[]
}): Promise<void> {
  await request(projectId, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ operation: 'create_model', model }) })
}

export async function saveContentRecord(projectId: string, input: {
  model_id: string
  record_id?: string
  values: Record<string, unknown>
  publish: boolean
}): Promise<void> {
  await request(projectId, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ operation: 'save_record', ...input }) })
}
