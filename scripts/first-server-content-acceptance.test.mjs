import assert from 'node:assert/strict'
import test from 'node:test'

import { run } from './first-server-content-acceptance.mjs'

const project = '00000000-0000-4000-8000-000000000003'
const organization = '00000000-0000-4000-8000-000000000001'
const user = '00000000-0000-4000-8000-000000000002'
const model = '00000000-0000-4000-8000-000000000010'
const record = '00000000-0000-4000-8000-000000000011'
const authorModel = '00000000-0000-4000-8000-000000000012'
const authorRecord = '00000000-0000-4000-8000-000000000013'
const taskModel = '00000000-0000-4000-8000-000000000014'
const taskRecord = '00000000-0000-4000-8000-000000000015'

test('accepts a synchronized personal Project and proves draft/public publication semantics', async () => {
  const requests = []
  let publicReads = 0
  let dataReads = 0
  const fetchImpl = async (url, init = {}) => {
    requests.push({ url, init })
    const body = init.body ? JSON.parse(init.body) : null
    if (url.includes('/public/')) {
      publicReads += 1
      return Response.json({ records: publicReads === 1 ? [] : [{ id: record, values: { title: 'Acceptance published' } }] })
    }
    if (url.includes('/internal/argus/data/')) {
      if (!body) {
        dataReads += 1
        return Response.json({ project_status: 'active', relations: dataReads === 1 ? [] : [
          { source_record_id: taskRecord, target_record_id: authorRecord, field_key: 'author' },
        ] })
      }
      if (body.operation === 'create_model') {
        const id = body.model.fields.some((field) => field.type === 'relationship') ? taskModel : authorModel
        return Response.json({ model: { id, kind: 'data' } }, { status: 201 })
      }
      const id = body.model_id === taskModel ? taskRecord : authorRecord
      return Response.json({ record: { id, editorial_status: 'published' } }, { status: 201 })
    }
    if (!body) return Response.json({ project_status: 'active', models: [], records: [], relations: [] })
    if (body.operation === 'create_model') return Response.json({ model: { id: model, public_read: true } }, { status: 201 })
    if (!body.record_id) return Response.json({ record: { id: record, editorial_status: 'draft' } }, { status: 201 })
    return Response.json({ record: { id: record, editorial_status: 'published' } })
  }

  const result = await run({ ARGUS_TEST_PROJECT_ID: project, ARGUS_TEST_ORG_ID: organization,
    ARGUS_TEST_USER_ID: user, ARGUS_CONTENT_SYNC_TOKEN: 'x'.repeat(32) }, fetchImpl)
  assert.equal(result.model_id, model)
  assert.equal(result.record_id, record)
  assert.equal(result.data_model_id, taskModel)
  assert.equal(result.data_record_id, taskRecord)
  assert.equal(requests.length, 12)
  assert.equal(JSON.parse(requests[2].init.body).publish, false)
  assert.equal(JSON.parse(requests[4].init.body).publish, true)
  assert.equal(requests[0].init.headers['x-argus-org-id'], organization)
})

test('fails closed when the Project has not synchronized', async () => {
  await assert.rejects(() => run({ ARGUS_TEST_PROJECT_ID: project, ARGUS_TEST_ORG_ID: organization,
    ARGUS_TEST_USER_ID: user, ARGUS_CONTENT_SYNC_TOKEN: 'x'.repeat(32) }, async () => new Response('{}', { status: 404 })), /returned 404/)
})
