# Incidents V1

Incidents turn operational findings into a durable investigation record. They are project-owned and Client-independent.

## Source resource

Every Incident starts from one graph resource:

- SERVICE
- SITE
- DOMAIN
- SERVER
- ENVIRONMENT
- REPOSITORY

The selected resource is the incident root. Argus resolves the current Dependency Graph impact before the Incident is written.

## Historical blast-radius snapshot

Incident creation stores:

- root resource type, ID and name;
- every affected resource;
- graph distance;
- full impact path.

These values are copied into incident-owned tables. Later changes to Services, Sites, Domains or dependency edges therefore do not rewrite historical Incident impact. The graph is evaluated once for the create request and the resulting snapshot becomes incident-owned history.

This is intentionally different from showing the current graph every time an old Incident is opened.

## Severity

V1 severity is explicit:

- MINOR
- MAJOR
- CRITICAL

Severity does not automatically publish anything publicly.

## Lifecycle

Supported states:

- INVESTIGATING
- IDENTIFIED
- MONITORING
- RESOLVED

Allowed transitions:

- INVESTIGATING -> IDENTIFIED
- INVESTIGATING -> MONITORING
- IDENTIFIED -> INVESTIGATING
- IDENTIFIED -> MONITORING
- MONITORING -> INVESTIGATING
- MONITORING -> RESOLVED

A regression from MONITORING to INVESTIGATING is supported. RESOLVED is terminal in V1; reopening should later create an explicit continuation/new incident rather than silently rewriting a resolved historical record.

## Timeline

Incident timeline entries are immutable and ordered. V1 records:

- CREATED
- STATUS_CHANGED
- NOTE

Notes are investigation updates, mitigation observations or operator context. Existing timeline entries have no edit/delete API.

## Audit and activity

Mutations emit project events and audit records:

- `incident.created`
- `incident.status_changed`
- `incident.note_added`

## Public status boundary

V1 Incidents are internal operational records. Nothing is exposed on a public status page automatically.

A later Status Pages phase must require an explicit public-safe message/publication decision. Internal graph paths, server names, notes, user IDs and audit details must not leak into unauthenticated responses.

## Non-goals

V1 does not implement:

- automatic Incident creation from monitoring;
- public Incident publication;
- root-cause attribution;
- automatic postmortems;
- SLA calculations;
- paging/alert escalation;
- maintenance-window Incident generation.

## Next phase

Change Correlation should enrich Incident investigation by showing nearby Deployments, Releases, server Commands and project resource changes around the Incident start time. Correlation must be described as potentially relevant change, never as automatically proven cause.

## Validation gate

Merge only after Rust workspace tests, `cargo fmt --all -- --check`, and the web TypeScript check pass, with no temporary workflow files left on the branch.
