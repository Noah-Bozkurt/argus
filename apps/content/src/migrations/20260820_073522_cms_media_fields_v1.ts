import { MigrateUpArgs, MigrateDownArgs, sql } from '@payloadcms/db-postgres'

export async function up({ db, payload, req }: MigrateUpArgs): Promise<void> {
  await db.execute(sql`
   ALTER TYPE "argus_content"."enum_data_models_fields_type" ADD VALUE 'media';`)
}

export async function down({ db, payload, req }: MigrateDownArgs): Promise<void> {
  await db.execute(sql`
   ALTER TABLE "argus_content"."data_models_fields" ALTER COLUMN "type" SET DATA TYPE text;
  DROP TYPE "argus_content"."enum_data_models_fields_type";
  CREATE TYPE "argus_content"."enum_data_models_fields_type" AS ENUM('text', 'textarea', 'number', 'boolean', 'date', 'datetime', 'json', 'relationship');
  ALTER TABLE "argus_content"."data_models_fields" ALTER COLUMN "type" SET DATA TYPE "argus_content"."enum_data_models_fields_type" USING "type"::"argus_content"."enum_data_models_fields_type";`)
}
