import { currentSessionToken, getWorkspaceUser } from './auth'

export type ContentField = {
  key: string
  label: string
  type: 'text' | 'textarea' | 'number' | 'boolean' | 'date' | 'datetime' | 'json' | 'relationship' | 'media'
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
  status: 'active' | 'archived'
  fields: ContentField[]
}

export type ContentRecord = {
  id: string
  model_id: string
  values: Record<string, unknown>
  layout: ContentBlock[]
  editorial_status: 'draft' | 'published'
  lifecycle_status: 'active' | 'archived'
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
  pagination: {
    records: { page: number; total_pages: number; total_docs: number; has_next_page: boolean; has_prev_page: boolean }
  }
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

async function headers(): Promise<Record<string, string>> {
  const token = process.env.ARGUS_CONTENT_SYNC_TOKEN
  const sessionToken = currentSessionToken()
  if (!token || !sessionToken) throw new Error('Argus content integration is not configured')
  const user = await getWorkspaceUser(sessionToken)
  if (!user?.organizationId || !user.argusUserId) throw new Error('The current user cannot access the Argus content integration')
  return {
    authorization: `Bearer ${token}`,
    'x-argus-org-id': user.organizationId,
    'x-argus-user-id': user.argusUserId,
    'x-argus-workspace-user-id': user.id,
  }
}

async function request<T>(projectId: string, init: RequestInit = {}, query = ''): Promise<T> {
  const response = await fetch(`${contentApi}/internal/argus/cms/projects/${projectId}${query}`, {
    ...init,
    cache: 'no-store',
    headers: { ...(await headers()), ...(init.headers ?? {}) },
  })
  if (!response.ok) throw new Error(`Content service ${response.status}: ${await response.text()}`)
  if (response.status === 204) return undefined as T
  return response.json()
}

export const getContentWorkspace = (projectId: string, recordPage = 1): Promise<ContentWorkspace> => request(projectId, {}, `?record_page=${encodeURIComponent(String(recordPage))}`)

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

export async function updateContentModel(projectId: string, modelId: string, model: {
  name: string
  slug: string
  description: string
  public_read: boolean
  content_role: ContentModel['content_role']
  allowed_component_ids: string[]
  fields: ContentField[]
}): Promise<void> {
  await request(projectId, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ operation: 'update_model', model_id: modelId, model }) })
}

export async function setContentModelStatus(projectId: string, modelId: string, status: 'active' | 'archived'): Promise<void> {
  await request(projectId, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ operation: 'set_model_status', model_id: modelId, status }) })
}

export async function deleteContentModel(projectId: string, modelId: string): Promise<void> {
  await request(projectId, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ operation: 'delete_model', model_id: modelId }) })
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

export async function setContentRecordStatus(projectId: string, recordId: string, status: 'active' | 'archived'): Promise<void> {
  await request(projectId, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ operation: 'set_record_status', record_id: recordId, status }) })
}

export async function deleteContentRecord(projectId: string, recordId: string): Promise<void> {
  await request(projectId, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ operation: 'delete_record', record_id: recordId }) })
}

async function mediaRequest<T>(projectId: string, init: RequestInit = {}): Promise<T> {
  const response = await fetch(`${contentApi}/internal/argus/media/projects/${projectId}`, {
    ...init, cache: 'no-store', headers: { ...(await headers()), ...(init.headers ?? {}) },
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
    method: 'DELETE', cache: 'no-store', headers: await headers(),
  })
  if (!response.ok) throw new Error(`Content service ${response.status}: ${await response.text()}`)
}

export async function updateMedia(projectId: string, input: { media_id: string; alt: string; caption: string; public_read: boolean }): Promise<void> {
  await mediaRequest(projectId, { method: 'PATCH', headers: { 'content-type': 'application/json' }, body: JSON.stringify(input) })
}

async function formsRequest<T>(projectId: string, init: RequestInit = {}): Promise<T> {
  const response = await fetch(`${contentApi}/internal/argus/forms/projects/${projectId}`, {
    ...init, cache: 'no-store', headers: { ...(await headers()), ...(init.headers ?? {}) },
  })
  if (!response.ok) throw new Error(`Content service ${response.status}: ${await response.text()}`)
  if (response.status === 204) return undefined as T
  return response.json()
}

export async function getFormsWorkspace(projectId: string, submissionPage = 1): Promise<FormsWorkspace> {
  const response = await fetch(`${contentApi}/internal/argus/forms/projects/${projectId}?submission_page=${encodeURIComponent(String(submissionPage))}`, {
    cache: 'no-store', headers: await headers(),
  })
  if (!response.ok) throw new Error(`Content service ${response.status}: ${await response.text()}`)
  return response.json() as Promise<FormsWorkspace>
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

export async function downloadFormSubmissionsCsv(projectId: string, formId: string): Promise<{ body: ArrayBuffer; disposition: string }> {
  const response = await fetch(`${contentApi}/internal/argus/forms/projects/${encodeURIComponent(projectId)}/exports/${encodeURIComponent(formId)}`, {
    cache: 'no-store', headers: await headers(),
  })
  if (!response.ok) throw new Error(`Content service ${response.status}: ${await response.text()}`)
  return { body: await response.arrayBuffer(), disposition: response.headers.get('content-disposition') ?? 'attachment; filename="form-submissions.csv"' }
}
