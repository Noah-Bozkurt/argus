# Content Platform

Argus uses Payload as its project-scoped application-data and CMS engine instead of implementing a second generic database/CMS framework in the Rust control plane. For author-facing instructions, see [Using the Argus CMS](cms.md).

## Service and data boundary

`apps/content` is a standalone Payload 3 / Next.js service. It shares the PostgreSQL instance with the Control API but owns the isolated `argus_content` schema. Rust control-plane tables remain owned by the Rust services.

The Argus Control API is the source of truth for Project identity. A background synchronization mirrors each Argus Project into a Payload `project-space`; Payload does not become a second project-management authority.

Production schema changes use committed Payload migrations. Development schema push is not a production migration strategy.

## Workspace identity and Project authorization

Payload `workspace-users` are the shared human identity provider for the Argus Web application and Payload CMS. Organization roles are `owner`, `admin`, `member` and `client`. Project content permissions are separate:

- `manager` — manage Project structure/memberships and destructive content lifecycle actions;
- `editor` — create and edit Project data/content, media and forms;
- `viewer` — read Project data/content, media, forms and submissions.

Organization owners/admins inherit access to Projects in their organization. Other users require an explicit Project membership.

The native Argus Content UI never receives the internal Content service credential. Server actions resolve the current `payload-token`, fetch the authenticated workspace user and forward that user's organization, Argus user ID and Payload workspace-user ID together with the server-side machine credential.

Every privileged native Content route then re-resolves the workspace user inside Payload and checks the required Project role. `overrideAccess` is used only after that explicit human + organization + Project authorization boundary has passed. A caller cannot gain native CMS access merely by presenting a valid machine token plus arbitrary identity headers.

This same authorization layer protects the shared App Data handler, CMS, media, forms and private form CSV exports.

## Models

`data-models` are dynamic Project-scoped schemas. A model has:

- stable immutable slug;
- immutable kind (`data` or `content`);
- immutable content role (`collection`, `page` or `component` for content models);
- active/archived lifecycle;
- up to 50 typed fields;
- a schema version.

Supported fields are short text, long text, number, boolean, date/date-time, JSON, relationship and media.

The native schema editor supports adding, removing and reordering fields. Relationship targets must be models in the same Project. Page models can allow only component schemas from the same Project.

