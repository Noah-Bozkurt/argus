# Argus documentation

These are the canonical docs for the current Argus codebase. They should describe what is on `main`, including its current limitations, rather than preserve old implementation milestones as current instructions.

## Start here

- [Installation](installation.md) — install a control plane, connect managed servers, verify an installation, update and uninstall.
- [Using Argus](usage.md) — navigation and the main project/infrastructure workflows.
- [Using the CMS](cms.md) — create Project content types, compose visual pages, publish records, use media/forms and integrate a website such as Astro.
- [Authentication](authentication.md) — login, shared sessions, workspace roles and the current identity boundary.
- [Roadmap](roadmap.md) — implemented scope, known limits and near-term work.

## Reference

- [Architecture](architecture.md) — runtime components, trust boundaries, data ownership, events and command flow.
- [Projects & Delivery](projects-and-delivery.md) — projects, repositories, services, environments, deployments, sites and domains.
- [Operations](operations.md) — server management, Docker/Compose, maintenance, monitoring, jobs, incidents, notifications and status pages.
- [Security & Recovery](security-and-recovery.md) — privileged execution, firewall controls, desired state, backups and transactional recovery.
- [Content Platform](content-platform.md) — Payload-backed application data, CMS internals, media, forms and public content access.
- [Development](development.md) — workspace layout, local development, checks, configuration and migrations.

## Installer

The product-facing installer URL is:

```text
https://install.noahbozkurt.nl
```

The normal bootstrap command is:

```bash
curl -fsSL https://install.noahbozkurt.nl/install | sudo bash
```

The Cloudflare Pages hostname behind the installer is an implementation detail and should not be used in user-facing instructions.

## Keeping the docs current

When behavior changes, update the relevant canonical document in the same change. Prefer changing an existing document over adding another milestone-specific file, and do not document functionality from an unmerged branch as if it already exists on `main`.
