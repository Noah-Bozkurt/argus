# Architecture

Argus is a project-centric control platform with explicit trust and data-ownership boundaries. The architecture is intentionally not client-centric: projects exist independently and may optionally reference a client later.

## Core principles

1. **Project is the primary organizational boundary.** Repositories, services, environments, deployments, sites, domains and application data belong to projects. A client relationship is optional.
2. **Privileged work is typed, not shell-driven.** Browser/API requests become typed commands and eventually reach a narrow local helper. Argus does not expose arbitrary remote shell execution as its normal control path.
3. **Desired and actual state are separate.** Observed server state is reported by agents; desired state is stored centrally and only supported fields may be reconciled automatically.
4. **Control data and application/content data have separate ownership.** The Rust control plane owns infrastructure/project operations. Payload owns application/content records in its own PostgreSQL schema.
5. **Background work uses the same domain boundaries as interactive work.** Monitoring, notifications, reconciliation and synchronization run through persisted jobs instead of hidden in-process timers.

## Runtime components

```text
Browser
  |
  v
Next.js Web (apps/web)
  |
  v
Rust Control API (services/control-api) ----> PostgreSQL / public control schema
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

### First-test deployment topology

The first supported server-test topology is intentionally hybrid:

```text
Internet
   |
   v
Caddy container :80/:443
   |---------------------> Web container
   |---------------------> Payload container
   `-- allowlisted Agent routes --> Control API container
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

Caddy, Web, Control API, Worker, Payload and PostgreSQL are orchestrated by Docker Compose. Agent and Helper remain native systemd services because their purpose is to observe and safely mutate the actual host. Putting the Helper behind a privileged container with broad host mounts would add a container boundary without reducing its necessary host privilege.

Only Caddy publishes public ports. The Control API additionally binds to host loopback for the local native Agent. PostgreSQL, Web and Payload are not published directly on host interfaces.

Argus-owned control-plane containers are labelled `com.argus.protected=true`. The privileged Helper refuses normal managed Docker/Compose start, stop or restart actions for containers carrying that label, preventing Argus from turning off its own control plane through its normal container-management surface.

### Web

`apps/web` is the operator-facing Next.js application. Backend credentials stay server-side. The browser does not receive the Control API service credential and does not directly contact privileged agents/helpers.

### Control API

`services/control-api` is the authoritative control-plane API. It owns organization/project authorization, server enrollment, command queuing, maintenance policy, project resources, operational state, audit events and domain events.

Control-plane SQL migrations live under `services/control-api/migrations/` and operate on the normal control schema.

### Jobs Worker

`services/worker` claims persisted jobs from PostgreSQL and calls internal Control API job handlers. Recurring work such as monitoring, notification materialization, desired-state reconciliation, domain lifecycle evaluation and Payload project synchronization uses this path.

### Agent

`crates/agent` runs on a managed Linux node. It establishes an outbound authenticated relationship with the Control API, sends heartbeats/snapshots, polls typed commands, calls the local helper and submits results.

Enrollment uses a short-lived single-use token. After enrollment the agent stores its long-lived device credential locally with restrictive permissions; the control plane stores a verifier rather than returning secrets through normal read APIs.

### Privileged Helper

`crates/helper` is the narrow root boundary. It listens only on a local Unix socket and executes fixed operation classes with validated inputs. Systemd, APT, Docker/Compose, firewall and recovery operations pass through this boundary. It does not expose a generic shell endpoint.

### Payload Content Service

`apps/content` is a separate Payload/Next.js service. It uses the same PostgreSQL instance but writes only to `argus_content` through the Payload Postgres adapter. This prevents Payload from taking ownership of control-plane tables.

The Control API remains the source of truth for Argus projects. A background sync mirrors project identity into Payload project spaces; Payload does not become a second project-management authority.

## Project/resource model

At a high level:

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
      -> Payload App Data / CMS Content
      -> optional Client context
```

Services are semantic resources rather than aliases for containers. A service may be implemented by a container, Compose service or future provider while retaining a stable project-level identity.

## Commands and capabilities

A command includes an ID, target server, typed command payload, expiry, idempotency key, risk level and status. Commands move through persisted states such as `QUEUED`, `ACCEPTED`, `RUNNING`, `SUCCEEDED`, `FAILED`, `UNKNOWN` and `EXPIRED`.

Agent capabilities are versioned independently from the agent binary. The Control API can therefore reject an unsupported operation before it reaches a helper.

The queue enforces organization/server ownership, expiry, idempotency and conflict groups. Disruptive operation classes can additionally require an active maintenance window.

## Events and audit

Mutations normally produce two different records:

- `audit_events` for security/technical accountability;
- `domain_events` for project activity, automation and notification inputs.

These are related but intentionally not the same concept. Incidents, lifecycle changes, deployments and other modules can emit project-scoped events without weakening the audit trail.

## Dependency and impact model

Argus stores dependency relationships between resources. This allows operational features to reason about impact across sites, services and infrastructure rather than showing isolated status values. The current implementation is foundational; richer propagated health and automated impact reasoning can build on it later.

## Current architectural boundaries

The first-test installer/deployment path is intentionally not yet a production-grade deployment system. Still not implemented as complete production capabilities:

- automatic upgrades and transactional rollback of the Argus control plane itself;
- release-channel management beyond the initial `main`/commit image tags;
- advanced identity such as OIDC/passkeys/mTLS;
- a general secrets manager;
- cloud/provider provisioning;
- Cloudflare/DNS mutation;
- arbitrary desired-state enforcement beyond explicitly safe operations;
- browser terminal as an escape hatch;
- full metrics/time-series observability.

See [Roadmap](roadmap.md) for planned sequencing.
