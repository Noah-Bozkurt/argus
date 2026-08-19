import { MigrateUpArgs, MigrateDownArgs, sql } from '@payloadcms/db-postgres'

export async function up({ db, payload, req }: MigrateUpArgs): Promise<void> {
  await db.execute(sql`
   CREATE TYPE "argus_content"."enum_workspace_users_role" AS ENUM('admin', 'member');
  CREATE TYPE "argus_content"."enum_project_spaces_status" AS ENUM('active', 'paused', 'archived');
  CREATE TYPE "argus_content"."enum_project_memberships_role" AS ENUM('manager', 'editor', 'viewer');
  CREATE TYPE "argus_content"."enum_data_models_fields_type" AS ENUM('text', 'textarea', 'number', 'boolean', 'date', 'datetime', 'json', 'relationship');
  CREATE TYPE "argus_content"."enum_data_models_kind" AS ENUM('data', 'content');
  CREATE TYPE "argus_content"."enum_data_models_status" AS ENUM('active', 'archived');
  CREATE TYPE "argus_content"."enum_data_records_status" AS ENUM('active', 'archived');
  CREATE TABLE "argus_content"."workspace_users_sessions" (
  	"_order" integer NOT NULL,
  	"_parent_id" uuid NOT NULL,
  	"id" varchar PRIMARY KEY NOT NULL,
  	"created_at" timestamp(3) with time zone,
  	"expires_at" timestamp(3) with time zone NOT NULL
  );
  
  CREATE TABLE "argus_content"."workspace_users" (
  	"id" uuid PRIMARY KEY DEFAULT gen_random_uuid() NOT NULL,
  	"display_name" varchar NOT NULL,
  	"organization_id" varchar NOT NULL,
  	"argus_user_id" varchar,
  	"role" "argus_content"."enum_workspace_users_role" DEFAULT 'member' NOT NULL,
  	"updated_at" timestamp(3) with time zone DEFAULT now() NOT NULL,
  	"created_at" timestamp(3) with time zone DEFAULT now() NOT NULL,
  	"email" varchar NOT NULL,
  	"reset_password_token" varchar,
  	"reset_password_expiration" timestamp(3) with time zone,
  	"salt" varchar,
  	"hash" varchar,
  	"login_attempts" numeric DEFAULT 0,
  	"lock_until" timestamp(3) with time zone
  );
  
  CREATE TABLE "argus_content"."project_spaces" (
  	"id" uuid PRIMARY KEY DEFAULT gen_random_uuid() NOT NULL,
  	"argus_project_id" varchar NOT NULL,
  	"organization_id" varchar NOT NULL,
  	"name" varchar NOT NULL,
  	"client_id" varchar,
  	"status" "argus_content"."enum_project_spaces_status" DEFAULT 'active' NOT NULL,
  	"updated_at" timestamp(3) with time zone DEFAULT now() NOT NULL,
  	"created_at" timestamp(3) with time zone DEFAULT now() NOT NULL
  );
  
  CREATE TABLE "argus_content"."project_memberships" (
  	"id" uuid PRIMARY KEY DEFAULT gen_random_uuid() NOT NULL,
  	"organization_id" varchar NOT NULL,
  	"project_id" uuid NOT NULL,
  	"user_id" uuid NOT NULL,
  	"role" "argus_content"."enum_project_memberships_role" DEFAULT 'viewer' NOT NULL,
  	"updated_at" timestamp(3) with time zone DEFAULT now() NOT NULL,
  	"created_at" timestamp(3) with time zone DEFAULT now() NOT NULL
  );
  
  CREATE TABLE "argus_content"."data_models_fields" (
  	"_order" integer NOT NULL,
  	"_parent_id" uuid NOT NULL,
  	"id" varchar PRIMARY KEY NOT NULL,
  	"key" varchar NOT NULL,
  	"label" varchar NOT NULL,
  	"type" "argus_content"."enum_data_models_fields_type" NOT NULL,
  	"required" boolean DEFAULT false,
  	"has_many" boolean DEFAULT false,
  	"target_model_id" uuid,
  	"settings" jsonb
  );
  
  CREATE TABLE "argus_content"."data_models" (
  	"id" uuid PRIMARY KEY DEFAULT gen_random_uuid() NOT NULL,
  	"organization_id" varchar NOT NULL,
  	"argus_project_id" varchar NOT NULL,
  	"project_id" uuid NOT NULL,
  	"name" varchar NOT NULL,
  	"slug" varchar NOT NULL,
  	"description" varchar,
  	"kind" "argus_content"."enum_data_models_kind" DEFAULT 'data' NOT NULL,
  	"schema_version" numeric DEFAULT 1 NOT NULL,
  	"status" "argus_content"."enum_data_models_status" DEFAULT 'active' NOT NULL,
  	"updated_at" timestamp(3) with time zone DEFAULT now() NOT NULL,
  	"created_at" timestamp(3) with time zone DEFAULT now() NOT NULL
  );
  
  CREATE TABLE "argus_content"."data_records" (
  	"id" uuid PRIMARY KEY DEFAULT gen_random_uuid() NOT NULL,
  	"organization_id" varchar NOT NULL,
  	"argus_project_id" varchar NOT NULL,
  	"project_id" uuid NOT NULL,
  	"model_id" uuid NOT NULL,
  	"schema_version" numeric NOT NULL,
  	"values" jsonb DEFAULT '{}'::jsonb NOT NULL,
  	"status" "argus_content"."enum_data_records_status" DEFAULT 'active' NOT NULL,
  	"created_by_id" uuid,
  	"updated_at" timestamp(3) with time zone DEFAULT now() NOT NULL,
  	"created_at" timestamp(3) with time zone DEFAULT now() NOT NULL
  );
  
  CREATE TABLE "argus_content"."data_relations" (
  	"id" uuid PRIMARY KEY DEFAULT gen_random_uuid() NOT NULL,
  	"organization_id" varchar NOT NULL,
  	"argus_project_id" varchar NOT NULL,
  	"project_id" uuid NOT NULL,
  	"source_model_id" uuid NOT NULL,
  	"source_record_id" uuid NOT NULL,
  	"field_key" varchar NOT NULL,
  	"target_model_id" uuid NOT NULL,
  	"target_record_id" uuid NOT NULL,
  	"updated_at" timestamp(3) with time zone DEFAULT now() NOT NULL,
  	"created_at" timestamp(3) with time zone DEFAULT now() NOT NULL
  );
  
  CREATE TABLE "argus_content"."payload_kv" (
  	"id" uuid PRIMARY KEY DEFAULT gen_random_uuid() NOT NULL,
  	"key" varchar NOT NULL,
  	"data" jsonb NOT NULL
  );
  
  CREATE TABLE "argus_content"."payload_locked_documents" (
  	"id" uuid PRIMARY KEY DEFAULT gen_random_uuid() NOT NULL,
  	"global_slug" varchar,
  	"updated_at" timestamp(3) with time zone DEFAULT now() NOT NULL,
  	"created_at" timestamp(3) with time zone DEFAULT now() NOT NULL
  );
  
  CREATE TABLE "argus_content"."payload_locked_documents_rels" (
  	"id" serial PRIMARY KEY NOT NULL,
  	"order" integer,
  	"parent_id" uuid NOT NULL,
  	"path" varchar NOT NULL,
  	"workspace_users_id" uuid,
  	"project_spaces_id" uuid,
  	"project_memberships_id" uuid,
  	"data_models_id" uuid,
  	"data_records_id" uuid,
  	"data_relations_id" uuid
  );
  
  CREATE TABLE "argus_content"."payload_preferences" (
  	"id" uuid PRIMARY KEY DEFAULT gen_random_uuid() NOT NULL,
  	"key" varchar,
  	"value" jsonb,
  	"updated_at" timestamp(3) with time zone DEFAULT now() NOT NULL,
  	"created_at" timestamp(3) with time zone DEFAULT now() NOT NULL
  );
  
  CREATE TABLE "argus_content"."payload_preferences_rels" (
  	"id" serial PRIMARY KEY NOT NULL,
  	"order" integer,
  	"parent_id" uuid NOT NULL,
  	"path" varchar NOT NULL,
  	"workspace_users_id" uuid
  );
  
  CREATE TABLE "argus_content"."payload_migrations" (
  	"id" uuid PRIMARY KEY DEFAULT gen_random_uuid() NOT NULL,
  	"name" varchar,
  	"batch" numeric,
  	"updated_at" timestamp(3) with time zone DEFAULT now() NOT NULL,
  	"created_at" timestamp(3) with time zone DEFAULT now() NOT NULL
  );
  
  ALTER TABLE "argus_content"."workspace_users_sessions" ADD CONSTRAINT "workspace_users_sessions_parent_id_fk" FOREIGN KEY ("_parent_id") REFERENCES "argus_content"."workspace_users"("id") ON DELETE cascade ON UPDATE no action;
  ALTER TABLE "argus_content"."project_memberships" ADD CONSTRAINT "project_memberships_project_id_project_spaces_id_fk" FOREIGN KEY ("project_id") REFERENCES "argus_content"."project_spaces"("id") ON DELETE set null ON UPDATE no action;
  ALTER TABLE "argus_content"."project_memberships" ADD CONSTRAINT "project_memberships_user_id_workspace_users_id_fk" FOREIGN KEY ("user_id") REFERENCES "argus_content"."workspace_users"("id") ON DELETE set null ON UPDATE no action;
  ALTER TABLE "argus_content"."data_models_fields" ADD CONSTRAINT "data_models_fields_target_model_id_data_models_id_fk" FOREIGN KEY ("target_model_id") REFERENCES "argus_content"."data_models"("id") ON DELETE set null ON UPDATE no action;
  ALTER TABLE "argus_content"."data_models_fields" ADD CONSTRAINT "data_models_fields_parent_id_fk" FOREIGN KEY ("_parent_id") REFERENCES "argus_content"."data_models"("id") ON DELETE cascade ON UPDATE no action;
  ALTER TABLE "argus_content"."data_models" ADD CONSTRAINT "data_models_project_id_project_spaces_id_fk" FOREIGN KEY ("project_id") REFERENCES "argus_content"."project_spaces"("id") ON DELETE set null ON UPDATE no action;
  ALTER TABLE "argus_content"."data_records" ADD CONSTRAINT "data_records_project_id_project_spaces_id_fk" FOREIGN KEY ("project_id") REFERENCES "argus_content"."project_spaces"("id") ON DELETE set null ON UPDATE no action;
  ALTER TABLE "argus_content"."data_records" ADD CONSTRAINT "data_records_model_id_data_models_id_fk" FOREIGN KEY ("model_id") REFERENCES "argus_content"."data_models"("id") ON DELETE set null ON UPDATE no action;
  ALTER TABLE "argus_content"."data_records" ADD CONSTRAINT "data_records_created_by_id_workspace_users_id_fk" FOREIGN KEY ("created_by_id") REFERENCES "argus_content"."workspace_users"("id") ON DELETE set null ON UPDATE no action;
  ALTER TABLE "argus_content"."data_relations" ADD CONSTRAINT "data_relations_project_id_project_spaces_id_fk" FOREIGN KEY ("project_id") REFERENCES "argus_content"."project_spaces"("id") ON DELETE set null ON UPDATE no action;
  ALTER TABLE "argus_content"."data_relations" ADD CONSTRAINT "data_relations_source_model_id_data_models_id_fk" FOREIGN KEY ("source_model_id") REFERENCES "argus_content"."data_models"("id") ON DELETE set null ON UPDATE no action;
  ALTER TABLE "argus_content"."data_relations" ADD CONSTRAINT "data_relations_source_record_id_data_records_id_fk" FOREIGN KEY ("source_record_id") REFERENCES "argus_content"."data_records"("id") ON DELETE set null ON UPDATE no action;
  ALTER TABLE "argus_content"."data_relations" ADD CONSTRAINT "data_relations_target_model_id_data_models_id_fk" FOREIGN KEY ("target_model_id") REFERENCES "argus_content"."data_models"("id") ON DELETE set null ON UPDATE no action;
  ALTER TABLE "argus_content"."data_relations" ADD CONSTRAINT "data_relations_target_record_id_data_records_id_fk" FOREIGN KEY ("target_record_id") REFERENCES "argus_content"."data_records"("id") ON DELETE set null ON UPDATE no action;
  ALTER TABLE "argus_content"."payload_locked_documents_rels" ADD CONSTRAINT "payload_locked_documents_rels_parent_fk" FOREIGN KEY ("parent_id") REFERENCES "argus_content"."payload_locked_documents"("id") ON DELETE cascade ON UPDATE no action;
  ALTER TABLE "argus_content"."payload_locked_documents_rels" ADD CONSTRAINT "payload_locked_documents_rels_workspace_users_fk" FOREIGN KEY ("workspace_users_id") REFERENCES "argus_content"."workspace_users"("id") ON DELETE cascade ON UPDATE no action;
  ALTER TABLE "argus_content"."payload_locked_documents_rels" ADD CONSTRAINT "payload_locked_documents_rels_project_spaces_fk" FOREIGN KEY ("project_spaces_id") REFERENCES "argus_content"."project_spaces"("id") ON DELETE cascade ON UPDATE no action;
  ALTER TABLE "argus_content"."payload_locked_documents_rels" ADD CONSTRAINT "payload_locked_documents_rels_project_memberships_fk" FOREIGN KEY ("project_memberships_id") REFERENCES "argus_content"."project_memberships"("id") ON DELETE cascade ON UPDATE no action;
  ALTER TABLE "argus_content"."payload_locked_documents_rels" ADD CONSTRAINT "payload_locked_documents_rels_data_models_fk" FOREIGN KEY ("data_models_id") REFERENCES "argus_content"."data_models"("id") ON DELETE cascade ON UPDATE no action;
  ALTER TABLE "argus_content"."payload_locked_documents_rels" ADD CONSTRAINT "payload_locked_documents_rels_data_records_fk" FOREIGN KEY ("data_records_id") REFERENCES "argus_content"."data_records"("id") ON DELETE cascade ON UPDATE no action;
  ALTER TABLE "argus_content"."payload_locked_documents_rels" ADD CONSTRAINT "payload_locked_documents_rels_data_relations_fk" FOREIGN KEY ("data_relations_id") REFERENCES "argus_content"."data_relations"("id") ON DELETE cascade ON UPDATE no action;
  ALTER TABLE "argus_content"."payload_preferences_rels" ADD CONSTRAINT "payload_preferences_rels_parent_fk" FOREIGN KEY ("parent_id") REFERENCES "argus_content"."payload_preferences"("id") ON DELETE cascade ON UPDATE no action;
  ALTER TABLE "argus_content"."payload_preferences_rels" ADD CONSTRAINT "payload_preferences_rels_workspace_users_fk" FOREIGN KEY ("workspace_users_id") REFERENCES "argus_content"."workspace_users"("id") ON DELETE cascade ON UPDATE no action;
  CREATE INDEX "workspace_users_sessions_order_idx" ON "argus_content"."workspace_users_sessions" USING btree ("_order");
  CREATE INDEX "workspace_users_sessions_parent_id_idx" ON "argus_content"."workspace_users_sessions" USING btree ("_parent_id");
  CREATE INDEX "workspace_users_organization_id_idx" ON "argus_content"."workspace_users" USING btree ("organization_id");
  CREATE INDEX "workspace_users_argus_user_id_idx" ON "argus_content"."workspace_users" USING btree ("argus_user_id");
  CREATE INDEX "workspace_users_updated_at_idx" ON "argus_content"."workspace_users" USING btree ("updated_at");
  CREATE INDEX "workspace_users_created_at_idx" ON "argus_content"."workspace_users" USING btree ("created_at");
  CREATE UNIQUE INDEX "workspace_users_email_idx" ON "argus_content"."workspace_users" USING btree ("email");
  CREATE UNIQUE INDEX "project_spaces_argus_project_id_idx" ON "argus_content"."project_spaces" USING btree ("argus_project_id");
  CREATE INDEX "project_spaces_organization_id_idx" ON "argus_content"."project_spaces" USING btree ("organization_id");
  CREATE INDEX "project_spaces_client_id_idx" ON "argus_content"."project_spaces" USING btree ("client_id");
  CREATE INDEX "project_spaces_updated_at_idx" ON "argus_content"."project_spaces" USING btree ("updated_at");
  CREATE INDEX "project_spaces_created_at_idx" ON "argus_content"."project_spaces" USING btree ("created_at");
  CREATE INDEX "project_memberships_organization_id_idx" ON "argus_content"."project_memberships" USING btree ("organization_id");
  CREATE INDEX "project_memberships_project_idx" ON "argus_content"."project_memberships" USING btree ("project_id");
  CREATE INDEX "project_memberships_user_idx" ON "argus_content"."project_memberships" USING btree ("user_id");
  CREATE INDEX "project_memberships_updated_at_idx" ON "argus_content"."project_memberships" USING btree ("updated_at");
  CREATE INDEX "project_memberships_created_at_idx" ON "argus_content"."project_memberships" USING btree ("created_at");
  CREATE INDEX "data_models_fields_order_idx" ON "argus_content"."data_models_fields" USING btree ("_order");
  CREATE INDEX "data_models_fields_parent_id_idx" ON "argus_content"."data_models_fields" USING btree ("_parent_id");
  CREATE INDEX "data_models_fields_target_model_idx" ON "argus_content"."data_models_fields" USING btree ("target_model_id");
  CREATE INDEX "data_models_organization_id_idx" ON "argus_content"."data_models" USING btree ("organization_id");
  CREATE INDEX "data_models_argus_project_id_idx" ON "argus_content"."data_models" USING btree ("argus_project_id");
  CREATE INDEX "data_models_project_idx" ON "argus_content"."data_models" USING btree ("project_id");
  CREATE INDEX "data_models_slug_idx" ON "argus_content"."data_models" USING btree ("slug");
  CREATE INDEX "data_models_updated_at_idx" ON "argus_content"."data_models" USING btree ("updated_at");
  CREATE INDEX "data_models_created_at_idx" ON "argus_content"."data_models" USING btree ("created_at");
  CREATE INDEX "data_records_organization_id_idx" ON "argus_content"."data_records" USING btree ("organization_id");
  CREATE INDEX "data_records_argus_project_id_idx" ON "argus_content"."data_records" USING btree ("argus_project_id");
  CREATE INDEX "data_records_project_idx" ON "argus_content"."data_records" USING btree ("project_id");
  CREATE INDEX "data_records_model_idx" ON "argus_content"."data_records" USING btree ("model_id");
  CREATE INDEX "data_records_created_by_idx" ON "argus_content"."data_records" USING btree ("created_by_id");
  CREATE INDEX "data_records_updated_at_idx" ON "argus_content"."data_records" USING btree ("updated_at");
  CREATE INDEX "data_records_created_at_idx" ON "argus_content"."data_records" USING btree ("created_at");
  CREATE INDEX "data_relations_organization_id_idx" ON "argus_content"."data_relations" USING btree ("organization_id");
  CREATE INDEX "data_relations_argus_project_id_idx" ON "argus_content"."data_relations" USING btree ("argus_project_id");
  CREATE INDEX "data_relations_project_idx" ON "argus_content"."data_relations" USING btree ("project_id");
  CREATE INDEX "data_relations_source_model_idx" ON "argus_content"."data_relations" USING btree ("source_model_id");
  CREATE INDEX "data_relations_source_record_idx" ON "argus_content"."data_relations" USING btree ("source_record_id");
  CREATE INDEX "data_relations_field_key_idx" ON "argus_content"."data_relations" USING btree ("field_key");
  CREATE INDEX "data_relations_target_model_idx" ON "argus_content"."data_relations" USING btree ("target_model_id");
  CREATE INDEX "data_relations_target_record_idx" ON "argus_content"."data_relations" USING btree ("target_record_id");
  CREATE INDEX "data_relations_updated_at_idx" ON "argus_content"."data_relations" USING btree ("updated_at");
  CREATE INDEX "data_relations_created_at_idx" ON "argus_content"."data_relations" USING btree ("created_at");
  CREATE UNIQUE INDEX "payload_kv_key_idx" ON "argus_content"."payload_kv" USING btree ("key");
  CREATE INDEX "payload_locked_documents_global_slug_idx" ON "argus_content"."payload_locked_documents" USING btree ("global_slug");
  CREATE INDEX "payload_locked_documents_updated_at_idx" ON "argus_content"."payload_locked_documents" USING btree ("updated_at");
  CREATE INDEX "payload_locked_documents_created_at_idx" ON "argus_content"."payload_locked_documents" USING btree ("created_at");
  CREATE INDEX "payload_locked_documents_rels_order_idx" ON "argus_content"."payload_locked_documents_rels" USING btree ("order");
  CREATE INDEX "payload_locked_documents_rels_parent_idx" ON "argus_content"."payload_locked_documents_rels" USING btree ("parent_id");
  CREATE INDEX "payload_locked_documents_rels_path_idx" ON "argus_content"."payload_locked_documents_rels" USING btree ("path");
  CREATE INDEX "payload_locked_documents_rels_workspace_users_id_idx" ON "argus_content"."payload_locked_documents_rels" USING btree ("workspace_users_id");
  CREATE INDEX "payload_locked_documents_rels_project_spaces_id_idx" ON "argus_content"."payload_locked_documents_rels" USING btree ("project_spaces_id");
  CREATE INDEX "payload_locked_documents_rels_project_memberships_id_idx" ON "argus_content"."payload_locked_documents_rels" USING btree ("project_memberships_id");
  CREATE INDEX "payload_locked_documents_rels_data_models_id_idx" ON "argus_content"."payload_locked_documents_rels" USING btree ("data_models_id");
  CREATE INDEX "payload_locked_documents_rels_data_records_id_idx" ON "argus_content"."payload_locked_documents_rels" USING btree ("data_records_id");
  CREATE INDEX "payload_locked_documents_rels_data_relations_id_idx" ON "argus_content"."payload_locked_documents_rels" USING btree ("data_relations_id");
  CREATE INDEX "payload_preferences_key_idx" ON "argus_content"."payload_preferences" USING btree ("key");
  CREATE INDEX "payload_preferences_updated_at_idx" ON "argus_content"."payload_preferences" USING btree ("updated_at");
  CREATE INDEX "payload_preferences_created_at_idx" ON "argus_content"."payload_preferences" USING btree ("created_at");
  CREATE INDEX "payload_preferences_rels_order_idx" ON "argus_content"."payload_preferences_rels" USING btree ("order");
  CREATE INDEX "payload_preferences_rels_parent_idx" ON "argus_content"."payload_preferences_rels" USING btree ("parent_id");
  CREATE INDEX "payload_preferences_rels_path_idx" ON "argus_content"."payload_preferences_rels" USING btree ("path");
  CREATE INDEX "payload_preferences_rels_workspace_users_id_idx" ON "argus_content"."payload_preferences_rels" USING btree ("workspace_users_id");
  CREATE INDEX "payload_migrations_updated_at_idx" ON "argus_content"."payload_migrations" USING btree ("updated_at");
  CREATE INDEX "payload_migrations_created_at_idx" ON "argus_content"."payload_migrations" USING btree ("created_at");`)
}

