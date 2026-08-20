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
import { ProjectMemberships } from './collections/ProjectMemberships'
import { ProjectSpaces } from './collections/ProjectSpaces'
import { WorkspaceUsers } from './collections/WorkspaceUsers'
import { migrations } from './migrations'

const filename = fileURLToPath(import.meta.url)
const dirname = path.dirname(filename)
const databaseURL = process.env.DATABASE_URL ?? ''
const secret = process.env.PAYLOAD_SECRET ?? ''
const schemaName = process.env.ARGUS_CONTENT_DB_SCHEMA ?? 'argus_content'

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
  ],
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
  secret,
  serverURL: process.env.PAYLOAD_PUBLIC_URL,
  sharp,
  typescript: {
    outputFile: path.resolve(dirname, 'payload-types.ts'),
  },
})
