import { test } from 'node:test'
import assert from 'node:assert/strict'
import { loadRelease, loadPreview } from './index.mjs'

test('release is pinned once and rejects a different project', async () => {
  let calls = 0
  const fetch = async () => { calls++; return Response.json({ id: 'release-a', snapshot: { version: 1, projectId: 'one', models: {} } }) }
  const release = await loadRelease({ contentURL: 'https://content.example', projectId: 'one', fetch })
  assert.equal(release.id, 'release-a'); assert.equal(calls, 1)
  await assert.rejects(loadRelease({ contentURL: 'https://content.example', projectId: 'two', fetch }), /Invalid/)
})
test('failed release never silently falls back to local content', async () => {
  await assert.rejects(loadRelease({ contentURL: 'https://content.example', projectId: 'one', fetch: async () => new Response(null, { status: 503 }) }), /refusing a partial build/)
})
test('preview passes only scoped authorization and rejects denial', async () => {
  await assert.rejects(loadPreview({ contentURL: 'https://content.example', token: 'scoped-grant', fetch: async (_url, options) => {
    assert.equal(options.headers.authorization, 'Bearer scoped-grant')
    assert.equal(options.cache, 'no-store')
    return new Response(null, { status: 403 })
  } }), /expired or denied/)
})
