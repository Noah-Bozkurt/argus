# Argus Documentation

This directory contains the canonical documentation for Argus. Older milestone-by-milestone notes have been consolidated so the docs describe the system as it exists now rather than preserving one file for every historical PR.

## Documents

- [Architecture](architecture.md) — system boundaries, runtime components, data ownership, events and command flow.
- [Projects & Delivery](projects-and-delivery.md) — projects, repositories, services, environments, deployments, sites, domains and release/readiness concepts.
- [Operations](operations.md) — server management, Docker/Compose, maintenance, monitoring, jobs, incidents, notifications and status pages.
- [Security & Recovery](security-and-recovery.md) — trust boundaries, privileged execution, firewall enforcement, desired state, backups and transactional restore.
- [Content Platform](content-platform.md) — Payload application data, project synchronization, CMS drafts/publication, migrations and public content access.
- [Development](development.md) — workspace layout, local development, checks, configuration and migration workflow.
- [Roadmap](roadmap.md) — what is implemented, the first-server-test gate and what remains planned.

## Documentation rule

New functionality should update the relevant canonical document instead of creating another `*-v1.md` milestone file. Add a new document only when a topic is genuinely large enough to remain independently useful.

The root [README](../README.md) is deliberately non-technical and should link here for implementation details.
