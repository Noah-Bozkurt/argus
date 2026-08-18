# Environments V1

Environments are project-owned deployment contexts. They exist for personal and client projects alike and never require a Client.

## Types

V1 supports:

- `development`
- `preview`
- `staging`
- `production`
- `custom`

The type gives Argus a stable semantic meaning while the display name remains flexible. A project can therefore use names such as `Local`, `PR Preview`, `Acceptance`, `Production EU`, or `QA 2`.

## Protection

Production environments are always protected. Other environment types may be marked protected manually.

Protected environments cannot be deleted. V1 protection is intentionally simple; later deployment/release phases will use it for stricter approval, rollback and maintenance requirements.

## Referential safety

An environment cannot be deleted while referenced by:

- a managed server;
- a Service Catalog service.

Service Catalog assignments validate that environment, server and project match. Selecting a server derives that server's environment; supplying a conflicting environment is rejected.

## Project UI

The project workspace can:

- create environments;
- edit name/type/description/protection;
- see server/service usage counts;
- delete only unprotected, unused environments;
- assign Service Catalog services to project environments and servers.

## Ordering

Default semantic ordering is:

1. Development
2. Preview
3. Staging
4. Production
5. Custom

This is stored as `sort_order` so later custom ordering can be added without changing environment identity.

## Activity and audit

Environment create/update/delete operations write both audit events and project-scoped domain events:

- `environment.created`
- `environment.updated`
- `environment.deleted`

## Validation gate

The phase is only mergeable after the normal repository checks pass: Rust workspace tests, `cargo fmt --all -- --check`, and the web TypeScript check.

## Next phase

Deployments and releases can now target a stable Service + Environment pair. The next phase should record immutable deployment attempts, source commit/version, actor, provider, status, duration and rollback relationship rather than storing "current deployment" directly on the environment.
