# Architecture

Argus is a project-centric control platform with explicit trust and data-ownership boundaries. Projects exist independently and may optionally reference a client; client context is not required for infrastructure, delivery, operations or content.

## Core principles

1. **Project is the primary organizational boundary.** Repositories, services, environments, deployments, sites, domains, managed infrastructure and application/content data belong to projects.
2. **Privileged work is typed, not shell-driven.** Browser/API requests become typed commands and eventually reach a narrow local helper. Arbitrary remote shell execution is not the normal management path.
3. **Desired and actual state are separate.** Agents report observed state. The control plane stores desired state and only explicitly supported fields may be reconciled automatically.
4. **Control data and content/application data have separate ownership.** The Rust control plane owns operational/project state. Payload owns content/application records in its own PostgreSQL schema.
5. **Background work uses the same domain boundaries as interactive work.** Monitoring, notifications, reconciliation and synchronization run through persisted jobs instead of hidden timers in web processes.
6. **Installed releases are immutable.** `main` can be used to discover a release, but an installation resolves it to a full Git revision and persists that immutable revision.

## Runtime components

```text
Browser
  |
  v
Next.js Web (apps/web)
  |
  v
Rust Control API (services/control-api) ----> PostgreSQL / control schema
  |                         |
  |                         +----> Jobs Worker (services/worker)
  |
  +---- typed command queue <---- authenticated Agent (crates/agent)
                                      |
                                      v
                               local Unix socket
                                      |
                                      v
                              privileged Helper

Payload Content Service (apps/content) ----> same PostgreSQL database
                                             isolated schema: argus_content
```

## Current deployment topology

The supported control-plane topology is currently a single Linux host:

```text
Internet
   |
   v
Caddy container :80/:443
   |---------------------> Web container
   |---------------------> Payload container
   `-- allowlisted routes --> Control API container
                                      |
                     +----------------+----------------+
                     |                                 |
                 Worker container                 PostgreSQL

Linux host
   |
   +-- argus-agent.service (unprivileged)
   |       |
   |       `-- /run/argus/helper.sock
   |
   `-- argus-helper.service (root, typed operations only)
```

Caddy, Web, Control API, Worker, Payload and PostgreSQL are orchestrated with Docker Compose. Agent and Helper remain native systemd services because they observe and safely mutate the actual host.

Only Caddy publishes public ports. The Control API also binds to host loopback for the local Agent. PostgreSQL, Web and Payload are not exposed directly on host interfaces.

Argus-owned control-plane containers are labelled `com.argus.protected=true`. The Helper rejects normal managed Docker/Compose stop/restart actions for protected containers so Argus cannot disable its own control plane through the regular management surface.

The current installer entry point is `https://install.noahbozkurt.nl`; see [Installation](installation.md).

## Web

`apps/web` is the operator-facing Next.js control panel. Backend credentials stay server-side. The browser does not receive the Control API service credential and cannot directly contact the Agent/Helper boundary.

The current global shell exposes Overview, Projects, Servers, Jobs and Notifications. Project workspaces are grouped into Deploy, Infrastructure, Observe, Work and Content.

Human authentication is first-party. The Web app and Payload CMS use the Payload `workspace-users` auth collection and share an HTTP-only session cookie across the configured Argus domains. Workspace roles are `owner`, `admin`, `member` and `client`; client accounts are denied access to the operator control panel. See [Authentication](authentication.md).

Web still authenticates its server-to-server Control API calls with `ARGUS_WEB_API_TOKEN`. Per-request forwarding of the authenticated operator's `argusUserId` into Control API audit attribution is not complete yet; the current client still uses the installation's bootstrap `ARGUS_USER_ID` for that attribution.

## Control API

`services/control-api` is the authoritative operational API. It owns organization/project authorization, server enrollment, command queuing, maintenance policy, project resources, operational state, audit events and domain events.

Control-plane migrations live under `services/control-api/migrations/` and operate on the normal control schema.

## Jobs Worker

`services/worker` claims persisted jobs from PostgreSQL and calls internal Control API handlers. Recurring work such as monitoring, notification materialization, desired-state reconciliation, domain lifecycle evaluation and Payload project synchronization uses this path.

