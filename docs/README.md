# Argus Documentation

This directory is the canonical documentation for the current Argus codebase.

Start with the user-facing guides, then use the deeper documents when working on a specific subsystem.

## Start here

- [Installation](installation.md) — install a control plane, connect managed servers, verify, update and uninstall.
- [Using Argus](usage.md) — current control-panel navigation and common project/infrastructure workflows.
- [Roadmap](roadmap.md) — current state, known limits and near-term priorities.

## Product and system reference

- [Architecture](architecture.md) — runtime components, trust boundaries, data ownership, events and command flow.
- [Projects & Delivery](projects-and-delivery.md) — projects, repositories, services, environments, deployments, sites and domains.
- [Operations](operations.md) — server management, Docker/Compose, maintenance, monitoring, jobs, incidents, notifications and status pages.
- [Security & Recovery](security-and-recovery.md) — privileged execution, firewall controls, desired state, backups and transactional recovery.
- [Content Platform](content-platform.md) — Payload-backed application data, CMS workflows, media, forms and public content access.
- [Development](development.md) — workspace layout, local development, checks, configuration and migrations.

## Current public installer

The supported installer entry point is:

```text
https://install.noahbozkurt.nl
```

The canonical bootstrap command is:

```bash
curl -fsSL https://install.noahbozkurt.nl/install | sudo bash
```

Do not document the underlying Cloudflare Pages hostname as the normal installation URL. `install.noahbozkurt.nl` is the product-facing endpoint.

## Documentation rules

Documentation should describe the repository as it exists now, not preserve old milestone plans as if they were current instructions.

When behavior changes:

1. update the relevant canonical document in the same change;
2. keep the root README focused on what Argus is and how to get started;
3. put detailed operator flows in `installation.md` or `usage.md`;
4. keep architecture/security documents implementation-focused;
5. remove or rewrite obsolete roadmap items instead of accumulating historical phases.

A feature that only exists on an unmerged branch should not be documented as available on `main`.
