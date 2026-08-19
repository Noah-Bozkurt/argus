# Payload Project Sync V1

Argus Control Plane remains authoritative for Project identity. Payload receives an eventually consistent project mirror used to scope App Data and later CMS content.

## Reconciliation model

Every Organization receives one `content.projects.sync` schedule through the existing PostgreSQL Jobs / Worker layer.

- interval: 300 seconds;
- organization scoped;
- existing Organizations are backfilled by migration `0021_content_project_sync.sql`;
- newly created Organizations receive the schedule from a database trigger;
- Payload availability never sits on the synchronous Project creation path.

Each run loads the current Project inventory from the Rust Control API database and sends an idempotent project upsert to Payload's server-only `/internal/argus/project-sync` endpoint.

This also backfills Projects that existed before Payload was enabled.

## Mirrored fields

- Argus Organization UUID;
- Argus Project UUID;
- Project name;
- optional Client UUID;
- active / paused / archived lifecycle.

The Client link remains optional. Personal Projects are mirrored exactly like client Projects.

## Configuration

Control API configuration is optional:

- `ARGUS_CONTENT_URL`, for example `http://argus-content:3001`;
- `ARGUS_CONTENT_SYNC_TOKEN`, the same dedicated server-to-server credential configured on `apps/content`.

Both variables must be configured together. The token must contain at least 32 characters.

If neither is configured, the scheduled job succeeds as `CONTENT_SYNC_DISABLED`. This lets the Control Plane operate without Payload. A partial or invalid configuration fails the job so the existing worker retry/dead-job behavior makes the problem visible.

## Request safety

- content URL must be an absolute HTTP(S) URL;
- redirects are disabled;
- each request has a 10-second timeout;
- the sync token is sent only as a Bearer credential and is never logged;
- response bodies are not stored;
- V1 caps one Organization reconciliation at 200 Projects.

A failed project upsert fails the run rather than silently treating the mirror as current. The next worker retry starts reconciliation again; Payload's endpoint is idempotent by Argus Project UUID.

## Ownership boundary

Payload cannot change the authoritative Argus Project or Client relationship through this path. Its endpoint refuses to move an existing Argus Project UUID between Organizations.

Payload remains the owner of project-scoped App Data records/models; the Rust Control Plane remains the owner of operational Project identity.

## Future evolution

A later optimization may enqueue an immediate per-Project sync job from `project.created` / future project metadata change events. The periodic reconciliation remains useful as a repair/backfill mechanism and avoids coupling normal project mutations to content-service uptime.