## Agent

`crates/agent` runs on managed Linux nodes. It maintains an outbound authenticated relationship with the Control API, sends heartbeats/snapshots, polls typed commands, calls the local Helper and submits results.

Enrollment uses a short-lived single-use token. After enrollment the Agent stores its device credential locally with restrictive permissions and the one-time enrollment token is removed from the persistent environment.

## Privileged Helper

`crates/helper` is the narrow root boundary. It listens only on a local Unix socket and executes fixed operation classes with validated inputs. Systemd, APT, Docker/Compose, firewall and recovery operations pass through this boundary. It does not expose a generic shell endpoint.

## `argusctl`

`crates/cli` is installed as `/usr/local/bin/argusctl` and provides local diagnostics and lifecycle operations:

```text
status
health
connection
smoke
system info
version
update
registry-login
uninstall
```

Control-plane self-update is executed by a host-side transactional updater. Owners may authorize
the same typed operation from the Web UI after password re-authentication; the Agent schedules a
delayed systemd unit so the acknowledgement is persisted before Web/API restart. The updater keeps
running independently, writes a bounded host log, verifies health and rolls back on failure. The
local `argusctl update` and recovery commands remain available when the UI is unavailable.

## Payload Content Service

`apps/content` is a separate Payload/Next.js service. It uses the same PostgreSQL instance but owns the isolated `argus_content` schema through the Payload Postgres adapter.

The Control API remains the source of truth for Argus projects. A background synchronization mirrors project identity into Payload project spaces; Payload is not a second project-management authority.

## Project/resource model

```text
Organization
  -> Project
      -> Repositories
      -> Services
      -> Environments
      -> Deployments / Releases
      -> Servers / Compose stacks
      -> Sites / Domains
      -> Monitoring / Incidents / Status
      -> Tasks / Milestones / Notes
      -> Payload App Data / CMS Content
      -> optional Client context
```

Services are semantic resources rather than aliases for containers. A service may be implemented by a container, Compose service or future provider while keeping a stable project-level identity.

## Commands and capabilities

A managed command carries a target server, typed payload, expiry, idempotency key, risk level and persisted status. Commands move through states such as `QUEUED`, `ACCEPTED`, `RUNNING`, `SUCCEEDED`, `FAILED`, `UNKNOWN` and `EXPIRED`.

Agent capabilities are versioned independently from the binary so unsupported operations can be rejected before reaching the Helper.

The queue enforces ownership, expiry, idempotency and conflict groups. Disruptive operation classes can additionally require an active maintenance window.

## Events and audit

Mutations generally produce two different record types:

- `audit_events` for security/technical accountability;
- `domain_events` for project activity, automation and notification inputs.

These are intentionally separate concepts. Operational modules can emit project-scoped domain events without weakening the audit trail.

## Releases and update architecture

A successful `main` CI revision produces a coordinated set of five SHA-tagged GHCR artifacts: Web, Control API, Worker, Content and host tools. The release workflow verifies the complete immutable set before promoting the moving `main` pointers.

During install/update, Argus resolves a requested alias such as `main` to the image's full `org.opencontainers.image.revision` and persists that SHA. Compose therefore runs a coordinated immutable revision rather than silently following moving tags.

`argusctl update` pre-pulls and validates the target set, preserves rollback files, takes a PostgreSQL snapshot at the appropriate transaction boundary and runs post-update health/smoke verification. Interrupted-update recovery is tied into the native Helper startup path. Details and limits are documented in [Security & Recovery](security-and-recovery.md).

## Current architectural limits

The architecture is intentionally not claiming production completeness. Important current limits include:

- single-host control plane rather than HA/multi-node operation;
- amd64-only supported installation;
- per-user operator identity is not yet forwarded through every Web -> Control API request and audit path;
- no general production secrets manager yet;
- no provider provisioning or Cloudflare/DNS mutation layer yet;
- desired-state enforcement remains limited to operations with explicit safety semantics;
- no full metrics/time-series observability platform;
- no general browser terminal as the normal control path;
- update rollback is a bounded transactional safety mechanism, not arbitrary point-in-time restore after successful updates.

See [Roadmap](roadmap.md) for current priorities.
