export type ContentField = {
  key: string
  label: string
  type: 'text' | 'textarea' | 'number' | 'boolean' | 'date' | 'datetime' | 'json' | 'relationship'
  required: boolean
  target_model_id: string | null
  has_many: boolean
}

export type ContentModel = {
  id: string
  name: string
  slug: string
  description: string
  public_read: boolean
  content_role: 'collection' | 'page' | 'component'
  allowed_component_ids: string[]
  schema_version: number
  status: string
  fields: ContentField[]
}

export type ContentRecord = {
  id: string
  model_id: string
  values: Record<string, unknown>
  layout: ContentBlock[]
  editorial_status: 'draft' | 'published'
  lifecycle_status: string
  published_at: string | null
  updated_at: string | null
}

export type ContentBlock = {
  id: string
  component: string
  values: Record<string, unknown>
}

export type ContentWorkspace = {
  project_status: string
  models: ContentModel[]
  records: ContentRecord[]
  relations: Array<{ id: string; source_record_id: string; target_record_id: string; field_key: string }>
}

export type MediaAsset = {
  id: string
  filename: string
  mime_type: string
  filesize: number
  width: number | null
  height: number | null
  alt: string
  caption: string
  public_read: boolean
  url: string | null
  sizes: Record<string, unknown>
  updated_at: string | null
}

export type FormField = {
  key: string
  label: string
  type: 'text' | 'email' | 'textarea' | 'number' | 'boolean' | 'select'
  required: boolean
  options: string[]
}

export type ProjectForm = {
  id: string
  name: string
  slug: string
  description: string
  success_message: string
  status: 'draft' | 'published' | 'archived'
  fields: FormField[]
  updated_at: string | null
}

export type FormSubmission = {
  id: string
  form_id: string
  values: Record<string, unknown>
  status: 'new' | 'reviewed' | 'spam' | 'archived'
  submitted_at: string | null
}

export type FormsWorkspace = {
  forms: ProjectForm[]
  submissions: FormSubmission[]
  submission_pagination: { page: number; total_pages: number; total_docs: number; has_next_page: boolean; has_prev_page: boolean }
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
  content_role: ContentModel['content_role']
  allowed_component_ids: string[]
  fields: ContentField[]
}): Promise<void> {
  await request(projectId, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ operation: 'create_model', model }) })
}

export async function saveContentRecord(projectId: string, input: {
  model_id: string
  record_id?: string
  values: Record<string, unknown>
  layout: ContentBlock[]
  publish: boolean
  relationships: Record<string, string[]>
}): Promise<void> {
  await request(projectId, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ operation: 'save_record', ...input }) })
}

async function mediaRequest<T>(projectId: string, init: RequestInit = {}): Promise<T> {
  const response = await fetch(`${contentApi}/internal/argus/media/projects/${projectId}`, {
    ...init, cache: 'no-store', headers: { ...headers(), ...(init.headers ?? {}) },
  })
  if (!response.ok) throw new Error(`Content service ${response.status}: ${await response.text()}`)
  if (response.status === 204) return undefined as T
  return response.json()
}

export async function getMediaLibrary(projectId: string): Promise<MediaAsset[]> {
  return (await mediaRequest<{ media: MediaAsset[] }>(projectId)).media
}

export async function uploadMedia(projectId: string, formData: FormData): Promise<void> {
  await mediaRequest(projectId, { method: 'POST', body: formData })
}

export async function deleteMedia(projectId: string, mediaId: string): Promise<void> {
  const response = await fetch(`${contentApi}/internal/argus/media/projects/${projectId}?media_id=${encodeURIComponent(mediaId)}`, {
    method: 'DELETE', cache: 'no-store', headers: headers(),
  })
  if (!response.ok) throw new Error(`Content service ${response.status}: ${await response.text()}`)
}

export async function updateMedia(projectId: string, input: { media_id: string; alt: string; caption: string; public_read: boolean }): Promise<void> {
  await mediaRequest(projectId, { method: 'PATCH', headers: { 'content-type': 'application/json' }, body: JSON.stringify(input) })
}

async function formsRequest<T>(projectId: string, init: RequestInit = {}): Promise<T> {
  const response = await fetch(`${contentApi}/internal/argus/forms/projects/${projectId}`, {
    ...init, cache: 'no-store', headers: { ...headers(), ...(init.headers ?? {}) },
  })
  if (!response.ok) throw new Error(`Content service ${response.status}: ${await response.text()}`)
  if (response.status === 204) return undefined as T
  return response.json()
}

export const getFormsWorkspace = (projectId: string, submissionPage = 1): Promise<FormsWorkspace> => {
  return fetch(`${contentApi}/internal/argus/forms/projects/${projectId}?submission_page=${encodeURIComponent(String(submissionPage))}`, {
    cache: 'no-store', headers: headers(),
  }).then(async (response) => {
    if (!response.ok) throw new Error(`Content service ${response.status}: ${await response.text()}`)
    return response.json() as Promise<FormsWorkspace>
  })
}

export async function createProjectForm(projectId: string, form: {
  name: string; slug: string; description: string; success_message: string; published: boolean; fields: FormField[]
}): Promise<void> {
  await formsRequest(projectId, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ operation: 'create_form', form }) })
}

export async function updateProjectFormStatus(projectId: string, formId: string, status: ProjectForm['status']): Promise<void> {
  await formsRequest(projectId, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ operation: 'update_form_status', form_id: formId, status }) })
}

export async function updateFormSubmissionStatus(projectId: string, submissionId: string, status: FormSubmission['status']): Promise<void> {
  await formsRequest(projectId, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ operation: 'update_submission_status', submission_id: submissionId, status }) })
}

export async function deleteFormSubmission(projectId: string, submissionId: string): Promise<void> {
  await formsRequest(projectId, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ operation: 'delete_submission', submission_id: submissionId }) })
}
