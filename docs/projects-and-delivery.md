# Projects & Delivery

Argus organizes delivery work around Projects. A Project is useful on its own and never requires a client. Personal software, homelab infrastructure, experiments and client work all use the same core model.

## Project workspace

Projects support general metadata, tags/status, notes, tasks and milestones. Project activity is backed by domain events while technical/security accountability remains in the separate audit log.

A Project may have an optional client reference. Client-specific workflows are deliberately outside the core ownership model so personal projects remain first-class.

## GitHub repositories

Projects can link GitHub repositories and use repository/branch/commit context as deployment input. GitHub is an integration rather than the owner of Project identity; disconnecting a repository does not delete the Project.

The current integration focuses on repository context needed by Argus. Richer pull-request/check/release automation can be added later without changing the Project model.

## Service catalog

Services represent applications and infrastructure roles such as web applications, APIs, workers, databases, caches and custom services. A service is semantic: Docker is one possible runtime implementation, not the identity of the service itself.

This separation lets deployments, monitoring, dependencies and sites refer to a stable resource even if its runtime changes later.

## Environments

Projects can define named environments such as development, staging and production. Environments can be linked to services, repositories and deployment targets. Production is treated as a protected destination rather than a special hard-coded project type.

Environment configuration is intended to eventually reference managed secrets rather than duplicate raw secret values.

## Deployments and releases

The delivery chain is modeled roughly as:

```text
Repository / revision
  -> deployment
  -> environment
  -> services
  -> release history
```

Argus stores deployment/release state, source context, target environment, status and history. Readiness checks can block or warn before delivery when required project resources are not in an acceptable state.

The current model is a foundation for future rollback, approvals, blue/green or canary delivery. Those advanced strategies are not implied by a successful basic deployment record.

## Sites and domains

A Site belongs to a Project, not to a Client. It can link to a service, repository and environment and can have one or more domains.

Domain inventory supports hostname, primary-domain relationship, registrar/DNS metadata, routing mode and known expiration information. Cloudflare-related routing modes currently describe desired/inventory state; they do not imply that Argus already provisions Cloudflare resources.

## Domain lifecycle

Argus derives lifecycle state from known expiration metadata and recent exact-host TLS observations.

Expiration states:

- `UNKNOWN`
- `OK`
- `WARNING`
- `CRITICAL`
- `EXPIRED`

TLS states:

- `UNKNOWN`
- `VALID`
- `STALE`
- `FAILED`

Lifecycle evaluation runs periodically and may also be triggered manually. Material changes emit project events so normal notification rules can react. V1 does not renew registrations, change DNS or provision certificates.

## Dependency graph

Resources can declare dependencies, allowing Argus to represent relationships such as a website depending on an API, database and server. The graph is used as a shared foundation for impact reasoning, incidents, maintenance and future automated actions.

## Release readiness

Readiness provides a project-level preflight surface rather than assuming that a deployment is safe merely because a command can run. Checks can report ready, warning or blocked conditions based on implemented project/operational rules.

## Current delivery limitations

The following remain future work:

- provider-created servers/resources;
- automated DNS/Cloudflare provisioning;
- production approval workflows;
- blue/green and canary delivery;
- generalized deployment rollback;
- reusable project blueprints/templates;
- client portal and client-specific approval surfaces.

These are sequenced in [Roadmap](roadmap.md).
