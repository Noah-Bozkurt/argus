# Payload CMS V1

CMS V1 builds editorial content on the existing Payload App Data substrate instead of creating a second content database.

## Model kinds

`data-models.kind` remains the boundary:

- `data`: application data; records publish immediately and do not use editorial draft semantics;
- `content`: editorial content; records may be saved as drafts and explicitly published.

Model `slug` and `kind` are immutable after creation because they form stable API/editorial semantics.

## Drafts and versions

`data-records` uses Payload Versions with Drafts and keeps up to 50 versions per record.

For `kind=data`, the record hook forces Payload `_status=published`, preserving the direct-write behavior from App Data V1.

For `kind=content`, Payload's normal draft behavior applies:

- draft edits are stored in the versions table;
- the previously published main record remains unchanged;
- publishing updates the main record;
- `publishedAt` is recorded when content transitions from draft to published.

CMS V1 intentionally uses manual draft saves rather than autosave. Argus validates dynamic required fields in its own hook, so enabling partial autosave before that validator understands draft context would create noisy invalid autosaves.

## Migration compatibility

The CMS schema migration starts from Payload's generated migration and Drizzle snapshot, then applies a small explicit ordering correction for the existing App Data V1 record lifecycle column.

The existing `data_records.status` values (`active` / `archived`) are preserved in place. Its PostgreSQL enum is renamed to `enum_data_records_lifecycle_status`, while Payload receives the freed `enum_data_records_status` name for its new internal `_status` draft state (`draft` / `published`). Existing records are marked published when `_status` is introduced, so adding CMS drafts does not silently hide application data that pre-dates CMS V1.

The committed migration is validated from a clean PostgreSQL 16 database together with the initial App Data migration. CI verifies the isolated `argus_content` schema, both migration records, repeat migration startup and a production Payload build.

## Explicit public exposure

Content models are private by default. A content model must satisfy all of the following before any record is exposed publicly:

- `kind=content`;
- model `status=active`;
- `publicRead=true`;
- mirrored Project `status=active`;
- record lifecycle `status=active`;
- Payload `_status=published`.

Setting `publicRead` on an application-data model is normalized back to false.

## Public API

Published content is available from:

```text
GET /public/projects/:argusProjectId/content/:modelSlug
```

Optional query parameters:

- `page`, minimum 1;
- `limit`, 1-100, default 50.

The response contains only:

- model slug and schema version;
- record public ID;
- validated scalar `values`;
- publication/update timestamps;
- pagination metadata.

It does not return Organization IDs, Payload project IDs, membership information, user records, drafts, archived records, internal model configuration, migration data or secrets.

The endpoint is read-only and sends permissive CORS headers because the returned records have already been explicitly marked public. It does not accept credentials. Responses may be cached for 60 seconds with stale-while-revalidate for 5 minutes.

## Relationships

CMS V1 does not project `data-relations` into the public API yet. Publishing relationship graphs safely requires checking that every target model is itself public and every target record is published; that should be added explicitly rather than leaking internal relation IDs.

## Not yet Visual CMS

This phase deliberately does not add:

- page-layout blocks;
- drag-and-drop editing;
- live site preview;
- media uploads;
- forms;
- client approval flows.

Those features can now build on stable content models, drafts, published versions and a public read boundary instead of inventing their own storage rules.
