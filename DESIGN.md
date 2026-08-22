# Argus Design

This document records the stable product and engineering principles behind Argus. Implementation topology belongs in [`docs/architecture.md`](docs/architecture.md); operational detail belongs in [`docs/operations.md`](docs/operations.md).

## Product intent

Argus is a self-hosted, project-first control plane for software work and the infrastructure that runs it. Projects are the organizing boundary; clients are optional metadata rather than the root of the model. The product should make normal operations clear without hiding system state or failure detail.

## Design principles

### Project-first, typed resources

Repositories, environments, services, releases, tasks, incidents, content, and servers are explicit resources with stable identities. Prefer typed operations and auditable state transitions over generic command execution or unstructured blobs.

### Safe by default

Sensitive capabilities require narrow authorization and explicit intent. Secrets remain server-side, logs are redacted, PostgreSQL and the Control API stay off public host interfaces, and the privileged Helper is reachable only through its local Unix-socket boundary.

### Privilege separation

The Agent is unprivileged and communicates with the root Helper through a constrained protocol. Do not turn either component into a general-purpose remote shell. New host capabilities must define validation, authorization, timeout, audit, and failure behavior.

### Transactional lifecycle operations

Install, repair, restore, domain changes, and updates must fail safely. Validate dependencies and target artifacts before mutation, snapshot recoverable state, apply changes in a bounded order, verify health, and preserve the original error if rollback is needed.

### Immutable coordinated releases

A deployable revision is a complete set of images and host tools bearing the same full Git SHA. Moving tags are discovery aliases only. Hosts persist and report the immutable revision they actually run.

### Durable background work

Long-running and scheduled work belongs in persisted jobs with explicit ownership, retry, timeout, and outcome state. Web requests should not become hidden job runners.

### Honest interfaces

Normal UI and CLI output should be concise, but never replace an actionable underlying cause with a generic failure. Verbose diagnostics must remain secret-safe. Partial, stale, unavailable, and failed states should be represented explicitly.

### Deliberate schema evolution

PostgreSQL and Payload schema changes use reviewed, committed migrations. Favor additive and backward-compatible changes; destructive changes require a documented migration, rollback, and rollout strategy.

### Simple implementation

Use existing local patterns before adding abstractions or dependencies. A feature is complete when its core behavior, failure paths, tests, operator documentation, and recovery implications are addressed.

## Current boundaries

The supported deployment is a single Ubuntu/Debian amd64 control-plane host plus managed Linux nodes. Argus is not yet HA or production-ready. Current limitations are documented in [`README.md`](README.md) and [`docs/roadmap.md`](docs/roadmap.md), and should not be obscured by aspirational design.

## Decision records

Material decisions that change these principles should update this file and the canonical subsystem document in the same pull request. If the decision needs historical alternatives and consequences, add a focused ADR under `docs/` only after establishing a consistent ADR convention.
