# Service Catalog V1

The Service Catalog models project-owned application components. It is deliberately separate from the host-level systemd service inventory reported by agents.

Examples:

- Web application
- API
- Worker
- Database
- Queue
- Cron/background service
- Other project component

A service belongs to a Project and never requires a Client.

## Model

The existing foundation `services` table is evolved rather than replaced with a competing catalog table. V1 adds:

- description
- optional linked repository
- optional environment
- optional server
- runtime/technology label
- optional owner user
- optional HTTP(S) endpoint
- lifecycle status (`ACTIVE`, `PAUSED`, `ARCHIVED`)
- updated timestamp

The existing `status` column is treated as the current health summary. Newly created catalog services start at `UNKNOWN`; later monitoring/deployment phases can derive `HEALTHY`, `DEGRADED` or `UNHEALTHY` instead of asking users to manually claim health.

## Referential safety

Every write is scoped by organization and project.

- Repository links must belong to the same project.
- Environments must belong to the same project.
- Servers must belong to the same project.
- If a server is selected, its environment becomes the service environment; an explicitly supplied different environment is rejected.
- Owner users must belong to the same organization.
- Endpoints accept only absolute HTTP or HTTPS URLs.

## UI scope

V1 lets a project create, edit, archive/pause and delete catalog services and link them to an existing project repository.

Environment/server assignment is represented in the backend model but intentionally remains read-only in this UI phase. The next Environments phase will add explicit project environment management and safe assignment controls rather than exposing raw UUID inputs.

## Activity and audit

Create, update and delete operations write both:

- technical audit events;
- project-scoped domain events (`service.created`, `service.updated`, `service.deleted`).

This allows Service Catalog changes to feed Project Activity and later change-correlation features.

## Future consumers

The Service Catalog becomes the stable project identity layer for:

- environments;
- deployments and releases;
- dependency graph edges;
- health/status pages;
- incidents and impact analysis;
- ownership/runbooks;
- cost attribution.

Those modules should reference catalog service IDs instead of inventing separate component identities.
