import type { Payload, PayloadRequest, Where } from 'payload'
import { isUUID } from './projectScope'

export type ReleaseRecord = { id: string; model: string; values: Record<string, unknown>; layout: unknown; published_at: string | null; updated_at: string }
export type ReleaseSnapshot = { version: 1; projectId: string; models: Record<string, { records: ReleaseRecord[] }>; media: Record<string, unknown> }

export async function withSiteLock<T>(payload: Payload, project: string, run: () => Promise<T>): Promise<T> {
  const client = await payload.db.pool.connect()
  try {
    await client.query('SELECT pg_advisory_lock(hashtextextended($1, 0))', [`argus-site:${project}`])
    return await run()
  } finally {
    try { await client.query('SELECT pg_advisory_unlock(hashtextextended($1, 0))', [`argus-site:${project}`]) } finally { client.release() }
  }
}
export async function createRelease(payload: Payload, project: string, projectId: string, requestId: string, recordIds: string[]) {
  if (!isUUID(requestId) || recordIds.length > 100 || recordIds.some(id => !isUUID(id))) throw new Error('INVALID_REQUEST')
  return withSiteLock(payload, project, async () => {
    const existing = await payload.find({ collection: 'content-releases', depth: 0, overrideAccess: true, limit: 1, where: { and: [{ project: { equals: project } }, { requestId: { equals: requestId } }] } })
    if (existing.docs[0]) return existing.docs[0]
    const transactionID = await payload.db.beginTransaction({ isolationLevel: 'repeatable read' })
    if (transactionID === null) throw new Error('TRANSACTION_UNAVAILABLE')
    const req = { transactionID } as PayloadRequest
    try {
      const models = await payload.find({ collection: 'data-models', depth: 0, limit: 1000, overrideAccess: true, req,
        where: { and: [{ project: { equals: project } }, { kind: { equals: 'content' } }, { status: { equals: 'active' } }] } })
      if (models.hasNextPage) throw new Error('WORKSPACE_TOO_LARGE')
      for (const id of [...new Set(recordIds)]) {
        const record = await payload.findByID({ collection: 'data-records', id, depth: 0, draft: true, overrideAccess: true, req })
        if (record.project !== project || record.status !== 'active') throw new Error('PERMISSION_DENIED')
        const model = models.docs.find(model => model.id === record.model)
        if (!model?.publicRead) throw new Error('PUBLIC_CONTENT_REQUIRED')
        await payload.update({ collection: 'data-records', id, draft: false, overrideAccess: true, req,
          data: { values: record.values, layout: record.layout, _status: 'published' } })
      }
      const snapshot: ReleaseSnapshot = { version: 1, projectId, models: {}, media: {} }
      const records: ReleaseRecord[] = []
      const publicModels = models.docs.filter(model => model.publicRead && model.contentRole !== 'component')
      for (const model of publicModels) {
        const output: ReleaseRecord[] = []
        let page = 1
        while (true) {
          const found = await payload.find({ collection: 'data-records', depth: 0, limit: 100, page, draft: false, overrideAccess: true, req,
            where: { and: [{ project: { equals: project } }, { model: { equals: model.id } }, { status: { equals: 'active' } }, { _status: { equals: 'published' } }] } })
          for (const record of found.docs) output.push({ id: record.id, model: model.slug, values: (record.values ?? {}) as Record<string, unknown>, layout: record.layout ?? [], published_at: record.publishedAt ?? null, updated_at: record.updatedAt })
          if (!found.hasNextPage) break
          page++
        }
        snapshot.models[model.slug] = { records: output }
        records.push(...output)
      }
      // Pin public asset metadata. Private assets are never copied into public releases.
      let page = 1
      while (true) {
        const assets = await payload.find({ collection: 'media', depth: 0, page, limit: 100, overrideAccess: true, req,
          where: { and: [{ project: { equals: project } }, { publicRead: { equals: true } }] } })
        for (const asset of assets.docs) snapshot.media[asset.id] = { id: asset.id, url: asset.url, alt: asset.alt, caption: asset.caption, width: asset.width, height: asset.height, sizes: asset.sizes }
        if (!assets.hasNextPage) break
        page++
      }
      const relationIds = new Set(records.map(record => record.id))
      for (const record of records) {
        const model = models.docs.find(model => model.slug === record.model)!
        const resolveMedia = (values: Record<string, unknown>, fields: typeof model.fields) => {
          const result = { ...values }
          for (const field of fields ?? []) if (field.type === 'media') {
            const raw = result[field.key]
            result[field.key] = field.hasMany ? (Array.isArray(raw) ? raw : []).map(id => snapshot.media[String(id)]).filter(Boolean) : snapshot.media[String(raw)] ?? null
          }
          return result
        }
        record.values = resolveMedia(record.values, model.fields)
        if (Array.isArray(record.layout)) record.layout = record.layout.map(block => ({ ...block, values: resolveMedia(block.values, models.docs.find(component => component.slug === block.component)?.fields ?? []) }))
        const fields = (model.fields ?? []).filter(field => field.type === 'relationship')
        for (const field of fields) {
          const where: Where = { and: [{ project: { equals: project } }, { sourceRecord: { equals: record.id } }, { fieldKey: { equals: field.key } }] }
          const relations = await payload.find({ collection: 'data-relations', depth: 0, pagination: false, limit: 100, overrideAccess: true, req, where })
          const ids = relations.docs.map(edge => String(edge.targetRecord)).filter(id => relationIds.has(id))
          record.values[field.key] = field.hasMany ? ids : ids[0] ?? null
        }
      }
      if (Buffer.byteLength(JSON.stringify(snapshot)) > 20 * 1024 * 1024) throw new Error('WORKSPACE_TOO_LARGE')
      const release = await payload.create({ collection: 'content-releases', overrideAccess: true, req,
        data: { project, requestId, snapshot: JSON.parse(JSON.stringify(snapshot)), status: 'queued' } })
      await payload.db.commitTransaction(transactionID)
      return release
    } catch (error) {
      await payload.db.rollbackTransaction(transactionID)
      throw error
    }
  })
}
