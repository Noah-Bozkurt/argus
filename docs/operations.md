# Operations

This document describes the operational capabilities that currently exist in Argus. Security-sensitive details and recovery guarantees are expanded in [Security & Recovery](security-and-recovery.md).

## Managed server model

A managed server is enrolled with an Argus Agent. Heartbeats provide system identity and snapshots used for online/offline state, CPU/RAM/disk/load/uptime information and capability-specific inventories.

The Agent polls the Control API for typed commands and calls the local privileged Helper. A server is not considered controllable merely because a hostname exists in inventory; it must have an authenticated Agent with the required capability.

## Services and systemd

Managed systemd services support typed start, stop and restart actions. Service names are validated and must be present in the Helper allowlist. The Helper calls `systemctl` directly and does not construct shell command strings.

Recent journald output can be collected for managed services. Diagnostics also include failed systemd units and listening TCP ports.

## Package updates and maintenance

APT-based hosts expose update inventory and reboot-required state.

Typed operations include:

- refresh package metadata;
- install security updates through the host's unattended-upgrades configuration;
- install available upgrades;
- request a reboot.

Disruptive update/reboot operations require an active maintenance window at the Control API, not only a disabled/enabled button in the UI. Maintenance windows are persisted with reason and timing and can be ended early.

A command being accepted is not by itself proof that a reboot completed; reconnect/uptime correlation is the stronger operational signal.

## Docker

The Agent reports Docker container inventory when Docker is available. Typed container start, stop and restart actions use validated container references and the local Helper.

Argus does not expose arbitrary `docker` CLI arguments through the command API.

## Compose stacks

Compose projects are represented as first-class stack resources rather than opaque shell commands. Argus discovers/configures stack identity and provides typed start, stop and restart operations.

The Helper resolves known Compose configuration files from Docker's own Compose project inventory before invoking actions. Project names and discovered paths are validated. Arbitrary uploaded Compose execution is not treated as equivalent to an approved stack.

## Monitoring

Site monitoring records operational checks such as DNS/HTTP reachability, response latency, TLS state and selected website metadata. Monitoring includes SSRF protections rather than blindly requesting arbitrary internal targets.

Checks are persisted and scheduled through the Jobs/Worker subsystem. Historical checks drive site state, incident automation and domain TLS lifecycle observations.

## Monitoring schedules and background jobs

Recurring operations are persisted in PostgreSQL job schedules. Workers claim due jobs, execute typed internal handlers and record attempts/results. This is used for tasks including:

- site monitoring;
- incident evaluation;
- notification materialization;
- desired-state reconciliation;
- domain lifecycle evaluation;
- Payload project synchronization.

Jobs have bounded retries and are designed to be idempotent or deduplicated at their mutation boundary. The Jobs administration surface exposes job status rather than hiding recurring work inside long-lived web processes.

## Incidents

Incidents are project-scoped operational records with severity, status, affected resources, timeline and resolution context. Site-monitoring automation can create/evaluate incidents after configured failure conditions rather than treating every individual failed check as a new incident.

The dependency graph and change-correlation data provide context for understanding what might be affected and what changed near the start of an incident.

## Change correlation

Argus records operational/domain events from deployments and other mutations. Change correlation combines relevant recent changes with incident/operational context so an operator can inspect plausible causes without manually searching every subsystem.

Correlation is evidence/context, not automatic root-cause proof.

## Notifications

Notification rules match domain events and materialize operator notifications through background jobs. Notifications can be read and acknowledged in Argus.

Because modules emit normal project-scoped events, features such as domain lifecycle can use the same notification system instead of implementing their own unrelated alert engine.

External channels such as email/chat/webhook/push are future extensions unless explicitly implemented by a provider later.

## Status pages

Projects can expose status-page information from selected operational components and incidents. Status pages are a presentation layer over Argus operational state; they do not bypass incident or monitoring ownership.

## Desired state and drift

Argus stores desired security state separately from actual Agent observations and reports drift for configured fields.

Current automatic reconciliation is deliberately narrow. Firewall `must be active` is the supported enforcement primitive because it has a rollback-safe implementation. Other observed policy fields remain monitor-only until equivalent preflight/rollback guarantees exist.

## Operational non-goals

Argus currently does not claim to provide:

- arbitrary remote shell/terminal as its normal management model;
- full metrics/time-series observability;
- generalized configuration management for every Linux setting;
- automatic provider provisioning;
- automatic DNS/Cloudflare mutation;
- completely autonomous remediation for arbitrary incidents.

The first real server test is intentionally blocked on the installer described in [Roadmap](roadmap.md).
