import { MigrateUpArgs, MigrateDownArgs, sql } from '@payloadcms/db-postgres'

export async function up({ db, payload, req }: MigrateUpArgs): Promise<void> {
  await db.execute(sql`
   CREATE TYPE "argus_content"."enum_data_models_content_role" AS ENUM('collection', 'page', 'component');
  CREATE TABLE "argus_content"."data_models_rels" (
	"id" serial PRIMARY KEY NOT NULL,
	"order" integer,
	"parent_id" uuid NOT NULL,
	"path" varchar NOT NULL,
	"data_models_id" uuid
  );

  ALTER TABLE "argus_content"."data_models" ADD COLUMN "content_role" "argus_content"."enum_data_models_content_role" DEFAULT 'collection' NOT NULL;
  ALTER TABLE "argus_content"."data_records" ADD COLUMN "layout" jsonb DEFAULT '[]'::jsonb;
  ALTER TABLE "argus_content"."_data_records_v" ADD COLUMN "version_layout" jsonb DEFAULT '[]'::jsonb;
  ALTER TABLE "argus_content"."data_models_rels" ADD CONSTRAINT "data_models_rels_parent_fk" FOREIGN KEY ("parent_id") REFERENCES "argus_content"."data_models"("id") ON DELETE cascade ON UPDATE no action;
  ALTER TABLE "argus_content"."data_models_rels" ADD CONSTRAINT "data_models_rels_data_models_fk" FOREIGN KEY ("data_models_id") REFERENCES "argus_content"."data_models"("id") ON DELETE cascade ON UPDATE no action;
  CREATE INDEX "data_models_rels_order_idx" ON "argus_content"."data_models_rels" USING btree ("order");
  CREATE INDEX "data_models_rels_parent_idx" ON "argus_content"."data_models_rels" USING btree ("parent_id");
  CREATE INDEX "data_models_rels_path_idx" ON "argus_content"."data_models_rels" USING btree ("path");
  CREATE INDEX "data_models_rels_data_models_id_idx" ON "argus_content"."data_models_rels" USING btree ("data_models_id");`)
}

export async function down({ db, payload, req }: MigrateDownArgs): Promise<void> {
  await db.execute(sql`
   DROP TABLE "argus_content"."data_models_rels" CASCADE;
  ALTER TABLE "argus_content"."data_models" DROP COLUMN "content_role";
  ALTER TABLE "argus_content"."data_records" DROP COLUMN "layout";
  ALTER TABLE "argus_content"."_data_records_v" DROP COLUMN "version_layout";
  DROP TYPE "argus_content"."enum_data_models_content_role";`)
}
