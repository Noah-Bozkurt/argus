import { MigrateUpArgs, MigrateDownArgs, sql } from '@payloadcms/db-postgres'

export async function up({ db, payload, req }: MigrateUpArgs): Promise<void> {
  await db.execute(sql`
   CREATE TYPE "argus_content"."enum_data_records_lifecycle_status" AS ENUM('active', 'archived');
  CREATE TYPE "argus_content"."enum__data_records_v_version_status" AS ENUM('draft', 'published');
  CREATE TABLE "argus_content"."_data_records_v" (
  	"id" uuid PRIMARY KEY DEFAULT gen_random_uuid() NOT NULL,
  	"parent_id" uuid,
  	"version_organization_id" varchar,
  	"version_argus_project_id" varchar,
  	"version_project_id" uuid,
  	"version_model_id" uuid,
  	"version_schema_version" numeric,
  	"version_values" jsonb DEFAULT '{}'::jsonb,
  	"version_status" "argus_content"."enum_data_records_lifecycle_status" DEFAULT 'active',
  	"version_published_at" timestamp(3) with time zone,
  	"version_created_by_id" uuid,
  	"version_updated_at" timestamp(3) with time zone,
  	"version_created_at" timestamp(3) with time zone,
  	"version__status" "argus_content"."enum__data_records_v_version_status" DEFAULT 'draft',
  	"created_at" timestamp(3) with time zone DEFAULT now() NOT NULL,
  	"updated_at" timestamp(3) with time zone DEFAULT now() NOT NULL,
  	"latest" boolean
  );
  
  ALTER TABLE "argus_content"."data_records" ALTER COLUMN "_status" SET DATA TYPE text;
  ALTER TABLE "argus_content"."data_records" ALTER COLUMN "_status" SET DEFAULT 'draft'::text;
  DROP TYPE "argus_content"."enum_data_records_status";
  CREATE TYPE "argus_content"."enum_data_records_status" AS ENUM('draft', 'published');
  ALTER TABLE "argus_content"."data_records" ALTER COLUMN "_status" SET DEFAULT 'draft'::"argus_content"."enum_data_records_status";
  ALTER TABLE "argus_content"."data_records" ALTER COLUMN "_status" SET DATA TYPE "argus_content"."enum_data_records_status" USING "_status"::"argus_content"."enum_data_records_status";
  ALTER TABLE "argus_content"."data_records" ALTER COLUMN "organization_id" DROP NOT NULL;
  ALTER TABLE "argus_content"."data_records" ALTER COLUMN "argus_project_id" DROP NOT NULL;
  ALTER TABLE "argus_content"."data_records" ALTER COLUMN "project_id" DROP NOT NULL;
  ALTER TABLE "argus_content"."data_records" ALTER COLUMN "model_id" DROP NOT NULL;
  ALTER TABLE "argus_content"."data_records" ALTER COLUMN "schema_version" DROP NOT NULL;
  ALTER TABLE "argus_content"."data_records" ALTER COLUMN "values" DROP NOT NULL;
  ALTER TABLE "argus_content"."data_records" ALTER COLUMN "status" DROP DEFAULT;
  ALTER TABLE "argus_content"."data_records" ALTER COLUMN "status" SET DATA TYPE "argus_content"."enum_data_records_lifecycle_status" USING "status"::text::"argus_content"."enum_data_records_lifecycle_status";
  ALTER TABLE "argus_content"."data_records" ALTER COLUMN "status" SET DEFAULT 'active';
  ALTER TABLE "argus_content"."data_records" ALTER COLUMN "status" DROP NOT NULL;
  ALTER TABLE "argus_content"."data_models" ADD COLUMN "public_read" boolean DEFAULT false;
  ALTER TABLE "argus_content"."data_records" ADD COLUMN "published_at" timestamp(3) with time zone;
  ALTER TABLE "argus_content"."data_records" ADD COLUMN "_status" "argus_content"."enum_data_records_status" DEFAULT 'draft';
  ALTER TABLE "argus_content"."_data_records_v" ADD CONSTRAINT "_data_records_v_parent_id_data_records_id_fk" FOREIGN KEY ("parent_id") REFERENCES "argus_content"."data_records"("id") ON DELETE set null ON UPDATE no action;
  ALTER TABLE "argus_content"."_data_records_v" ADD CONSTRAINT "_data_records_v_version_project_id_project_spaces_id_fk" FOREIGN KEY ("version_project_id") REFERENCES "argus_content"."project_spaces"("id") ON DELETE set null ON UPDATE no action;
  ALTER TABLE "argus_content"."_data_records_v" ADD CONSTRAINT "_data_records_v_version_model_id_data_models_id_fk" FOREIGN KEY ("version_model_id") REFERENCES "argus_content"."data_models"("id") ON DELETE set null ON UPDATE no action;
  ALTER TABLE "argus_content"."_data_records_v" ADD CONSTRAINT "_data_records_v_version_created_by_id_workspace_users_id_fk" FOREIGN KEY ("version_created_by_id") REFERENCES "argus_content"."workspace_users"("id") ON DELETE set null ON UPDATE no action;
  CREATE INDEX "_data_records_v_parent_idx" ON "argus_content"."_data_records_v" USING btree ("parent_id");
  CREATE INDEX "_data_records_v_version_version_organization_id_idx" ON "argus_content"."_data_records_v" USING btree ("version_organization_id");
  CREATE INDEX "_data_records_v_version_version_argus_project_id_idx" ON "argus_content"."_data_records_v" USING btree ("version_argus_project_id");
  CREATE INDEX "_data_records_v_version_version_project_idx" ON "argus_content"."_data_records_v" USING btree ("version_project_id");
  CREATE INDEX "_data_records_v_version_version_model_idx" ON "argus_content"."_data_records_v" USING btree ("version_model_id");
  CREATE INDEX "_data_records_v_version_version_created_by_idx" ON "argus_content"."_data_records_v" USING btree ("version_created_by_id");
  CREATE INDEX "_data_records_v_version_version_updated_at_idx" ON "argus_content"."_data_records_v" USING btree ("version_updated_at");
  CREATE INDEX "_data_records_v_version_version_created_at_idx" ON "argus_content"."_data_records_v" USING btree ("version_created_at");
  CREATE INDEX "_data_records_v_version_version__status_idx" ON "argus_content"."_data_records_v" USING btree ("version__status");
  CREATE INDEX "_data_records_v_created_at_idx" ON "argus_content"."_data_records_v" USING btree ("created_at");
  CREATE INDEX "_data_records_v_updated_at_idx" ON "argus_content"."_data_records_v" USING btree ("updated_at");
  CREATE INDEX "_data_records_v_latest_idx" ON "argus_content"."_data_records_v" USING btree ("latest");
  CREATE INDEX "data_records__status_idx" ON "argus_content"."data_records" USING btree ("_status");`)
}

