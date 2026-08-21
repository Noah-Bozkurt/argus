# Argus

Argus is a self-hosted control panel for software projects and the infrastructure behind them. It keeps project work, servers, deployments, monitoring, incidents and content in one place.

Argus is project-first. A project can be personal, internal, open source or client work; client data is optional and does not define the rest of the system.

> **Status:** Argus is under active development and is not production-ready. The supported control-plane install is currently a single Ubuntu/Debian amd64 host.

## Why Argus?

The name comes from **Argus Panoptes**, the many-eyed watchman from Greek mythology. The idea is simple: one place that keeps watch over the different parts of a project.

## What works today

- Project workspaces with repositories, environments, services, tasks, milestones and notes.
- Managed Linux servers with live health data and typed system, Docker/Compose and maintenance operations.
- Releases, readiness checks, sites and domains.
- Persisted jobs, monitoring, incidents, notifications, dependencies and public status pages.
- GitHub repository integration with repository, issue, pull-request and CI state.
- Payload-backed application data and CMS workflows scoped to projects.
- First-party Argus login with shared operator/CMS sessions and workspace roles.
- Host lifecycle tooling through `argusctl`, including health checks, updates, registry login and uninstall.

The operator UI has global **Overview**, **Projects**, **Servers**, **Jobs** and **Notifications** views. Project work is grouped into **Deploy**, **Infrastructure**, **Observe**, **Work** and **Content**.

## Install

The public installer is available at **https://install.noahbozkurt.nl**.

On a supported server:

```bash
curl -fsSL https://install.noahbozkurt.nl/install | sudo bash
```

The bootstrap downloads the native installer, verifies its checksum, and starts the guided setup. Before anything is installed, you can review the values with the arrow keys and press Enter on a row to change it. Passwords and tokens stay masked.

Current requirements:

- Ubuntu or Debian on amd64;
- root or sudo access;
- ports 80 and 443 available for a control-plane install;
- public DNS for the Argus and content domains;
- a GitHub classic PAT with `read:packages` access to the private Argus GHCR packages.

The installer can either set up a control plane or connect a managed server to an existing Argus instance. See [Installation](docs/installation.md) for the full flow.

For a control plane, the installer asks for a certificate contact email. It normally uses Let's Encrypt and automatically tries ZeroSSL if issuance fails. If both public hostnames are already behind Cloudflare, it asks whether to use public ACME or Cloudflare Origin CA. The Cloudflare option needs a scoped API token, stored root-only so Argus can repair a missing certificate later.

## `argusctl`

Useful commands on an installed host:

```bash
argusctl status
argusctl health
argusctl connection
argusctl doctor
argusctl system info
argusctl version
sudo argusctl smoke
sudo argusctl repair
sudo argusctl credentials
sudo argusctl update --version main
sudo argusctl registry-login
sudo argusctl uninstall
```

`argusctl update` resolves the requested release to an immutable Git revision instead of leaving an installation on a moving `main` tag.

Start with `argusctl doctor` when something does not look right. It checks the local services, containers, DNS, HTTPS and Agent connection in one pass and suggests a next step for each failure. `argusctl repair` restores damaged installation files without changing the installed version or deleting data. If `argusctl` itself is missing or broken, run the public installer again and choose **Repair**.

## Current limitations

Argus is deliberately narrow while the deployment and security model is hardened:

- the control plane is single-host rather than HA;
- amd64 is the only supported installation architecture;
- installation and updates currently require access to the private GHCR packages;
- per-user identity propagation into all Control API audit attribution is not complete yet;
- provider provisioning, production secrets management and full time-series observability are not finished.

The old reverse-proxy Basic Auth prompt is no longer used. Human authentication now uses Argus/Payload sessions; see [Authentication](docs/authentication.md) for the current model and its remaining limitations.

## Repository layout

```text
apps/
  web/          operator control panel
  content/      Payload content/application-data service
  installer/    static installer site
crates/         Rust agent, helper, CLI and shared libraries
services/       Control API and background worker
deploy/         Compose, Caddy, systemd and image assets
docs/           product, operations and development documentation
scripts/        lifecycle, update, recovery and validation tooling
install.sh      canonical host installer
```

## Documentation

Start with [docs/README.md](docs/README.md). The main references are:

- [Installation](docs/installation.md)
- [Using Argus](docs/usage.md)
- [Authentication](docs/authentication.md)
- [Architecture](docs/architecture.md)
- [Operations](docs/operations.md)
- [Security & recovery](docs/security-and-recovery.md)
- [Content platform](docs/content-platform.md)
- [Development](docs/development.md)
- [Roadmap](docs/roadmap.md)

## Releases

Release images are published only from a successful `main` revision. Argus publishes the coordinated image set under immutable SHA tags before promoting the moving `main` pointers used for update discovery.

## License

Argus is **proprietary, closed-source software**. The repository is private and the project is not licensed for public use, copying, modification or redistribution. See [LICENSE](LICENSE) and [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
