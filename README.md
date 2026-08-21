# Argus

Argus is a self-hosted control plane for projects, servers, deployments, monitoring and content.

It is **project-first** rather than client-first: a project can be personal, experimental, open-source, infrastructure-only or connected to a client. Client-specific functionality is optional and does not sit at the center of the data model.

> **Status:** Argus is under active development and is not production-ready yet. The current deployment path targets a single Ubuntu/Debian amd64 server and still uses a temporary browser-authentication layer around the operator UI.

## What Argus does

Argus brings the operational parts of a project into one control panel:

- **Projects** — repositories, environments, services, tasks, milestones, notes and activity.
- **Infrastructure** — managed Linux servers, health information and typed operational actions.
- **Deployments** — Compose stacks, releases, readiness checks, sites and domains.
- **Operations** — jobs, monitoring, incidents, notifications, dependency information and status pages.
- **GitHub** — link repositories and surface repository, issue, pull-request and CI state inside a project.
- **Content** — project-scoped application data and CMS workflows backed by Payload.
- **Lifecycle** — install, verify, update, diagnose and uninstall with `argusctl`.

The web interface is organized as a modern control panel with global **Overview**, **Projects**, **Servers**, **Jobs** and **Notifications** views. Individual projects are split into **Deploy**, **Infrastructure**, **Observe**, **Work** and **Content** sections.

## Quick start

The supported install path is the Argus installer site:

**https://install.noahbozkurt.nl**

On a clean supported server, run:

```bash
curl -fsSL https://install.noahbozkurt.nl/install | sudo bash
```

The bootstrap downloads `install.sh`, downloads its SHA-256 checksum, verifies the installer and only then executes it.

### Current host requirements

- Ubuntu or Debian
- amd64
- root/sudo access
- Docker-compatible host; the installer installs Docker when needed
- ports 80 and 443 available for the control plane
- public DNS for the Argus and content domains
- a GitHub classic PAT with `read:packages` and access to the private Argus GHCR packages

The installer guides you through either:

1. **Control plane** — install a complete Argus instance on this server.
2. **Managed server** — connect another server to an existing Argus control plane using a short-lived setup code generated from the project UI.

See [Installation](docs/installation.md) for the complete flow.

## Using Argus

After installation, open the domain configured during setup and use the control panel.

A typical workflow is:

1. create or open a project;
2. link its GitHub repository;
3. create environments and services;
4. add managed servers when needed;
5. configure Compose stacks, releases, sites and domains;
6. add monitoring and incident automation;
7. use the project Content area for CMS/application data when the project needs it.

For a tour of the UI and common workflows, see [Using Argus](docs/usage.md).

## `argusctl`

The installer places `argusctl` on the host for diagnostics and lifecycle operations.

```bash
argusctl status
argusctl health
argusctl connection
sudo argusctl smoke
argusctl system info
argusctl version
```

Updates and registry credentials:

```bash
sudo argusctl update --version main
sudo argusctl registry-login
```

Uninstall:

```bash
sudo argusctl uninstall
```

Use `sudo argusctl uninstall --purge-data` only when the retained Argus state should also be deleted.

## Repository layout

```text
apps/
  web/          Argus operator control panel
  content/      Payload-based content service
  installer/    static installer site
crates/         shared Rust libraries, agent/helper and argusctl
services/       backend services such as the Control API and worker
deploy/         Compose, systemd and container build assets
docs/           product, operations and development documentation
scripts/        lifecycle, update, recovery and validation tooling
install.sh      canonical host installer
```

## Documentation

Start with the [documentation index](docs/README.md).

- [Installation](docs/installation.md)
- [Using Argus](docs/usage.md)
- [Architecture](docs/architecture.md)
- [Projects & delivery](docs/projects-and-delivery.md)
- [Operations](docs/operations.md)
- [Security & recovery](docs/security-and-recovery.md)
- [Content platform](docs/content-platform.md)
- [Development](docs/development.md)
- [Roadmap](docs/roadmap.md)

## Releases and images

Normal CI runs before release images are published. A successful `main` revision produces a coordinated set of SHA-tagged Argus images in GHCR, verifies that complete immutable set, and only then promotes the `main` release pointers.

An installed control plane records the resolved immutable Git commit rather than silently following a moving `main` tag. `main` is a discovery/update target, not the persisted installed version.

## License

Argus is **proprietary, closed-source software**. The repository is private and the project is not licensed for public use, copying, modification or redistribution. See [LICENSE](LICENSE) and [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
