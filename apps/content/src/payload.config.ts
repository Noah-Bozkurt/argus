import { postgresAdapter } from '@payloadcms/db-postgres'
import { lexicalEditor } from '@payloadcms/richtext-lexical'
import path from 'path'
import { buildConfig } from 'payload'
import sharp from 'sharp'
import { fileURLToPath } from 'url'

import { DataModels } from './collections/DataModels'
import { DataRecords } from './collections/DataRecords'
import { DataRelations } from './collections/DataRelations'
import { Media } from './collections/Media'
import { FormDefinitions } from './collections/FormDefinitions'
import { FormSubmissions } from './collections/FormSubmissions'
import { ProjectMemberships } from './collections/ProjectMemberships'
import { ProjectSpaces } from './collections/ProjectSpaces'
import { WorkspaceUsers } from './collections/WorkspaceUsers'
import { migrations } from './migrations'

const filename = fileURLToPath(import.meta.url)
const dirname = path.dirname(filename)
const databaseURL = process.env.DATABASE_URL ?? ''
const secret = process.env.PAYLOAD_SECRET ?? ''
const schemaName = process.env.ARGUS_CONTENT_DB_SCHEMA ?? 'argus_content'
const payloadPublicURL = process.env.PAYLOAD_PUBLIC_URL ?? ''
const webPublicURL = process.env.ARGUS_WEB_PUBLIC_URL ?? ''
const trustedOrigins = [payloadPublicURL, webPublicURL].filter(Boolean)

if (!databaseURL) {
  throw new Error('DATABASE_URL is required for @argus/content')
}
if (secret.length < 32) {
  throw new Error('PAYLOAD_SECRET must be at least 32 characters')
}
if (!/^[a-z_][a-z0-9_]*$/.test(schemaName)) {
  throw new Error('ARGUS_CONTENT_DB_SCHEMA must be a safe PostgreSQL schema identifier')
}

export default buildConfig({
  admin: {
    user: WorkspaceUsers.slug,
    importMap: {
      baseDir: path.resolve(dirname),
    },
  },
  collections: [
    WorkspaceUsers,
    ProjectSpaces,
    ProjectMemberships,
    DataModels,
    DataRecords,
    DataRelations,
    Media,
    FormDefinitions,
    FormSubmissions,
  ],
  cors: trustedOrigins,
  csrf: trustedOrigins,
  db: postgresAdapter({
    idType: 'uuid',
    migrationDir: path.resolve(dirname, 'migrations'),
    pool: {
      connectionString: databaseURL,
    },
    prodMigrations: migrations,
    push: process.env.PAYLOAD_DB_PUSH === 'true',
    schemaName,
  }),
  editor: lexicalEditor(),
  onInit: async (payload) => {
    const existing = await payload.find({
      collection: 'workspace-users',
      depth: 0,
      limit: 1,
      overrideAccess: true,
      pagination: false,
    })
    if (existing.docs.length > 0) return

    const email = process.env.ARGUS_OPERATOR_EMAIL?.trim().toLowerCase()
    const password = process.env.ARGUS_OPERATOR_PASSWORD
    const organizationId = process.env.ARGUS_ORG_ID
    const argusUserId = process.env.ARGUS_USER_ID
    if (!email || !password || !organizationId || !argusUserId) {
      payload.logger.warn('Shared Argus auth bootstrap skipped because operator bootstrap environment is incomplete')
      return
    }

    await payload.create({
      collection: 'workspace-users',
      overrideAccess: true,
      data: {
        displayName: 'Argus Owner',
        email,
        password,
        organizationId,
        argusUserId,
        role: 'owner',
      } as any,
    })
    payload.logger.info(`Created initial Argus owner account for ${email}`)
  },
  secret,
  serverURL: payloadPublicURL,
  sharp,
  typescript: {
    outputFile: path.resolve(dirname, 'payload-types.ts'),
  },
})
