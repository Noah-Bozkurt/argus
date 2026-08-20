import assert from 'node:assert/strict'
import test from 'node:test'

import { run } from './first-server-content-acceptance.mjs'

const project = '00000000-0000-4000-8000-000000000003'
const organization = '00000000-0000-4000-8000-000000000001'
const user = '00000000-0000-4000-8000-000000000002'
const model = '00000000-0000-4000-8000-000000000010'
const record = '00000000-0000-4000-8000-000000000011'

test('accepts a synchronized personal Project and proves draft/public publication semantics', async () => {
  const requests = []
  let publicReads = 0
  const fetchImpl = async (url, init = {}) => {
    requests.push({ url, init })
    const body = init.body ? JSON.parse(init.body) : null
    if (url.includes('/public/')) {
      publicReads += 1
      return Response.json({ records: publicReads === 1 ? [] : [{ id: record, values: { title: 'Acceptance published' } }] })
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
  assert.equal(requests.length, 6)
  assert.equal(JSON.parse(requests[2].init.body).publish, false)
  assert.equal(JSON.parse(requests[4].init.body).publish, true)
  assert.equal(requests[0].init.headers['x-argus-org-id'], organization)
})

test('fails closed when the Project has not synchronized', async () => {
  await assert.rejects(() => run({ ARGUS_TEST_PROJECT_ID: project, ARGUS_TEST_ORG_ID: organization,
    ARGUS_TEST_USER_ID: user, ARGUS_CONTENT_SYNC_TOKEN: 'x'.repeat(32) }, async () => new Response('{}', { status: 404 })), /returned 404/)
})
