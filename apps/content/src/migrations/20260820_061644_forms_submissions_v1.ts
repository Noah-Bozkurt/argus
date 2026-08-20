import { MigrateUpArgs, MigrateDownArgs, sql } from '@payloadcms/db-postgres'

export async function up({ db, payload, req }: MigrateUpArgs): Promise<void> {
  await db.execute(sql`
   CREATE TYPE "argus_content"."enum_form_definitions_fields_type" AS ENUM('text', 'email', 'textarea', 'number', 'boolean', 'select');
  CREATE TYPE "argus_content"."enum_form_definitions_status" AS ENUM('draft', 'published', 'archived');
  CREATE TYPE "argus_content"."enum_form_submissions_status" AS ENUM('new', 'reviewed', 'spam', 'archived');
  CREATE TABLE "argus_content"."form_definitions_fields_options" (
	"_order" integer NOT NULL,
	"_parent_id" varchar NOT NULL,
	"id" varchar PRIMARY KEY NOT NULL,
	"value" varchar NOT NULL
  );

  CREATE TABLE "argus_content"."form_definitions_fields" (
	"_order" integer NOT NULL,
	"_parent_id" uuid NOT NULL,
	"id" varchar PRIMARY KEY NOT NULL,
	"key" varchar NOT NULL,
	"label" varchar NOT NULL,
	"type" "argus_content"."enum_form_definitions_fields_type" NOT NULL,
	"required" boolean DEFAULT false
  );

  CREATE TABLE "argus_content"."form_definitions" (
	"id" uuid PRIMARY KEY DEFAULT gen_random_uuid() NOT NULL,
	"organization_id" varchar NOT NULL,
	"argus_project_id" varchar NOT NULL,
	"project_id" uuid NOT NULL,
	"name" varchar NOT NULL,
	"slug" varchar NOT NULL,
	"description" varchar,
	"success_message" varchar NOT NULL,
	"status" "argus_content"."enum_form_definitions_status" DEFAULT 'draft' NOT NULL,
	"updated_at" timestamp(3) with time zone DEFAULT now() NOT NULL,
	"created_at" timestamp(3) with time zone DEFAULT now() NOT NULL
  );

  CREATE TABLE "argus_content"."form_submissions" (
	"id" uuid PRIMARY KEY DEFAULT gen_random_uuid() NOT NULL,
	"organization_id" varchar NOT NULL,
	"argus_project_id" varchar NOT NULL,
	"project_id" uuid NOT NULL,
	"form_id" uuid NOT NULL,
	"values" jsonb NOT NULL,
	"status" "argus_content"."enum_form_submissions_status" DEFAULT 'new' NOT NULL,
	"source_hash" varchar NOT NULL,
	"rate_window" varchar NOT NULL,
	"rate_key" varchar NOT NULL,
	"submitted_at" timestamp(3) with time zone NOT NULL,
	"updated_at" timestamp(3) with time zone DEFAULT now() NOT NULL,
	"created_at" timestamp(3) with time zone DEFAULT now() NOT NULL
  );

  ALTER TABLE "argus_content"."payload_locked_documents_rels" ADD COLUMN "form_definitions_id" uuid;
  ALTER TABLE "argus_content"."payload_locked_documents_rels" ADD COLUMN "form_submissions_id" uuid;
  ALTER TABLE "argus_content"."form_definitions_fields_options" ADD CONSTRAINT "form_definitions_fields_options_parent_id_fk" FOREIGN KEY ("_parent_id") REFERENCES "argus_content"."form_definitions_fields"("id") ON DELETE cascade ON UPDATE no action;
  ALTER TABLE "argus_content"."form_definitions_fields" ADD CONSTRAINT "form_definitions_fields_parent_id_fk" FOREIGN KEY ("_parent_id") REFERENCES "argus_content"."form_definitions"("id") ON DELETE cascade ON UPDATE no action;
  ALTER TABLE "argus_content"."form_definitions" ADD CONSTRAINT "form_definitions_project_id_project_spaces_id_fk" FOREIGN KEY ("project_id") REFERENCES "argus_content"."project_spaces"("id") ON DELETE set null ON UPDATE no action;
  ALTER TABLE "argus_content"."form_submissions" ADD CONSTRAINT "form_submissions_project_id_project_spaces_id_fk" FOREIGN KEY ("project_id") REFERENCES "argus_content"."project_spaces"("id") ON DELETE set null ON UPDATE no action;
  ALTER TABLE "argus_content"."form_submissions" ADD CONSTRAINT "form_submissions_form_id_form_definitions_id_fk" FOREIGN KEY ("form_id") REFERENCES "argus_content"."form_definitions"("id") ON DELETE set null ON UPDATE no action;
  CREATE INDEX "form_definitions_fields_options_order_idx" ON "argus_content"."form_definitions_fields_options" USING btree ("_order");
  CREATE INDEX "form_definitions_fields_options_parent_id_idx" ON "argus_content"."form_definitions_fields_options" USING btree ("_parent_id");
  CREATE INDEX "form_definitions_fields_order_idx" ON "argus_content"."form_definitions_fields" USING btree ("_order");
  CREATE INDEX "form_definitions_fields_parent_id_idx" ON "argus_content"."form_definitions_fields" USING btree ("_parent_id");
  CREATE INDEX "form_definitions_organization_id_idx" ON "argus_content"."form_definitions" USING btree ("organization_id");
  CREATE INDEX "form_definitions_argus_project_id_idx" ON "argus_content"."form_definitions" USING btree ("argus_project_id");
  CREATE INDEX "form_definitions_project_idx" ON "argus_content"."form_definitions" USING btree ("project_id");
  CREATE INDEX "form_definitions_slug_idx" ON "argus_content"."form_definitions" USING btree ("slug");
  CREATE INDEX "form_definitions_updated_at_idx" ON "argus_content"."form_definitions" USING btree ("updated_at");
  CREATE INDEX "form_definitions_created_at_idx" ON "argus_content"."form_definitions" USING btree ("created_at");
  CREATE INDEX "form_submissions_organization_id_idx" ON "argus_content"."form_submissions" USING btree ("organization_id");
  CREATE INDEX "form_submissions_argus_project_id_idx" ON "argus_content"."form_submissions" USING btree ("argus_project_id");
  CREATE INDEX "form_submissions_project_idx" ON "argus_content"."form_submissions" USING btree ("project_id");
  CREATE INDEX "form_submissions_form_idx" ON "argus_content"."form_submissions" USING btree ("form_id");
  CREATE INDEX "form_submissions_source_hash_idx" ON "argus_content"."form_submissions" USING btree ("source_hash");
  CREATE INDEX "form_submissions_rate_window_idx" ON "argus_content"."form_submissions" USING btree ("rate_window");
  CREATE UNIQUE INDEX "form_submissions_rate_key_idx" ON "argus_content"."form_submissions" USING btree ("rate_key");
  CREATE INDEX "form_submissions_submitted_at_idx" ON "argus_content"."form_submissions" USING btree ("submitted_at");
  CREATE INDEX "form_submissions_updated_at_idx" ON "argus_content"."form_submissions" USING btree ("updated_at");
  CREATE INDEX "form_submissions_created_at_idx" ON "argus_content"."form_submissions" USING btree ("created_at");
  ALTER TABLE "argus_content"."payload_locked_documents_rels" ADD CONSTRAINT "payload_locked_documents_rels_form_definitions_fk" FOREIGN KEY ("form_definitions_id") REFERENCES "argus_content"."form_definitions"("id") ON DELETE cascade ON UPDATE no action;
  ALTER TABLE "argus_content"."payload_locked_documents_rels" ADD CONSTRAINT "payload_locked_documents_rels_form_submissions_fk" FOREIGN KEY ("form_submissions_id") REFERENCES "argus_content"."form_submissions"("id") ON DELETE cascade ON UPDATE no action;
  CREATE INDEX "payload_locked_documents_rels_form_definitions_id_idx" ON "argus_content"."payload_locked_documents_rels" USING btree ("form_definitions_id");
  CREATE INDEX "payload_locked_documents_rels_form_submissions_id_idx" ON "argus_content"."payload_locked_documents_rels" USING btree ("form_submissions_id");`)
}

