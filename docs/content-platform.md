# Content Platform

Argus uses Payload as the application-data and content layer instead of building a second generic CMS/database framework inside the Rust control plane.

## Service boundary

`apps/content` is a standalone Payload 3 / Next.js service. It can run alongside the older operator web application without forcing both applications onto the same Next.js version.

Payload uses the same PostgreSQL database instance as the Control API but is isolated to the `argus_content` schema. Control-plane tables remain owned by the Rust services.

Production configuration uses committed Payload migrations; automatic development schema push must not be treated as the production migration strategy.

## Project ownership and synchronization

The Argus Control API remains the source of truth for Project identity. Payload stores a mirrored `project-space` containing the Argus Project UUID, organization boundary, name/status metadata and an optional client reference.

A background organization-level job periodically reconciles current Argus Projects into Payload. Existing projects are therefore backfilled as well as newly created ones.

Synchronization is idempotent and bounded. Payload unavailability must not block creation of a Project in the core Control API; the worker can retry synchronization later.

## Workspace users and project roles

Payload has its own authenticated workspace users scoped to an organization. Project membership is separate from organization membership.

Project roles:

- `manager` — manage project data structure/memberships;
- `editor` — edit project application/content data;
- `viewer` — read project data.

Organization admins can manage their organization while access filters prevent cross-organization reads/writes. Project scope is immutable after creation for records/models where moving data between projects would bypass authorization assumptions.

Client identity is optional metadata. Personal projects do not need a client object to use any App Data/CMS capability.

## App Data models

`data-models` define dynamic project-scoped schemas. Models have a stable slug, kind, status and versioned field definitions.

Supported scalar concepts include text, long text, number, boolean, date/date-time and JSON. Relationship fields point to another data model in the same project.

Model slugs and model kind are immutable after creation because they form stable API/storage semantics. Schema updates increment the model schema version.

## Records

`data-records` hold validated scalar values for a selected model. Record writes validate:

- project/model scope;
- archived model state;
- known field keys;
- scalar type compatibility;
- required scalar fields;
- separation of relationship fields from scalar JSON.

Record project/model ownership is immutable after creation.

## Relationships

`data-relations` store explicit record-to-record relationships rather than hiding foreign references inside arbitrary JSON.

The relationship validator checks:

- source and target records belong to the same project;
- the source model actually declares the relationship field;
- the target record belongs to the relationship's declared target model;
- single-value relationships do not receive multiple targets.

Public CMS V1 does not expand these relationships yet; public expansion needs an additional rule that every target is independently public/published.

## Data vs content

A model's `kind` distinguishes two semantics on the same substrate:

- `data` — application data, direct-write semantics;
- `content` — editorial content, draft/publish semantics.

This avoids maintaining two unrelated databases for structurally similar project data.

## CMS drafts and publication

`data-records` uses Payload Versions/Drafts and retains up to 50 versions per record.

For `kind=data`, Argus forces Payload `_status=published`, preserving immediate application-data behavior.

For `kind=content`:

- draft edits can exist without replacing the currently published main record;
- publishing updates the main record;
- `publishedAt` tracks draft-to-published transitions;
- manual draft saves are used in CMS V1.

Autosave is intentionally not enabled yet because Argus performs dynamic required-field validation; partial autosave needs explicit draft-aware validation rules first.

The existing Argus record lifecycle remains `status=active|archived`. Payload's editorial `_status=draft|published` is a separate concept.

## Public content access

Content is private by default. A model must explicitly set `publicRead=true`, be active and be `kind=content` before its published records can be returned through the public endpoint.

```text
GET /public/projects/:argusProjectId/content/:modelSlug
```

The endpoint also requires an active mirrored Project and active/published record. It returns only the public record ID, scalar values, publication/update timestamps and pagination metadata. Internal organization/project IDs, memberships, users, drafts and model internals are not included.

Pagination is bounded. Public CORS/cache headers are intentional because only explicitly public content reaches this route.

## Production migrations

Payload schema changes are committed as migrations under `apps/content/src/migrations/`.

The initial App Data migration was generated with Payload and validated on PostgreSQL 16. The CMS migration also starts from Payload-generated schema/snapshot output but includes an explicit ordering correction for upgrading existing App Data records safely.

During CMS upgrade:

- existing record lifecycle `status` values are preserved;
- the existing lifecycle enum is renamed for clarity;
- Payload receives a separate draft/published enum for `_status`;
- existing records are marked published when `_status` is introduced so previously visible app-data does not disappear;
- the versions table and public-content fields are added.

Migration validation covers a fresh PostgreSQL 16 database, schema isolation, repeated/idempotent production startup and a production Payload build.

## What CMS V1 is not

CMS V1 is the editorial/storage boundary, not the final editor experience. It does not yet include:

- page/component layout schemas;
- visual page builder;
- live site preview;
- media library/uploads/variants;
- forms/submissions;
- client approvals/portal;
- public relationship graph expansion.

Those features should build on the existing project/data/content boundaries instead of introducing new ownership models.
