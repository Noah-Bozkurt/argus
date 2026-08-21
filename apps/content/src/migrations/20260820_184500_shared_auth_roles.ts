import { MigrateDownArgs, MigrateUpArgs, sql } from '@payloadcms/db-postgres'

export async function up({ db }: MigrateUpArgs): Promise<void> {
  await db.execute(sql`
    ALTER TYPE "argus_content"."enum_workspace_users_role" RENAME TO "enum_workspace_users_role_old";
    CREATE TYPE "argus_content"."enum_workspace_users_role" AS ENUM('owner', 'admin', 'member', 'client');
    ALTER TABLE "argus_content"."workspace_users" ALTER COLUMN "role" DROP DEFAULT;
    ALTER TABLE "argus_content"."workspace_users"
      ALTER COLUMN "role" TYPE "argus_content"."enum_workspace_users_role"
      USING "role"::text::"argus_content"."enum_workspace_users_role";
    ALTER TABLE "argus_content"."workspace_users" ALTER COLUMN "role" SET DEFAULT 'member';
    DROP TYPE "argus_content"."enum_workspace_users_role_old";
  `)
}

export async function down({ db }: MigrateDownArgs): Promise<void> {
  await db.execute(sql`
    UPDATE "argus_content"."workspace_users" SET "role" = 'admin' WHERE "role" = 'owner';
    UPDATE "argus_content"."workspace_users" SET "role" = 'member' WHERE "role" = 'client';
    ALTER TYPE "argus_content"."enum_workspace_users_role" RENAME TO "enum_workspace_users_role_new";
    CREATE TYPE "argus_content"."enum_workspace_users_role" AS ENUM('admin', 'member');
    ALTER TABLE "argus_content"."workspace_users" ALTER COLUMN "role" DROP DEFAULT;
    ALTER TABLE "argus_content"."workspace_users"
      ALTER COLUMN "role" TYPE "argus_content"."enum_workspace_users_role"
      USING "role"::text::"argus_content"."enum_workspace_users_role";
    ALTER TABLE "argus_content"."workspace_users" ALTER COLUMN "role" SET DEFAULT 'member';
    DROP TYPE "argus_content"."enum_workspace_users_role_new";
  `)
}