`schemaVersion` increases only when the actual schema shape changes (field definitions or a Page's allowed Component set). Display-name, description, public-read and lifecycle-only edits do not create a false schema revision.

Models can be archived/restored. Permanent deletion requires `manager` access and is rejected while records still exist.

## Data versus editorial content

Models share one substrate but have different semantics:

- `kind=data` — application data, direct-write and always-published semantics;
- `kind=content` — editorial drafts/publication.

The internal App Data route re-exports the same scoped model/record implementation using the `data` kind. It does not maintain a parallel authorization or persistence implementation.

## Records, drafts and publication

`data-records` validate values against their model. Project/model ownership is immutable. A record has two separate state dimensions:

- Payload editorial `_status=draft|published`;
- Argus lifecycle `status=active|archived`.

Content records retain up to 50 Payload versions. Application-data records are forced published.

The native editor supports create/edit, draft save, publish, archive/restore and permanent delete. Archived records are not delivered publicly. Permanent deletion requires `manager` access.

Record values and the complete relationship edge set are written in one Payload/PostgreSQL transaction. A failed relationship replacement therefore rolls back the record change instead of leaving partial relationship state. Permanent record deletion removes incoming/outgoing relationship edges and the record in one transaction as well.

## Relationships

`data-relations` store explicit record-to-record edges instead of hiding foreign references in arbitrary JSON. Validation enforces:

- same Project for source and target;
- declared relationship field on the source model;
- declared target model;
- required/cardinality rules;
- active target record.

Public relationship expansion is opt-in with `?expand=relationships`, limited to one bounded level, and only returns independently public/published/active targets. Draft, private, archived and cross-Project targets are omitted.

Component-block relationship fields are still not stored as `data-relations`; blocks should use scalar/media configuration and site renderers can query related public collections independently.

## Pages, Components and the visual editor

Content models have three roles:

- `collection` — repeatable standalone entries;
- `component` — reusable embedded page-block schema, no standalone records;
- `page` — normal fields plus an ordered list of allowed component blocks.

A Page layout is bounded to 100 blocks. Every block has a stable UUID, component slug and values validated against the component's current schema. Unknown, archived or disallowed components are rejected by the backend.

The native visual editor is a structured canvas rather than a free-form HTML designer. It includes:

- a Project component palette;
- drag-and-drop block ordering;
- desktop/tablet/mobile canvas widths;
- structured live previews using authored text/media;
- a typed field inspector;
- duplicate/remove actions.

The website maps component slugs such as `hero` or `cta` to real frontend components. Argus therefore owns authored layout/content while the repository remains the source of truth for rendering, CSS and application behavior. Stored content is not executable HTML.

## Native workspace loading

The internal CMS workspace returns models plus a paginated record set. Records are loaded 100 per page and relations are scoped to the loaded records. This removes the previous silent 500-record workspace truncation and keeps large Projects bounded.

Public content delivery has independent pagination and is not coupled to the native authoring workspace page size.

## Public content API

Content is private by default. Public delivery requires:

- active mirrored Project;
- `kind=content`;
- active model;
- `publicRead=true`;
- active record;
- published Payload version.

Endpoint:

```text
GET /public/projects/:argusProjectId/content/:modelSlug
```

Pagination is bounded to 100 records per request. Public responses expose only content-facing values/layout, publication timestamps and pagination metadata. Internal organizations, memberships, users and drafts are not returned.

CORS is intentionally permissive for this explicitly public read surface. A public website must never receive the internal Content synchronization credential.

## Media library

Each Project has a Payload-backed image library. Accepted formats are JPEG, PNG, WebP and AVIF up to 10 MiB. Alternative text is required; captions are optional. Payload generates bounded thumbnail, medium and large variants without enlarging the source.

Native media permissions are:

- viewer: list;
- editor: upload/edit metadata and public visibility;
- manager: permanent delete.

Media ownership is immutable. Content records and component blocks can reference only media in the same Project. Public content resolves media metadata only for assets currently marked public; private/deleted assets do not leak through stale UUID references.

Production media bytes live in the persistent `content_media` volume. This is persistence, not a complete media backup strategy.

## Forms and submissions

Forms are Project-owned definitions with a stable slug, success message and `draft|published|archived` lifecycle. Supported fields are short text, email, long text, number, boolean/consent and bounded select choices.

Public endpoints:

```text
GET  /public/projects/:argusProjectId/forms/:formSlug
POST /public/projects/:argusProjectId/forms/:formSlug
```

Only active Projects and published forms resolve publicly. Public POST validates body size, known fields, required fields, types and choices. A honeypot and durable PostgreSQL-backed rate limit protect submissions without storing raw source addresses.

Submission values have no anonymous read endpoint. Native permissions are viewer for private reads/exports, editor for triage/form lifecycle, and manager for permanent submission deletion.

CSV export is capped at 10,000 rows, preserves form field order, prevents spreadsheet-formula injection, disables caching/sniffing and excludes rate/source fingerprints.

## Site integration

Sites consume public content using the Project UUID and model slug. Static-site generators such as Astro can fetch during build; SSR sites can fetch on each request or according to their own cache policy.

Argus does not currently trigger arbitrary external site rebuilds when a record is published. A static deployment therefore needs its normal rebuild/deploy mechanism before newly published content appears.

See [Using the Argus CMS](cms.md) for a concrete integration pattern and a migration design based on `Noah-Bozkurt/youpspace.com`.

## Migrations and runtime validation

Payload schema changes live under `apps/content/src/migrations/`. CI validates migrations against PostgreSQL 16 and starts a real Content runtime.

The Content runtime suite covers CMS drafts/publication, relationships, page/component layouts, media, forms and first-server acceptance. Runtime CI bootstraps a real owner account so the same human/Project authorization checks used in production are active during acceptance tests.

Contract tests additionally cover internal Project role ranking and organization isolation.

## Current limits

The Content platform is still pre-production. Notable limits include:

- visual previews are structured approximations; they do not iframe/render arbitrary external site templates inside Argus;
- no autosave for partially invalid dynamic records yet;
- no client approval workflow;
- no recursive relationship graph expansion;
- no automatic external-site rebuild hook on publish;
- no general rich portable long-form field beyond current typed values/JSON/long text.

These limits should extend the existing Project/content ownership model rather than introduce parallel CMS authority.
