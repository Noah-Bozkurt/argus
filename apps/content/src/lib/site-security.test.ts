import { test } from 'node:test'
import assert from 'node:assert/strict'
import { randomBytes } from 'node:crypto'
import { seal, unseal, previewToken, verifyPreview, httpsURL } from './site-security.ts'
process.env.ARGUS_SITE_ENCRYPTION_KEY = randomBytes(32).toString('hex')
test('connection secrets are encrypted, randomized and bound to the project', () => {
  const secret = randomBytes(32).toString('hex')
  const first = seal(secret, 'a')
  assert.notEqual(first, seal(secret, 'a'))
  assert.equal(first.includes(secret), false)
  assert.equal(unseal(first, 'a'), secret)
  assert.throws(() => unseal(first, 'b'))
  assert.throws(() => unseal(first.slice(0, -4), 'a'))
})
test('preview grants bind record, identity and origin and reject modified signatures', () => {
  const grant = { project: 'project-a', record: 'record-a', user: 'user-a', origin: 'https://preview.example' }
  const token = previewToken(grant)
  assert.equal(verifyPreview(token)?.project, grant.project)
  assert.equal(verifyPreview(token)?.origin, grant.origin)
  assert.equal(verifyPreview(`${token}.extra`), null)
  const [body, signature] = token.split('.')
  const altered = Buffer.from(JSON.stringify({ ...grant, record: 'record-b', exp: Date.now() / 1000 + 300 })).toString('base64url')
  assert.equal(verifyPreview(`${altered}.${signature}`), null)
  assert.ok(body)
})
test('connection URLs reject credentials, unsafe schemes, numeric hosts and query secrets', () => {
  for (const url of ['http://site.example', 'https://localhost', 'https://127.0.0.1', 'https://user:password@site.example', 'https://site.example?token=example', 'file:///etc/passwd']) assert.throws(() => httpsURL(url))
  assert.equal(httpsURL('https://site.example/preview'), 'https://site.example/preview')
})
