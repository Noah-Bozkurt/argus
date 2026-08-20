#!/usr/bin/env node

import { pathToFileURL } from 'node:url'

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i

function required(env, key) {
  const value = env[key]
  if (!value) throw new Error(`missing ${key}`)
  return value
}

async function json(fetchImpl, url, init = {}) {
  const response = await fetchImpl(url, init)
  const body = await response.json().catch(() => ({}))
  if (!response.ok) throw new Error(`${init.method ?? 'GET'} ${url} returned ${response.status}: ${JSON.stringify(body)}`)
  return body
}

export async function run(env = process.env, fetchImpl = fetch) {
  const projectId = required(env, 'ARGUS_TEST_PROJECT_ID')
  const organizationId = required(env, 'ARGUS_TEST_ORG_ID')
  const userId = required(env, 'ARGUS_TEST_USER_ID')
  const token = required(env, 'ARGUS_CONTENT_SYNC_TOKEN')
  const baseUrl = env.ARGUS_TEST_CONTENT_URL ?? 'http://127.0.0.1:3000'
  if (![projectId, organizationId, userId].every((value) => UUID.test(value))) throw new Error('acceptance identity contains an invalid UUID')

  const headers = {
    authorization: `Bearer ${token}`,
    'content-type': 'application/json',
    'x-argus-org-id': organizationId,
    'x-argus-user-id': userId,
  }
  const internal = `${baseUrl}/internal/argus/cms/projects/${projectId}`
  const workspace = await json(fetchImpl, internal, { headers })
  if (workspace.project_status !== 'active') throw new Error('the Project was not synchronized to an active Payload project-space')

  const suffix = `${Date.now()}-${Math.floor(Math.random() * 1_000_000)}`
  const slug = `acceptance_${suffix.replaceAll('-', '_')}`
  const model = await json(fetchImpl, internal, {
    method: 'POST', headers,
    body: JSON.stringify({ operation: 'create_model', model: { name: `Acceptance ${suffix}`, slug, public_read: true,
      fields: [{ key: 'title', label: 'Title', type: 'text', required: true }, { key: 'body', label: 'Body', type: 'textarea', required: true }] } }),
  })
  const modelId = model.model?.id
  if (!UUID.test(modelId ?? '') || model.model.public_read !== true) throw new Error('content model response is invalid')

  const draft = await json(fetchImpl, internal, {
    method: 'POST', headers,
    body: JSON.stringify({ operation: 'save_record', model_id: modelId, values: { title: 'Acceptance draft', body: 'Not public' }, publish: false }),
  })
  const recordId = draft.record?.id
  if (!UUID.test(recordId ?? '') || draft.record.editorial_status !== 'draft') throw new Error('draft response is invalid')
  const publicUrl = `${baseUrl}/public/projects/${projectId}/content/${slug}`
  const beforePublish = await json(fetchImpl, publicUrl)
  if (!Array.isArray(beforePublish.records) || beforePublish.records.length !== 0) throw new Error('draft record leaked through public content read')

  const published = await json(fetchImpl, internal, {
    method: 'POST', headers,
    body: JSON.stringify({ operation: 'save_record', model_id: modelId, record_id: recordId,
      values: { title: 'Acceptance published', body: 'Public content' }, publish: true }),
  })
  if (published.record?.editorial_status !== 'published') throw new Error('record did not publish')
  const publicRead = await json(fetchImpl, publicUrl)
  if (publicRead.records?.length !== 1 || publicRead.records[0]?.id !== recordId ||
      publicRead.records[0]?.values?.title !== 'Acceptance published') throw new Error('published public read is inconsistent')

  const dataInternal = `${baseUrl}/internal/argus/data/projects/${projectId}`
  const dataWorkspace = await json(fetchImpl, dataInternal, { headers })
  if (dataWorkspace.project_status !== 'active') throw new Error('App Data Project scope is unavailable')
  const authorModel = await json(fetchImpl, dataInternal, { method: 'POST', headers,
    body: JSON.stringify({ operation: 'create_model', model: { name: `Authors ${suffix}`, slug: `authors_${suffix.replaceAll('-', '_')}`,
      fields: [{ key: 'name', label: 'Name', type: 'text', required: true }] } }) })
  const authorModelId = authorModel.model?.id
  if (!UUID.test(authorModelId ?? '') || authorModel.model.kind !== 'data') throw new Error('App Data author model response is invalid')
  const author = await json(fetchImpl, dataInternal, { method: 'POST', headers,
    body: JSON.stringify({ operation: 'save_record', model_id: authorModelId, values: { name: 'Ada' }, publish: false }) })
  const authorId = author.record?.id
  if (!UUID.test(authorId ?? '') || author.record.editorial_status !== 'published') throw new Error('App Data record was not written immediately')
  const taskModel = await json(fetchImpl, dataInternal, { method: 'POST', headers,
    body: JSON.stringify({ operation: 'create_model', model: { name: `Tasks ${suffix}`, slug: `tasks_${suffix.replaceAll('-', '_')}`,
      fields: [{ key: 'title', label: 'Title', type: 'text', required: true },
        { key: 'author', label: 'Author', type: 'relationship', required: true, target_model_id: authorModelId, has_many: false }] } }) })
  const taskModelId = taskModel.model?.id
  if (!UUID.test(taskModelId ?? '') || taskModel.model.kind !== 'data') throw new Error('App Data relationship model response is invalid')
  const task = await json(fetchImpl, dataInternal, { method: 'POST', headers,
    body: JSON.stringify({ operation: 'save_record', model_id: taskModelId, values: { title: 'Ship Argus' }, relationships: { author: [authorId] } }) })
  const taskId = task.record?.id
  if (!UUID.test(taskId ?? '') || task.record.editorial_status !== 'published') throw new Error('related App Data record is invalid')
  const storedData = await json(fetchImpl, dataInternal, { headers })
  if (!storedData.relations?.some((relation) => relation.source_record_id === taskId && relation.target_record_id === authorId && relation.field_key === 'author')) {
    throw new Error('App Data relationship was not persisted')
  }

  return { model_id: modelId, record_id: recordId, model_slug: slug,
    data_model_id: taskModelId, data_record_id: taskId, data_relation_target_id: authorId }
}

if (process.argv[1] === '-' || (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href)) {
  run().then((result) => process.stdout.write(`${JSON.stringify(result)}\n`)).catch((error) => {
    process.stderr.write(`[argus-content-acceptance] error: ${error.message}\n`)
    process.exitCode = 1
  })
}
