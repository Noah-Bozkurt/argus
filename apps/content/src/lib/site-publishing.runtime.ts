import assert from 'node:assert/strict'
import { randomUUID, randomBytes } from 'node:crypto'
import { getPayload } from 'payload'
import config from '../payload.config'
import { createRelease } from './content-release'
import { processSite } from './site-publishing'
import { seal } from './site-security'
if (process.env.ARGUS_TEST_DATABASE !== '1') throw new Error('Use an isolated test database and set ARGUS_TEST_DATABASE=1')
const payload = await getPayload({ config })
const organizationId = randomUUID()
const projectId = randomUUID()
const project = await payload.create({ collection: 'project-spaces', overrideAccess: true, data: { argusProjectId: projectId, organizationId, name: 'Publication runtime test', status: 'active' } })
const other = await payload.create({ collection: 'project-spaces', overrideAccess: true, data: { argusProjectId: randomUUID(), organizationId: randomUUID(), name: 'Isolated project', status: 'active' } })
const model = await payload.create({ collection: 'data-models', overrideAccess: true, data: { project: project.id, organizationId, argusProjectId: projectId, name: 'Articles', slug: 'articles', kind: 'content', contentRole: 'collection', schemaVersion: 1, status: 'active', publicRead: true, fields: [{ key: 'title', label: 'Title', type: 'text', required: true }] } })
const draft = await payload.create({ collection: 'data-records', draft: true, overrideAccess: true, data: { project: project.id, model: model.id, organizationId, argusProjectId: projectId, schemaVersion: 1, values: { title: 'First release' }, _status: 'draft' } })
const image = Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aB1UAAAAASUVORK5CYII=', 'base64')
const asset = await payload.create({ collection: 'media', overrideAccess: true, file: { data: image, mimetype: 'image/png', name: `${randomUUID()}.png`, size: image.length }, data: { project: project.id, organizationId, argusProjectId: projectId, alt: 'Release asset', publicRead: true } })
const requestId = randomUUID()
const release = await createRelease(payload, project.id, projectId, requestId, [draft.id])
assert.equal(release.status, 'queued')
await assert.rejects(payload.update({ collection: 'media', id: asset.id, overrideAccess: true, data: { publicRead: false } }), /retained by a website release/)
await assert.rejects(payload.delete({ collection: 'media', id: asset.id, overrideAccess: true }), /retained by a website release/)
const snapshot = release.snapshot as { models: { articles: { records: Array<{ values: { title: string } }> } } }
assert.equal(snapshot.models.articles.records[0].values.title, 'First release')
assert.equal((await createRelease(payload, project.id, projectId, requestId, [draft.id])).id, release.id)
await payload.update({ collection: 'data-records', id: draft.id, draft: true, overrideAccess: true, data: { values: { title: 'Unpublished next edit' }, _status: 'draft' } })
const saved = await payload.findByID({ collection: 'content-releases', id: release.id, overrideAccess: true })
assert.deepEqual(saved.snapshot, release.snapshot)
const nextRelease = await createRelease(payload, project.id, projectId, randomUUID(), [])
assert.equal((nextRelease.snapshot as typeof snapshot).models.articles.records[0].values.title, 'First release')
await assert.rejects(createRelease(payload, other.id, other.argusProjectId, randomUUID(), [draft.id]), /PERMISSION_DENIED/)
const invalidRequest = randomUUID()
await assert.rejects(createRelease(payload, project.id, projectId, invalidRequest, [draft.id, randomUUID()]))
const rollback = await payload.find({ collection: 'content-releases', overrideAccess: true, where: { requestId: { equals: invalidRequest } } })
assert.equal(rollback.docs.length, 0)
const stillDraft = await payload.findByID({ collection: 'data-records', id: draft.id, draft: true, overrideAccess: true })
assert.equal(stillDraft._status, 'draft')
// A provider timeout must not cause automatic duplicate build triggers.
await payload.create({ collection: 'site-connections', overrideAccess: true, data: { project: project.id, siteURL: 'https://site.example', provider: 'pages', accountId: 'a'.repeat(32), target: 'test', branch: 'main', credential: seal(randomBytes(32).toString('hex'), projectId), hook: seal('https://api.cloudflare.com/client/v4/pages/webhooks/deploy_hooks/test', projectId) } })
const originalFetch = globalThis.fetch
let triggers = 0
globalThis.fetch = async () => { triggers++; throw new Error('Simulated uncertain response') }
try {
 await processSite(payload, project.id, projectId)
 assert.equal(triggers, 1)
 const unknown = await payload.findByID({ collection: 'content-releases', id: release.id, overrideAccess: true })
 assert.equal(unknown.status, 'unknown')
 await processSite(payload, project.id, projectId)
 assert.equal(triggers, 1)
} finally { globalThis.fetch = originalFetch }
console.log('Publication runtime checks passed: snapshot, idempotency, isolation, transaction rollback, uncertain trigger handling.')
await payload.destroy()
