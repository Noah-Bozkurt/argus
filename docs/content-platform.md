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

The native Content workspace exposes relationship target/cardinality settings and project-local record pickers. Relationship writes are validated before persistence: the field must belong to the source model, every target must use the declared target model in the same Project, required/cardinality rules are enforced, and immutable edges are replaced as a set when a record is saved.

Public reads do not expand relationships by default. Callers may explicitly request one bounded level with `?expand=relationships`. Expansion is capped at 100 edges per response and includes a target only when its Project, model and record are active, the target model permits public reads, and the target record is independently published. Draft, private, archived and cross-project targets are omitted rather than leaked; recursive expansion is not supported.

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

## Argus-native content workflow

The Project workspace links to an Argus-native Content screen. Operators can create project-scoped content types with typed scalar fields, create and edit records, save drafts, and publish without navigating Payload collections or copying internal IDs. Public visibility remains an explicit content-type setting; draft records are never returned by the public endpoint.

Every saved record has an operator preview inside the protected Argus Web interface. The preview renders stored values as text/structured data through React rather than injecting stored HTML, so draft content cannot introduce executable markup. Published records on public content types also link to their real public read endpoint; drafts never receive a public link.

Content schemas have an explicit role:

- `collection` stores repeatable standalone entries;
- `component` defines a reusable block schema and cannot create standalone records or be public by itself;
- `page` stores page fields plus an ordered layout of explicitly allowed component schemas.

The native page editor can add, edit, reorder and remove typed blocks. Every block has a stable UUID, component slug and values validated against the current component schema. Page schemas can only allow active component schemas from the same Project, layouts are bounded to 100 blocks, and unknown/disallowed blocks fail the write. The protected preview renders draft page fields and blocks; published public page responses include the same validated layout without exposing Payload relationship IDs or model internals.

The Web server talks to Payload through `/internal/argus/cms/projects/:projectId`. This route is not a public browser API. It requires the high-entropy internal content token plus Argus organization and user headers, and resolves the mirrored Project by both Argus Project UUID and organization before every query or mutation. Payload access checks are bypassed only after that explicit server-to-server scope check; normal Payload user endpoints retain their existing membership rules.

The native editor supports scalar fields (text, long text, number, boolean, date/date-time and JSON) in collection records, page fields and component blocks. Collection/page schemas can also declare single- or multi-value relationships and authors select targets from active records in the same Project. Component-block relationship fields remain a future richer-editor extension.

## Media library

Each Project has an Argus-native image library backed by Payload uploads. Operators can upload JPEG, PNG, WebP and AVIF images up to 10 MiB with required alternative text and an optional caption. Payload records the original dimensions and generates bounded 320 px thumbnail, 960 px medium and 1920 px large variants without enlarging the source.

Media ownership is immutable and scoped to the mirrored Project and organization. The internal list/upload/update/delete API repeats that scope check before using privileged Payload local operations. Anonymous file delivery is denied unless the individual asset has `publicRead=true` and its Project is active; changing either condition revokes delivery immediately. Private and public assets can coexist in the same Project. Caddy exposes only Payload's checked `/api/media/file/*` delivery handler on the public content hostname, not the Payload admin or generic collection API.

Production Compose stores originals and generated variants in the named `content_media` volume mounted at `/app/media`, so replacing the immutable content container does not discard uploads. Deleting an asset removes its original and generated files as well as its metadata. This persistence is not yet a media backup: full volume backup/restore remains part of the broader disaster-recovery roadmap.

## Forms and submissions

Forms are Project-owned Payload definitions with a stable public slug, draft/published/archived lifecycle, success message and up to 30 typed fields. Supported fields are short text, email, long text, number, boolean/consent and bounded select choices. The Argus-native Content workspace creates and publishes forms, shows their public endpoint, paginates private submissions and lets operators triage submissions as new, reviewed, spam or archived. An explicitly confirmed scoped deletion permanently removes a submission when retention is no longer appropriate.

```text
GET  /public/projects/:argusProjectId/forms/:formSlug
POST /public/projects/:argusProjectId/forms/:formSlug
```

Only active Projects and published forms resolve publicly. The GET response exposes the renderable schema without Payload IDs. POST accepts a JSON object containing `values`; unknown keys, missing required values, invalid email/type/choice values and bodies above 64 KiB are rejected. CORS allows credential-free use from a separately hosted Project site. The optional `_company` honeypot receives a non-distinguishing accepted response without creating a submission.

Submission values are never exposed by a public read endpoint. Argus derives an HMAC source fingerprint from the proxy-provided address and Payload secret; raw source addresses are not stored. A PostgreSQL-backed fixed ten-minute window permits ten submissions per form/source. Unique durable rate slots prevent concurrent requests from racing past the cap, and rate state therefore survives application restarts. Operational database failures return a retryable service-unavailable response rather than being mislabeled as throttling.

Operators can download a private CSV for one form through the Argus Web surface. The Web route keeps the Content integration credential server-side, while Content revalidates Organization, Project and form ownership. Exports use the form's stable field order, are capped at 10,000 submissions, disable caching/sniffing, and prefix spreadsheet-formula-like cells before RFC-style CSV quoting. Source/rate fingerprints are never included.

## Production migrations

Payload schema changes are committed as migrations under `apps/content/src/migrations/`.

The initial App Data migration was generated with Payload and validated on PostgreSQL 16. The CMS migration also starts from Payload-generated schema/snapshot output but includes an explicit ordering correction for upgrading existing App Data records safely.

During CMS upgrade:

- existing record lifecycle `status` values are preserved;
- the existing lifecycle enum is renamed for clarity;
- Payload receives a separate draft/published enum for `_status`;
- existing records are marked published when `_status` is introduced so previously visible app-data does not disappear;
- the versions table and public-content fields are added.

The page/component migration is additive: existing content models default to `collection`, existing current/versioned records receive an empty layout, and the page allowlist uses a Payload-managed self-relationship table.

The media migration adds a separate project-scoped upload collection and Payload lock relationship. Media bytes remain outside PostgreSQL in the persistent media volume.

The forms migration adds definitions, normalized field/options rows and private submission records. Its unique rate-key index is part of the abuse-control correctness boundary.

Migration validation covers a fresh PostgreSQL 16 database, schema isolation, repeated/idempotent production startup and a production Payload build.

## What CMS V1 is not

CMS V1 now includes a basic Argus-native model, draft/publication workflow and typed block editor. It does not yet include:

- rich drag-and-drop or site-template-aware visual design (the current editor uses explicit add/move/remove controls and a safe generic preview);
- client approvals/portal;
- recursive or arbitrarily deep public relationship graph expansion.

Those features should build on the existing project/data/content boundaries instead of introducing new ownership models.
