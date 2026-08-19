# Payload / App Data Architecture V1

Argus uses Payload as a dedicated application-data and later CMS service. Payload does not replace the Rust Control API and does not own infrastructure, deployment, incident, server or project-control tables.

## Process boundary

- `apps/web`: existing Argus operator UI.
- `services/control-api`: authoritative operational/control-plane domain.
- `apps/content`: Payload 3 service for project-scoped application data and later CMS data.
- PostgreSQL may be the same database server/database, but Payload uses a separate schema, `argus_content` by default.

Payload's Postgres adapter is configured with `schemaName`. It must never be pointed at the `public` schema used by the Rust Control API.

## Project ownership

The Rust Control Plane remains authoritative for Organization, Project and optional Client identity.

Payload stores a `project-spaces` mirror containing:

- Argus Project UUID;
- Argus Organization UUID;
- display name;
- optional Client UUID;
- active/paused/archived state.

`clientId` is optional. Personal projects use exactly the same app-data substrate as client projects.

A server-only `POST /internal/argus/project-sync` endpoint performs idempotent project mirror upserts. It requires a dedicated `ARGUS_CONTENT_SYNC_TOKEN` and rejects attempts to move an existing Argus Project UUID between organizations.

This endpoint is an integration boundary, not a browser API. Eventual project-event driven synchronization can be added on top without changing Payload's data ownership model.

## Authentication and tenant isolation

Payload has a `workspace-users` authentication collection.

- The first account may bootstrap only while no Payload users exist and becomes an Organization admin.
- After bootstrap, only Organization admins may create users.
- Organization identity is immutable after user creation.
- Non-admin users cannot change their role or Argus user link.
- Organization admins can only see/mutate users in their own organization.

This is intentionally separate from final Argus SSO. A later identity phase can replace login duplication while keeping the collection-level access rules.

## Project roles

`project-memberships` grants:

- `manager`: manage project membership and destructive app-data operations;
- `editor`: create/edit models and records;
- `viewer`: read project app data.

Project and user endpoints of a membership are immutable after creation, preventing an update authorized in Project A from moving the membership to Project B.

Organization admins implicitly have manager-equivalent access to project app data in their organization.

## Dynamic data models

`data-models` describe project-scoped schemas rather than creating arbitrary PostgreSQL tables at runtime.

Supported V1 field kinds:

- text;
- long text;
- number;
- boolean;
- date;
- date/time;
- JSON;
- relationship.

Model and field keys are stable lowercase identifiers. Relationship target models must belong to the same Project. Model schema versions increment on edits.

Models have a `kind` of either `data` or `content`. The distinction is metadata in V1, allowing the later Visual CMS to build on the same substrate instead of creating a parallel CMS database.

## Records

`data-records` store scalar values in a JSON object but that object is not arbitrary:

- unknown keys are rejected;
- values are checked against the model field type;
- required scalar fields are enforced;
- relationship fields are rejected from JSON and must use `data-relations`;
- archived models reject record writes;
- project and model identity are immutable after record creation;
- the model schema version used by the latest successful write is stored on the record.

This gives flexible app data without handing applications a raw schemaless tenant-wide JSON bucket.

## Relationships

`data-relations` stores explicit record-to-record edges.

Every edge verifies:

- source and target records are in the same Project;
- the source Model actually declares the named relationship field;
- the target record uses the Model declared by that field;
- duplicate targets are rejected;
- one-to-one fields accept at most one target.

Cross-project relationships are rejected.

## Database lifecycle

Development may explicitly set:

```text
PAYLOAD_DB_PUSH=true
```

Production must leave push disabled and use Payload migrations. Migration commands live in `apps/content/package.json`:

- `migrate:create`
- `migrate`
- `migrate:status`
- `generate:db-schema`

The initial production migration should be generated and committed against the finalized V1 Payload config before deploying the content service to a production database.

## Secrets

Required secrets/configuration:

- `DATABASE_URL`
- `PAYLOAD_SECRET` (minimum 32 characters)
- `ARGUS_CONTENT_DB_SCHEMA`
- `ARGUS_CONTENT_SYNC_TOKEN` (server-only, minimum 32 characters)
- `PAYLOAD_PUBLIC_URL`

Neither Payload secrets nor the project sync credential may be exposed to browser code.

## What V1 deliberately does not do

- no visual page builder yet;
- no media library yet;
- no forms yet;
- no client portal;
- no automatic DNS/Cloudflare behavior;
- no arbitrary runtime PostgreSQL table creation;
- no cross-project relationships;
- no ownership of Control API tables;
- no claim that Payload authentication is final Argus SSO.

The next product layer is the CMS/Visual CMS built on these models, records and project boundaries.
