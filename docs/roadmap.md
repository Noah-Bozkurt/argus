# Roadmap

Argus is in active pre-production development. The repository already contains a broad working control-plane foundation, but the supported deployment model is intentionally narrow while installation, identity and recovery behavior are hardened.

## Current implemented foundation

The current `main` branch includes:

- a project-first control panel with global Overview, Projects, Servers, Jobs and Notifications views;
- project workspaces for repositories, environments, services, Compose stacks, releases, sites and domains;
- managed Linux Agent/Helper infrastructure control;
- server inventory, health collection and typed host operations;
- jobs, site monitoring, monitor schedules and incident automation;
- incidents, notifications, dependency information and public status pages;
- GitHub repository integration and synchronized repository/CI metadata;
- project tasks, milestones, notes, activity and audit/event foundations;
- backup/recovery and desired-state foundations for sensitive host changes;
- Payload-backed application data and CMS functionality;
- drafts, publication, pages/components, media, relationships and forms;
- an Argus-native Content workflow inside each project;
- first-party Argus login with Payload-backed shared sessions and `owner`, `admin`, `member` and `client` workspace roles;
- a static installer site and checksum-verifying bootstrap;
- guided control-plane and managed-node installation;
- `argusctl` diagnostics, smoke verification, registry login, update and uninstall flows;
- coordinated GHCR releases where a green `main` revision is published as a complete SHA-tagged image set and only then promoted to the `main` release pointers;
- immutable installed revisions even when `main` is used as the requested update target;
- a modern control-panel UI shell for desktop and responsive layouts.

Implemented does not mean production-ready. Several parts still need stronger product-level hardening and real-world validation.

## Current supported deployment target

The supported installation path currently assumes:

- Ubuntu or Debian;
- amd64;
- a single control-plane host;
- Docker Compose for control-plane services;
- native systemd Agent and Helper services;
- direct HTTP/HTTPS ingress through Caddy;
- public DNS for separate Argus and content domains;
- private GHCR access using a GitHub package credential.

Installation starts at [install.noahbozkurt.nl](https://install.noahbozkurt.nl) and is documented in [Installation](installation.md).

## Current limitations

The main reasons Argus should still be considered pre-production are:

- per-user identity is not yet forwarded through every Web -> Control API request, so control-plane audit attribution still has a bootstrap-user limitation;
- the deployment model is single-host rather than HA;
- amd64 is the only supported architecture;
- private package access is required during installation and updates;
- provider provisioning and Cloudflare Tunnel automation are not yet productized;
- broader disaster-recovery targets and production secrets management are incomplete;
- some workflows expose strong backend capability before their final UX is finished.

These limitations should be treated as product work, not solved by adding undocumented manual setup steps.

## Near-term priorities

### 1. Finish identity and access control

First-party login, shared sessions and workspace roles are implemented. The next identity work is to forward each authenticated operator's `argusUserId` through Web -> Control API calls so audit attribution is per-user, then improve account-management UX and add stronger authentication options such as 2FA, passkeys or SSO where they make sense.

### 2. Installation and release hardening

Keep `https://install.noahbozkurt.nl` as the stable product-facing installer entry point and improve the surrounding release lifecycle:

- clearer install/update diagnostics;
- stronger release/version presentation;
- safer credential handling and rotation UX;
- continued validation of reruns, reboots, failed updates and rollback paths;
- named/versioned releases when the lifecycle is mature enough to move beyond `main` as the discovery pointer.

### 3. Control-panel UX completion

Continue consolidating older feature panels into the newer control-panel design so all project, infrastructure, operations and content workflows feel like one product rather than independently added slices.

### 4. Recovery and secrets maturity

Expand the existing recovery foundation toward full disaster recovery and production secret handling:

- broader backup targets;
- restore/runbook UX;
- encrypted secret storage and rotation;
- clearer maintenance/recovery modes;
- stronger audit/security-event tooling.

### 5. Deployment and infrastructure breadth

Once the single-host lifecycle is dependable:

- add arm64 support;
- add optional Cloudflare Tunnel/proxy modes;
- introduce provider adapters and server provisioning;
- add reusable project/service templates;
- only add multi-node/rolling control-plane update semantics when Argus actually supports that topology.

### 6. Content platform maturity

Build on the current CMS/application-data foundation with richer editing and delivery UX, including more advanced field configuration, media workflows, previews, form workflows and site-template-aware content experiences.

## Product principles that remain fixed

- Projects are the core unit; Clients remain optional context.
- GitHub remains the source of truth for source code, pull requests and issues.
- Privileged host mutations use typed APIs and the Agent/Helper trust boundary rather than arbitrary remote shell execution.
- Recovery-sensitive changes need preflight, audit and rollback semantics.
- A green coordinated release should be promoted only from a revision that passed normal CI.
- Installed versions should resolve to immutable revisions.
- Documentation should describe current behavior and be changed together with the feature that changes it.