export async function down({ db, payload, req }: MigrateDownArgs): Promise<void> {
  await db.execute(sql`
   ALTER TABLE "argus_content"."payload_locked_documents_rels" DROP CONSTRAINT "payload_locked_documents_rels_form_definitions_fk";
  ALTER TABLE "argus_content"."payload_locked_documents_rels" DROP CONSTRAINT "payload_locked_documents_rels_form_submissions_fk";
  DROP INDEX "argus_content"."payload_locked_documents_rels_form_definitions_id_idx";
  DROP INDEX "argus_content"."payload_locked_documents_rels_form_submissions_id_idx";
  ALTER TABLE "argus_content"."payload_locked_documents_rels" DROP COLUMN "form_definitions_id";
  ALTER TABLE "argus_content"."payload_locked_documents_rels" DROP COLUMN "form_submissions_id";
  ALTER TABLE "argus_content"."form_definitions_fields_options" DISABLE ROW LEVEL SECURITY;
  ALTER TABLE "argus_content"."form_definitions_fields" DISABLE ROW LEVEL SECURITY;
  ALTER TABLE "argus_content"."form_definitions" DISABLE ROW LEVEL SECURITY;
  ALTER TABLE "argus_content"."form_submissions" DISABLE ROW LEVEL SECURITY;
  DROP TABLE "argus_content"."form_definitions_fields_options" CASCADE;
  DROP TABLE "argus_content"."form_definitions_fields" CASCADE;
  DROP TABLE "argus_content"."form_submissions" CASCADE;
  DROP TABLE "argus_content"."form_definitions" CASCADE;
  DROP TYPE "argus_content"."enum_form_definitions_fields_type";
  DROP TYPE "argus_content"."enum_form_definitions_status";
  DROP TYPE "argus_content"."enum_form_submissions_status";`)
}
