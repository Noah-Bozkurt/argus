import { MigrateUpArgs, MigrateDownArgs, sql } from '@payloadcms/db-postgres'

export async function up({ db, payload, req }: MigrateUpArgs): Promise<void> {
  await db.execute(sql`
  CREATE TABLE "argus_content"."media" (
	"id" uuid PRIMARY KEY DEFAULT gen_random_uuid() NOT NULL,
	"organization_id" varchar NOT NULL,
	"argus_project_id" varchar NOT NULL,
	"project_id" uuid NOT NULL,
	"alt" varchar NOT NULL,
	"caption" varchar,
	"public_read" boolean DEFAULT false,
	"updated_at" timestamp(3) with time zone DEFAULT now() NOT NULL,
	"created_at" timestamp(3) with time zone DEFAULT now() NOT NULL,
	"url" varchar,
	"thumbnail_u_r_l" varchar,
	"filename" varchar,
	"mime_type" varchar,
	"filesize" numeric,
	"width" numeric,
	"height" numeric,
	"focal_x" numeric,
	"focal_y" numeric,
	"sizes_thumbnail_url" varchar,
	"sizes_thumbnail_width" numeric,
	"sizes_thumbnail_height" numeric,
	"sizes_thumbnail_mime_type" varchar,
	"sizes_thumbnail_filesize" numeric,
	"sizes_thumbnail_filename" varchar,
	"sizes_medium_url" varchar,
	"sizes_medium_width" numeric,
	"sizes_medium_height" numeric,
	"sizes_medium_mime_type" varchar,
	"sizes_medium_filesize" numeric,
	"sizes_medium_filename" varchar,
	"sizes_large_url" varchar,
	"sizes_large_width" numeric,
	"sizes_large_height" numeric,
	"sizes_large_mime_type" varchar,
	"sizes_large_filesize" numeric,
	"sizes_large_filename" varchar
  );

  ALTER TABLE "argus_content"."payload_locked_documents_rels" ADD COLUMN "media_id" uuid;
  ALTER TABLE "argus_content"."media" ADD CONSTRAINT "media_project_id_project_spaces_id_fk" FOREIGN KEY ("project_id") REFERENCES "argus_content"."project_spaces"("id") ON DELETE set null ON UPDATE no action;
  CREATE INDEX "media_organization_id_idx" ON "argus_content"."media" USING btree ("organization_id");
  CREATE INDEX "media_argus_project_id_idx" ON "argus_content"."media" USING btree ("argus_project_id");
  CREATE INDEX "media_project_idx" ON "argus_content"."media" USING btree ("project_id");
  CREATE INDEX "media_updated_at_idx" ON "argus_content"."media" USING btree ("updated_at");
  CREATE INDEX "media_created_at_idx" ON "argus_content"."media" USING btree ("created_at");
  CREATE UNIQUE INDEX "media_filename_idx" ON "argus_content"."media" USING btree ("filename");
  CREATE INDEX "media_sizes_thumbnail_sizes_thumbnail_filename_idx" ON "argus_content"."media" USING btree ("sizes_thumbnail_filename");
  CREATE INDEX "media_sizes_medium_sizes_medium_filename_idx" ON "argus_content"."media" USING btree ("sizes_medium_filename");
  CREATE INDEX "media_sizes_large_sizes_large_filename_idx" ON "argus_content"."media" USING btree ("sizes_large_filename");
  ALTER TABLE "argus_content"."payload_locked_documents_rels" ADD CONSTRAINT "payload_locked_documents_rels_media_fk" FOREIGN KEY ("media_id") REFERENCES "argus_content"."media"("id") ON DELETE cascade ON UPDATE no action;
  CREATE INDEX "payload_locked_documents_rels_media_id_idx" ON "argus_content"."payload_locked_documents_rels" USING btree ("media_id");`)
}

export async function down({ db, payload, req }: MigrateDownArgs): Promise<void> {
  await db.execute(sql`
   ALTER TABLE "argus_content"."payload_locked_documents_rels" DROP CONSTRAINT "payload_locked_documents_rels_media_fk";
  DROP INDEX "argus_content"."payload_locked_documents_rels_media_id_idx";
  ALTER TABLE "argus_content"."payload_locked_documents_rels" DROP COLUMN "media_id";
  ALTER TABLE "argus_content"."media" DISABLE ROW LEVEL SECURITY;
  DROP TABLE "argus_content"."media" CASCADE;`)
}