export async function down({ db, payload, req }: MigrateDownArgs): Promise<void> {
  await db.execute(sql`
   ALTER TABLE "argus_content"."_data_records_v" DISABLE ROW LEVEL SECURITY;
  DROP TABLE "argus_content"."_data_records_v" CASCADE;
  ALTER TABLE "argus_content"."data_records" ALTER COLUMN "status" SET DATA TYPE text;
  ALTER TABLE "argus_content"."data_records" ALTER COLUMN "status" SET DEFAULT 'active'::text;
  DROP TYPE "argus_content"."enum_data_records_status";
  CREATE TYPE "argus_content"."enum_data_records_status" AS ENUM('active', 'archived');
  ALTER TABLE "argus_content"."data_records" ALTER COLUMN "status" SET DEFAULT 'active'::"argus_content"."enum_data_records_status";
  ALTER TABLE "argus_content"."data_records" ALTER COLUMN "status" SET DATA TYPE "argus_content"."enum_data_records_status" USING "status"::"argus_content"."enum_data_records_status";
  DROP INDEX "argus_content"."data_records__status_idx";
  ALTER TABLE "argus_content"."data_records" ALTER COLUMN "organization_id" SET NOT NULL;
  ALTER TABLE "argus_content"."data_records" ALTER COLUMN "argus_project_id" SET NOT NULL;
  ALTER TABLE "argus_content"."data_records" ALTER COLUMN "project_id" SET NOT NULL;
  ALTER TABLE "argus_content"."data_records" ALTER COLUMN "model_id" SET NOT NULL;
  ALTER TABLE "argus_content"."data_records" ALTER COLUMN "schema_version" SET NOT NULL;
  ALTER TABLE "argus_content"."data_records" ALTER COLUMN "values" SET NOT NULL;
  ALTER TABLE "argus_content"."data_records" ALTER COLUMN "status" DROP DEFAULT;
  ALTER TABLE "argus_content"."data_records" ALTER COLUMN "status" SET DATA TYPE "argus_content"."enum_data_records_status" USING "status"::text::"argus_content"."enum_data_records_status";
  ALTER TABLE "argus_content"."data_records" ALTER COLUMN "status" SET DEFAULT 'active';
  ALTER TABLE "argus_content"."data_records" ALTER COLUMN "status" SET NOT NULL;
  ALTER TABLE "argus_content"."data_models" DROP COLUMN "public_read";
  ALTER TABLE "argus_content"."data_records" DROP COLUMN "published_at";
  ALTER TABLE "argus_content"."data_records" DROP COLUMN "_status";
  DROP TYPE "argus_content"."enum_data_records_lifecycle_status";
  DROP TYPE "argus_content"."enum__data_records_v_version_status";`)
}
