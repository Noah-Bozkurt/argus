# Development

Argus is a Rust + TypeScript monorepo. This document describes the current developer workflow; it is not the planned end-user/server installer.

## Workspace layout

```text
apps/
  web/                 Next.js operator UI
  content/             Payload/Next.js application-data and CMS service
services/
  control-api/         Rust/Axum control-plane API + SQL migrations
  worker/              persisted background-job worker
crates/
  agent/               managed-node agent
  helper/              privileged local helper
  protocol/            typed Agent/Control/Helper protocol
  system/              system inventory helpers
  cli/                 argusctl diagnostics CLI
  common/              shared Rust utilities
packages/              shared TypeScript packages
```

Cargo and pnpm have separate workspaces but share one repository and CI pipeline.

## Required local tooling

Current development expects:

- a stable Rust toolchain compatible with the workspace;
- Node.js 20+;
- pnpm 9;
- PostgreSQL for persistent Control API/Payload work;
- Linux for actually exercising Agent/Helper system operations.

Some application/UI work can be developed on other operating systems, but privileged Helper behavior targets Linux utilities and systemd.

## Install dependencies

```bash
pnpm install --no-frozen-lockfile
cargo fetch
```

CI installs pnpm dependencies with `--frozen-lockfile`; dependency changes must update and commit `pnpm-lock.yaml`. Rust validation likewise uses the committed `Cargo.lock`.

## Standard checks

Run before merging code changes:

```bash
cargo test --workspace
cargo fmt --all -- --check
pnpm --filter @argus/web exec tsc --noEmit
pnpm --filter @argus/content run typecheck
```

Payload schema changes should additionally pass a production migration/runtime test against a clean PostgreSQL database and a production content-app build.

## Control API

The Control API requires PostgreSQL and a private web-backend token.

Typical variables:

```text
DATABASE_URL=postgres://...
ARGUS_WEB_API_TOKEN=<high-entropy secret>
ARGUS_CONTROL_API_BIND=0.0.0.0:8080   # optional
ARGUS_WORKER_TOKEN=<internal worker secret>
```

Run:

```bash
cargo run -p control-api
```

Control-plane migrations under `services/control-api/migrations/` are applied by the application startup path.

## Worker

The worker uses the same database and the internal Control API worker credential. It claims persisted jobs rather than relying on in-process web timers.

Run the worker through its Cargo package when testing recurring monitoring/reconciliation/synchronization flows.

## Web

`apps/web` talks to the Control API from the server side. Development configuration includes:

```text
ARGUS_CONTROL_API_URL=http://localhost:8080
ARGUS_WEB_API_TOKEN=<same backend token expected by Control API>
ARGUS_ORG_ID=<development organization UUID>
ARGUS_USER_ID=<development user UUID>
```

These are backend/server variables. Do not expose the Control API credential through public browser environment variables.

### Generated Control API contract

Core operator-facing Control API routes publish an OpenAPI document from Rust types. The running Control API exposes the same generated contract at `GET /openapi.json`. Regenerate the committed browser contract after changing a documented route or schema:

```bash
pnpm generate:api
```

This writes `apps/web/openapi/control-api.json` and `apps/web/lib/generated/control-api.ts`. The generated TypeScript file is owned by this command and should not be edited by hand. The server fleet is the first UI flow using the generated schema together with TanStack Query and TanStack Table; new migrations should follow the same contract-first pattern instead of introducing duplicate handwritten DTOs.

## Payload content service

`apps/content` owns application/content data in the isolated PostgreSQL schema `argus_content`.

Typical variables are documented in `apps/content/.env.example` and include:

```text
DATABASE_URL=postgres://...
ARGUS_CONTENT_DB_SCHEMA=argus_content
PAYLOAD_SECRET=<high-entropy secret>
PAYLOAD_PUBLIC_URL=http://localhost:3001
ARGUS_CONTENT_SYNC_TOKEN=<server-only project sync secret>
PAYLOAD_DB_PUSH=false
```

Development-only schema push may be explicitly enabled when experimenting locally, but production must use committed migrations.

Useful commands:

```bash
pnpm --filter @argus/content run dev
pnpm --filter @argus/content run typecheck
pnpm --filter @argus/content run generate:types
pnpm --filter @argus/content run migrate:create <name>
pnpm --filter @argus/content run migrate:status
pnpm --filter @argus/content run build
```

### Payload migration rule

When the Payload schema changes:

1. generate the migration from the previous committed snapshot;
2. inspect generated SQL, especially for renames/type changes;
3. do not accept interactive rename defaults without confirming data preservation;
4. apply the full migration chain to clean PostgreSQL 16;
5. verify Payload stays inside `argus_content`;
6. run the migration path again to verify idempotent startup;
7. build the production content application;
8. commit the migration, snapshot/index and generated Payload types.

Generated migrations are a starting point, not an excuse to merge destructive or incorrectly ordered SQL.

## Agent and Helper

First enrollment uses the Control API-generated enrollment token plus server/control-plane information. The Agent stores its resulting device credential in its local configuration.

The Helper is intended to run as root with a local Unix socket; the Agent should be unprivileged and be granted only the group/socket access necessary to call the Helper.

Do not run the Helper as a network service or add generic shell execution to simplify development.

Docker resource operations in the Helper use Bollard and the local Docker Engine API for typed container listing, inspection, stats, logs, protection-label checks, and start/stop/restart. Docker Compose remains the orchestration boundary for stack-level operations; do not reimplement Compose semantics in Bollard.

## CLI

Local diagnostic commands are available through `argusctl` (`crates/cli`). They are an operator/developer diagnostic surface, not a replacement for normal UI/API workflows.

Lifecycle image downloads use Bollard against the local Docker Engine API so the installer and concise updater can report Docker layer byte progress without parsing CLI text. Docker Compose remains the owner of Compose configuration and service orchestration (`config`, `up`, `down`, and related stack operations); do not replace transactional update or rollback semantics with direct container calls just to avoid Compose.

## Branch and PR workflow

Feature work should use a branch and PR. Required CI must be green before merge. Temporary migration/cleanup workflows should remove themselves and must not land on `main` unless they are intentionally permanent CI.

New features should update one of the canonical documents in `docs/` instead of adding another phase-specific markdown file.

## Installer and lifecycle boundary

The supported installer is published at `https://install.noahbozkurt.nl`. It installs or repairs the native Agent, Helper, CLI, systemd units, and coordinated Compose deployment. Lifecycle changes must preserve preflight validation, immutable revision resolution, snapshots, smoke checks, and rollback behavior.

For installer and update development, see [Installation](installation.md), [Operations](operations.md), and [Security & Recovery](security-and-recovery.md).
