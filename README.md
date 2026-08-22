# Argus

[![CI](https://github.com/Noah-Bozkurt/argus/actions/workflows/ci.yml/badge.svg)](https://github.com/Noah-Bozkurt/argus/actions/workflows/ci.yml)
[![Publish images](https://github.com/Noah-Bozkurt/argus/actions/workflows/publish-images.yml/badge.svg)](https://github.com/Noah-Bozkurt/argus/actions/workflows/publish-images.yml)

Argus is a self-hosted, project-first control plane for software work and the infrastructure behind it. It brings projects, servers, deployments, monitoring, incidents, background jobs, and content operations into one operator workspace.

> [!WARNING]
> Argus is under active development and is **not production-ready**. The currently supported control-plane deployment is a single Ubuntu/Debian amd64 host.

## Contents

- [Why Argus](#why-argus)
- [Capabilities](#capabilities)
- [Architecture](#architecture)
- [Installation](#installation)
- [Operating Argus](#operating-argus)
- [Development](#development)
- [Documentation](#documentation)
- [Project status](#project-status)
- [Contributing and security](#contributing-and-security)
- [License](#license)

## Why Argus

The name comes from **Argus Panoptes**, the many-eyed watchman from Greek mythology. Argus applies the same idea to software operations: keep the important parts of a project visible without reducing them to an unstructured collection of remote shell commands.

The product is deliberately project-first. A project may be personal, internal, open source, or client work; client data is optional metadata rather than the root of the system model.

## Capabilities

### Projects and delivery

- Project workspaces with repositories, environments, services, releases, tasks, milestones, and notes.
- Release readiness, sites, domains, dependencies, and delivery state.
- GitHub repository integration for issue, pull-request, and CI visibility.

### Infrastructure and operations

- Managed Linux servers with live host and service inventory.
- Typed Docker, Compose, system, and maintenance operations.
- Persisted jobs, monitoring, incidents, notifications, and public status pages.
- Transactional installation, repair, domain changes, updates, smoke verification, and rollback.

### Content and access

- Payload-backed project content and CMS workflows.
- First-party Argus login, shared operator/CMS sessions, and workspace roles.
- Separate Web, Control API, Worker, Content, Agent, and privileged Helper boundaries.

The operator UI provides global **Overview**, **Projects**, **Servers**, **Jobs**, and **Notifications** views. Project work is grouped into **Deploy**, **Infrastructure**, **Observe**, **Work**, and **Content**.

## Architecture

```text
                         public HTTPS
                              │
                         Caddy proxy
                      ┌───────┴────────┐
                      │                │
                  Web / UI       Content / CMS
                      │                │
                      └──────┬─────────┘
                             │ loopback/private network
                        Control API ───── Worker
                             │               │
                             └──── PostgreSQL┘
                             │ authenticated protocol
                       managed-node Agent
                             │ local Unix socket
                       privileged Helper
```

The control plane runs application services through Docker Compose. Agent and Helper run as native systemd services. PostgreSQL and the Control API are not exposed on public host interfaces. Host actions use typed protocol operations; the Helper is not a general-purpose network shell.

Deployable releases are coordinated sets of Web, Control API, Worker, Content, and Host Tools images carrying the same immutable Git revision. Moving tags such as `main` are discovery aliases, not installed-version identities.

See [`DESIGN.md`](DESIGN.md) for engineering principles and [`docs/architecture.md`](docs/architecture.md) for component and data-flow detail.

## Installation

The public installer is available at **https://install.noahbozkurt.nl**.

```bash
curl -fsSL https://install.noahbozkurt.nl/install | sudo bash
```

The bootstrap downloads the native installer, verifies its published checksum, and opens guided terminal setup. It can install a control plane or connect a managed server to an existing Argus instance.

### Current requirements

- Ubuntu or Debian on amd64.
- Root or sudo access.
- Public TCP ports 80 and 443 for a control-plane install.
- DNS for separate Argus and content hostnames.
- Outbound HTTPS access to GitHub and the public Argus packages on GHCR.

The installer uses public registry access and does not require a GitHub package token. Cloudflare Origin CA remains optional and, when selected, uses a separately scoped Cloudflare API token stored root-only.

Read [`docs/installation.md`](docs/installation.md) before installing on a non-disposable host.

## Operating Argus

Start with the high-level lifecycle commands:

```bash
argusctl status
argusctl doctor
argusctl logs
sudo argusctl repair
```

Common targeted operations include:

```bash
argusctl logs control-plane --tail 200
argusctl logs web -f
argusctl logs agent --since 1h
argusctl system info
argusctl version
sudo argusctl credentials
sudo argusctl update --version main
sudo argusctl uninstall
```

`argusctl update` resolves a requested moving tag to a full revision, downloads and validates the coordinated image set before mutation, creates rollback state, applies the target, and verifies health. Normal failures identify the phase and underlying cause; add `--verbose` for complete secret-safe diagnostics.

The lower-level `health`, `connection`, and `smoke` commands remain available for automation and deep diagnostics. See [`docs/operations.md`](docs/operations.md) for update, backup, recovery, and logging behavior.

## Development

Argus is a Rust and TypeScript monorepo.

```text
apps/
  web/          Next.js operator application
  content/      Payload/Next.js content service
  installer/    public installer portal
crates/
  agent/        unprivileged managed-node agent
  helper/       privileged local helper
  cli/          argusctl and native installer
  protocol/     shared Agent/Control/Helper types
  system/       host inventory and system helpers
services/
  control-api/  Rust/Axum API and SQL migrations
  worker/       persisted background-job worker
deploy/         Compose, Caddy, systemd, and image assets
docs/           product, architecture, operations, and development docs
scripts/        lifecycle, smoke, update, rollback, and validation scripts
```

### Tooling

- Stable Rust compatible with the workspace.
- Node.js 20+ and pnpm 9.
- PostgreSQL 16 for API, worker, migration, and Payload work.
- Docker with Buildx for image and lifecycle validation.
- Linux/systemd for Agent and Helper integration testing.

### Baseline checks

```bash
pnpm install --no-frozen-lockfile
cargo fmt --all -- --check
cargo test --workspace
pnpm --filter @argus/web exec tsc --noEmit
pnpm --filter @argus/content run typecheck
node --test apps/installer/test.mjs
bash -n install.sh scripts/*.sh
```

Some CI jobs require PostgreSQL, Docker, and production application builds. See [`docs/development.md`](docs/development.md) and [`CONTRIBUTING.md`](CONTRIBUTING.md) before opening a pull request.

## Documentation

The documentation index is [`docs/README.md`](docs/README.md). Key references:

- [Installation](docs/installation.md)
- [Using Argus](docs/usage.md)
- [Authentication](docs/authentication.md)
- [Architecture](docs/architecture.md)
- [Operations](docs/operations.md)
- [Security and recovery](docs/security-and-recovery.md)
- [Content platform](docs/content-platform.md)
- [Development](docs/development.md)
- [Roadmap](docs/roadmap.md)
- [Changelog](CHANGELOG.md)

## Project status

Current boundaries and known limitations include:

- A single-host control plane rather than HA.
- amd64-only supported installation.
- No stable release or semantic-version compatibility promise yet.
- Incomplete per-user identity propagation in some Control API audit paths.
- Ongoing work on provider provisioning, production secret management, and time-series observability.

The roadmap is maintained in [`docs/roadmap.md`](docs/roadmap.md). Release images are published only from a successful `main` revision after the complete immutable image set has been verified.

## Contributing and security

Read [`CONTRIBUTING.md`](CONTRIBUTING.md) before proposing a change. Coding agents should also follow [`AGENTS.md`](AGENTS.md).

Do **not** report vulnerabilities in public issues. Use the private reporting process in [`SECURITY.md`](SECURITY.md).

## License

Argus is free and open-source software licensed under the **GNU Affero General Public License v3.0 only**. If you modify Argus and make it available to users over a network, the AGPL requires you to offer those users the corresponding source code. See [`LICENSE`](LICENSE) and [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md).
