# Deployments & Releases V1

Argus V1 records deployment attempts and multi-component releases before it executes deployments through providers. This establishes stable history, status semantics, audit events and references for later provider adapters.

## Deployments

A deployment belongs to:

- one Project;
- one Service Catalog service;
- one Environment;
- optionally one linked repository.

It records source commit/version, provider, status, actor, timestamps, URL, error summary, notes, previous successful deployment and optional rollback target.

### Status lifecycle

Allowed transitions are deliberately narrow:

- `QUEUED -> RUNNING | CANCELLED`
- `RUNNING -> SUCCEEDED | FAILED | CANCELLED`

Terminal records are not restarted or rewritten into new attempts. A retry creates a new Deployment record.

A rollback is also a new deployment attempt with `rollback_of_deployment_id`. When that rollback deployment succeeds, the old successful deployment is marked `ROLLED_BACK`; the rollback deployment remains `SUCCEEDED`.

## Referential safety

- Service and Environment must belong to the same project/organization.
- If the Service Catalog entry is already assigned to an environment, deployment to a different environment is rejected.
- A linked repository must belong to the same project.
- If a service already has a repository, a conflicting repository cannot be supplied.
- A rollback target must be a successful deployment of the same service and environment.
- Commit identifiers accept only 7-64 hexadecimal characters.
- Deployment URLs accept only absolute HTTP(S) URLs.

## Releases

A release groups separately versioned services. This supports releases such as:

```text
Argus 1.4.0
Web          deployment A
Control API  deployment B
Worker       deployment C
```

Release lifecycle:

- `DRAFT -> READY | FAILED`
- `READY -> RELEASED | FAILED`
- `RELEASED -> ROLLED_BACK`

Components may only be added while the release is `DRAFT`.

A release can become `READY` only when it has at least one component and every component references a `SUCCEEDED` deployment. The same readiness condition is checked again immediately before `RELEASED`, so a component that was rolled back after READY blocks release publication.

## Provider boundary

V1 accepts only provider `manual`. It records what happened but does not execute a deploy.

The next provider-execution iteration should introduce a `DeploymentProvider` interface with methods such as:

- deploy
- status
- rollback
- deployment URL / provider metadata

Cloudflare Pages, Vercel or another provider can then implement the interface without changing Deployment identity/history.

## Audit and Activity

Project-scoped events include:

- `deployment.created`
- `deployment.status_changed`
- `deployment.rolled_back`
- `release.created`
- `release.component_added`
- `release.status_changed`

Each mutation also writes a technical audit record.

## Non-goals

- executing provider deployments;
- storing complete CI/build logs;
- arbitrary deployment shell commands;
- editing immutable historical attempts;
- deleting successful deployment history;
- provider-specific configuration in the generic schema.

## Validation gate

The phase is mergeable only after the normal repository checks pass: Rust workspace tests, `cargo fmt --all -- --check`, and the web TypeScript check.
