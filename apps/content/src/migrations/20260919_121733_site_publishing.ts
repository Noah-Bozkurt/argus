import { MigrateUpArgs, MigrateDownArgs, sql } from '@payloadcms/db-postgres'

export async function up({ db, payload, req }: MigrateUpArgs): Promise<void> {
  await db.execute(sql`
   CREATE TYPE "argus_content"."enum_site_connections_provider" AS ENUM('pages', 'workers');
  CREATE TYPE "argus_content"."enum_content_releases_status" AS ENUM('queued', 'triggering', 'building', 'deployed', 'failed', 'unknown');
  CREATE TABLE "argus_content"."site_connections" (
  	"id" uuid PRIMARY KEY DEFAULT gen_random_uuid() NOT NULL,
  	"project_id" uuid NOT NULL,
  	"site_u_r_l" varchar NOT NULL,
  	"preview_u_r_l" varchar,
  	"provider" "argus_content"."enum_site_connections_provider" NOT NULL,
  	"account_id" varchar NOT NULL,
  	"target" varchar NOT NULL,
  	"branch" varchar NOT NULL,
  	"credential" varchar NOT NULL,
  	"hook" varchar,
  	"components" jsonb DEFAULT '[]'::jsonb,
  	"updated_at" timestamp(3) with time zone DEFAULT now() NOT NULL,
  	"created_at" timestamp(3) with time zone DEFAULT now() NOT NULL
  );
  
  CREATE TABLE "argus_content"."content_releases" (
  	"id" uuid PRIMARY KEY DEFAULT gen_random_uuid() NOT NULL,
  	"project_id" uuid NOT NULL,
  	"request_id" varchar NOT NULL,
  	"snapshot" jsonb NOT NULL,
  	"status" "argus_content"."enum_content_releases_status" DEFAULT 'queued' NOT NULL,
  	"provider_id" varchar,
  	"error_code" varchar,
  	"started_at" timestamp(3) with time zone,
  	"completed_at" timestamp(3) with time zone,
  	"updated_at" timestamp(3) with time zone DEFAULT now() NOT NULL,
  	"created_at" timestamp(3) with time zone DEFAULT now() NOT NULL
  );
  
  ALTER TABLE "argus_content"."payload_locked_documents_rels" ADD COLUMN "site_connections_id" uuid;
  ALTER TABLE "argus_content"."payload_locked_documents_rels" ADD COLUMN "content_releases_id" uuid;
  ALTER TABLE "argus_content"."site_connections" ADD CONSTRAINT "site_connections_project_id_project_spaces_id_fk" FOREIGN KEY ("project_id") REFERENCES "argus_content"."project_spaces"("id") ON DELETE set null ON UPDATE no action;
  ALTER TABLE "argus_content"."content_releases" ADD CONSTRAINT "content_releases_project_id_project_spaces_id_fk" FOREIGN KEY ("project_id") REFERENCES "argus_content"."project_spaces"("id") ON DELETE set null ON UPDATE no action;
  CREATE UNIQUE INDEX "site_connections_project_idx" ON "argus_content"."site_connections" USING btree ("project_id");
  CREATE INDEX "site_connections_updated_at_idx" ON "argus_content"."site_connections" USING btree ("updated_at");
  CREATE INDEX "site_connections_created_at_idx" ON "argus_content"."site_connections" USING btree ("created_at");
  CREATE INDEX "content_releases_project_idx" ON "argus_content"."content_releases" USING btree ("project_id");
  CREATE UNIQUE INDEX "content_releases_request_id_idx" ON "argus_content"."content_releases" USING btree ("request_id");
  CREATE INDEX "content_releases_updated_at_idx" ON "argus_content"."content_releases" USING btree ("updated_at");
  CREATE INDEX "content_releases_created_at_idx" ON "argus_content"."content_releases" USING btree ("created_at");
  ALTER TABLE "argus_content"."payload_locked_documents_rels" ADD CONSTRAINT "payload_locked_documents_rels_site_connections_fk" FOREIGN KEY ("site_connections_id") REFERENCES "argus_content"."site_connections"("id") ON DELETE cascade ON UPDATE no action;
  ALTER TABLE "argus_content"."payload_locked_documents_rels" ADD CONSTRAINT "payload_locked_documents_rels_content_releases_fk" FOREIGN KEY ("content_releases_id") REFERENCES "argus_content"."content_releases"("id") ON DELETE cascade ON UPDATE no action;
  CREATE INDEX "payload_locked_documents_rels_site_connections_id_idx" ON "argus_content"."payload_locked_documents_rels" USING btree ("site_connections_id");
  CREATE INDEX "payload_locked_documents_rels_content_releases_id_idx" ON "argus_content"."payload_locked_documents_rels" USING btree ("content_releases_id");`)
}

export async function down({ db, payload, req }: MigrateDownArgs): Promise<void> {
  await db.execute(sql`
   ALTER TABLE "argus_content"."site_connections" DISABLE ROW LEVEL SECURITY;
  ALTER TABLE "argus_content"."content_releases" DISABLE ROW LEVEL SECURITY;
  DROP TABLE "argus_content"."site_connections" CASCADE;
  DROP TABLE "argus_content"."content_releases" CASCADE;
  ALTER TABLE "argus_content"."payload_locked_documents_rels" DROP CONSTRAINT IF EXISTS "payload_locked_documents_rels_site_connections_fk";
  
  ALTER TABLE "argus_content"."payload_locked_documents_rels" DROP CONSTRAINT IF EXISTS "payload_locked_documents_rels_content_releases_fk";
  
  DROP INDEX "argus_content"."payload_locked_documents_rels_site_connections_id_idx";
  DROP INDEX "argus_content"."payload_locked_documents_rels_content_releases_id_idx";
  ALTER TABLE "argus_content"."payload_locked_documents_rels" DROP COLUMN "site_connections_id";
  ALTER TABLE "argus_content"."payload_locked_documents_rels" DROP COLUMN "content_releases_id";
  DROP TYPE "argus_content"."enum_site_connections_provider";
  DROP TYPE "argus_content"."enum_content_releases_status";`)
}
