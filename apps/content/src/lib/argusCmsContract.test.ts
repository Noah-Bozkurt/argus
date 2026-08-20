import assert from 'node:assert/strict'
import test from 'node:test'

import { internalIdentity, normalizeModelInput, validateValues } from './argusCmsContract.ts'

test('requires the internal token and valid Argus identity headers', () => {
  process.env.ARGUS_CONTENT_SYNC_TOKEN = '0123456789abcdef0123456789abcdef'
  const request = new Request('http://content.test/internal', { headers: {
    authorization: 'Bearer 0123456789abcdef0123456789abcdef',
    'x-argus-org-id': '00000000-0000-4000-8000-000000000001',
    'x-argus-user-id': '00000000-0000-4000-8000-000000000002',
  } })
  assert.deepEqual(internalIdentity(request), {
    organizationId: '00000000-0000-4000-8000-000000000001',
    userId: '00000000-0000-4000-8000-000000000002',
  })
  assert.equal(internalIdentity(new Request('http://content.test/internal')), null)
})

test('normalizes a bounded content model and rejects duplicate fields', () => {
  const valid = normalizeModelInput({
    name: 'Articles',
    slug: 'Articles',
    public_read: true,
    fields: [
      { key: 'title', label: 'Title', type: 'text', required: true },
      { key: 'body', label: 'Body', type: 'textarea' },
    ],
  })
  assert.equal(valid?.slug, 'articles')
  assert.equal(valid?.publicRead, true)
  assert.equal(normalizeModelInput({ name: 'Bad', slug: 'bad', fields: [
    { key: 'title', label: 'One', type: 'text' },
    { key: 'title', label: 'Two', type: 'text' },
  ] }), null)
})

test('validates dynamic scalar values and required fields', () => {
  const fields = [
    { key: 'title', label: 'Title', type: 'text' as const, required: true },
    { key: 'rating', label: 'Rating', type: 'number' as const, required: false },
  ]
  assert.deepEqual(validateValues(fields, { title: 'Hello', rating: 4 }), { title: 'Hello', rating: 4 })
  assert.equal(validateValues(fields, { rating: 4 }), null)
  assert.equal(validateValues(fields, { title: 'Hello', unknown: true }), null)
  assert.equal(validateValues(fields, { title: 'Hello', rating: 'four' }), null)
})