export async function down({ db, payload, req }: MigrateDownArgs): Promise<void> {
  await db.execute(sql`
   DROP TABLE "argus_content"."workspace_users_sessions" CASCADE;
  DROP TABLE "argus_content"."workspace_users" CASCADE;
  DROP TABLE "argus_content"."project_spaces" CASCADE;
  DROP TABLE "argus_content"."project_memberships" CASCADE;
  DROP TABLE "argus_content"."data_models_fields" CASCADE;
  DROP TABLE "argus_content"."data_models" CASCADE;
  DROP TABLE "argus_content"."data_records" CASCADE;
  DROP TABLE "argus_content"."data_relations" CASCADE;
  DROP TABLE "argus_content"."payload_kv" CASCADE;
  DROP TABLE "argus_content"."payload_locked_documents" CASCADE;
  DROP TABLE "argus_content"."payload_locked_documents_rels" CASCADE;
  DROP TABLE "argus_content"."payload_preferences" CASCADE;
  DROP TABLE "argus_content"."payload_preferences_rels" CASCADE;
  DROP TABLE "argus_content"."payload_migrations" CASCADE;
  DROP TYPE "argus_content"."enum_workspace_users_role";
  DROP TYPE "argus_content"."enum_project_spaces_status";
  DROP TYPE "argus_content"."enum_project_memberships_role";
  DROP TYPE "argus_content"."enum_data_models_fields_type";
  DROP TYPE "argus_content"."enum_data_models_kind";
  DROP TYPE "argus_content"."enum_data_models_status";
  DROP TYPE "argus_content"."enum_data_records_status";`)
}
