import assert from 'node:assert/strict'
import test from 'node:test'

import { csvCell, formSubmissionsCsv, normalizeFormInput, readBoundedJson, submissionSourceHash, validateSubmission } from './argusFormsContract.ts'

const form = normalizeFormInput({
  name: 'Contact', slug: 'Contact', success_message: 'Thanks', published: true,
  fields: [
    { key: 'email', label: 'Email', type: 'email', required: true },
    { key: 'topic', label: 'Topic', type: 'select', options: ['Support', 'Sales'] },
    { key: 'message', label: 'Message', type: 'textarea' },
  ],
})

test('normalizes bounded public form schemas', () => {
  assert.equal(form?.slug, 'contact')
  assert.deepEqual(form?.fields[1].options, ['Support', 'Sales'])
  assert.equal(normalizeFormInput({ name: 'Bad', slug: 'bad', success_message: 'ok', fields: [
    { key: 'topic', label: 'Topic', type: 'select', options: [] },
  ] }), null)
})

test('validates submissions without retaining unknown fields', () => {
  assert.deepEqual(validateSubmission(form!.fields, { email: 'person@example.com', topic: 'Support', message: 'Hello' }), {
    email: 'person@example.com', topic: 'Support', message: 'Hello',
  })
  assert.equal(validateSubmission(form!.fields, { email: 'bad', topic: 'Support' }), null)
  assert.equal(validateSubmission(form!.fields, { email: 'person@example.com', topic: 'Other' }), null)
  assert.equal(validateSubmission(form!.fields, { email: 'person@example.com', extra: 'nope' }), null)
})

test('source hashes are scoped, stable and do not retain source addresses', () => {
  const first = submissionSourceHash('secret', 'form-a', '203.0.113.10, 10.0.0.1', null)
  assert.equal(first, submissionSourceHash('secret', 'form-a', '203.0.113.10', null))
  assert.notEqual(first, submissionSourceHash('secret', 'form-b', '203.0.113.10', null))
  assert.equal(first.includes('203.0.113.10'), false)
})

test('bounded JSON reader accepts normal bodies and rejects oversized declarations', async () => {
  assert.deepEqual(await readBoundedJson(new Request('http://test', { method: 'POST', body: '{"ok":true}' })), { ok: true })
  assert.equal(await readBoundedJson(new Request('http://test', { method: 'POST', headers: { 'content-length': '70000' }, body: '{}' })), null)
})

test('CSV export is stable and neutralizes spreadsheet formulas', () => {
  assert.equal(csvCell('  =cmd()'), '"\'  =cmd()"')
  assert.equal(csvCell('say "hello"'), '"say ""hello"""')
  const csv = formSubmissionsCsv(form!.fields, [{ id: 'one', status: 'new', submittedAt: '2026-08-20T00:00:00Z', values: { email: '+malicious', topic: 'Support' } }])
  assert.match(csv, /^\uFEFF"submission_id","status","submitted_at","email","topic","message"\r\n/)
  assert.match(csv, /"'\+malicious"/)
})
