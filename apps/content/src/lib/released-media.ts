import { sql, type PostgresAdapter } from '@payloadcms/db-postgres'
import { APIError, type CollectionBeforeOperationHook } from 'payload'

export class ReleasedMediaError extends APIError {
  constructor() { super('This media is retained by a website release. Upload a new asset instead of replacing or deleting it.', 409) }
}

/** Retain files referenced by releases, including failed builds that can be retried. */
export const retainReleasedMedia: CollectionBeforeOperationHook = async ({ args, operation, req }) => {
  if (operation !== 'update' && operation !== 'delete') return args
  const input = args as typeof args & { id?: string; where?: Record<string, unknown> }
  const transactionID = await req.transactionID
  if (!transactionID) throw new APIError('Media changes require a transaction.', 503)
  const transaction = (req.payload.db as unknown as PostgresAdapter).sessions[transactionID]?.db
  if (!transaction) throw new APIError('Media changes require a transaction.', 503)
  const assets = await req.payload.find({ collection: 'media', req, depth: 0, pagination: false,
    overrideAccess: args.overrideAccess,
    where: input.id ? { id: { equals: input.id } } : input.where })
  const projects = [...new Set(assets.docs.map(asset => typeof asset.project === 'string' ? asset.project : asset.project.id))].sort()
  for (const project of projects) {
    // Same lock as publication; retained until this mutation commits or rolls back.
    await transaction.execute(sql`SELECT pg_advisory_xact_lock(hashtextextended(${`argus-site:${project}`}, 0))`)
    let page = 1
    while (true) {
      const releases = await req.payload.find({ collection: 'content-releases', req, depth: 0, overrideAccess: true,
        limit: 100, page, where: { project: { equals: project } } })
      if (releases.docs.some(release => {
        const snapshot = release.snapshot as { media?: Record<string, unknown> } | null
        return assets.docs.some(asset => Object.hasOwn(snapshot?.media ?? {}, asset.id))
      })) throw new ReleasedMediaError()
      if (!releases.hasNextPage) break
      page++
    }
  }
  return args
}
