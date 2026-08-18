# Notifications & Event Rules V1

Notifications V1 materializes selected project `domain_events` into a persistent in-app inbox. It is workspace-level functionality and remains independent of Clients.

## Architecture

Argus already emits project domain events for Incidents, Deployments, Sites, Domains, Services, Runbooks and other modules. Notifications does not introduce a second competing event bus.

Instead:

```text
domain_events
  -> enabled notification rules
  -> materializer
  -> deduplicated notifications
  -> per-user read / acknowledgement state
```

## Execution model

V1 deliberately does not pretend that a background worker already exists.

The Notifications page exposes `Refresh from events`, which runs the materializer explicitly. A later jobs/worker process can invoke the same materialization logic periodically without changing the rule or inbox data model.

The V1 scan looks back seven days and considers at most 5,000 recent project events per refresh. Deduplication prevents repeated refreshes from creating duplicate notifications.

## Rules

A rule contains:

- optional Project scope; null means all Projects in the Organization;
- human-readable name;
- event pattern;
- optional JSON data field + expected scalar value;
- notification severity: INFO / WARNING / CRITICAL;
- enabled flag.

Rules have no delete API in V1. Disable an obsolete rule instead, preserving the provenance of already-created notifications.

## Event patterns

Patterns are either exact:

```text
incident.created
```

or a suffix wildcard:

```text
incident.*
```

Leading or middle wildcards are rejected. This keeps matching predictable and cheap.

## Optional data filter

Rules may require one scalar JSON field to equal one expected value. Dot notation supports up to four levels.

Examples:

```text
site.check.completed
field: status
value: DOWN
```

```text
deployment.status_changed
field: to
value: FAILED
```

String comparisons are case-insensitive. Boolean and numeric scalar values are supported. Arrays/objects are not matched in V1.

Both field and value must be supplied together.

## Materialization and deduplication

Each persisted notification records:

- Organization;
- Project;
- matching rule;
- source event ID and event type;
- concise title/message;
- severity;
- original event timestamp.

`(rule_id, source_event_id)` is unique, so processing the same event repeatedly is idempotent.

The materializer joins events to Projects through the established project-event convention (`domain_events.resource_id = project_id`). Events that do not resolve to a Project are not turned into V1 notifications.

## Data minimization

Notifications do not copy arbitrary event payloads into the inbox. The displayed summary is derived from event type, Project name and at most one safe scalar detail such as name, hostname, version, status or target status.

Raw command output, logs, secrets and complete event JSON are not copied into notification records.

## User state

Notifications are Organization-wide in V1, but read/acknowledgement state is per user:

- unread/read;
- unacknowledged/acknowledged.

One user reading or acknowledging an item therefore does not clear it for colleagues.

Acknowledging also marks the item read for that user.

## Audit boundary

Rule creation/update and explicit materializer runs write security audit records, but Notifications does not emit new `domain_events` for its own bookkeeping. This prevents notification rules from recursively generating notifications about notification processing.

Read/ack state is lightweight per-user inbox state and is not emitted into the project Activity stream.

## Example useful rules

- `incident.created` -> CRITICAL/WARNING depending on team preference;
- `incident.*` -> WARNING;
- `site.check.completed` + `status=DOWN` -> CRITICAL;
- `deployment.status_changed` + `to=FAILED` -> CRITICAL;
- `backup.*` -> WARNING when future backup events are standardized;
- `domain.*` -> INFO for teams that want domain-change visibility.

V1 does not seed these automatically because different projects have different noise tolerances.

## Non-goals

V1 does not implement:

- automatic background polling;
- e-mail delivery;
- Slack/Discord delivery;
- webhooks;
- SMS/paging;
- per-user rule recipients/subscriptions;
- complex boolean filter expressions;
- escalation policies;
- automatic remediation.

## Next phase

A small jobs/worker foundation can later schedule Notification materialization and Site Monitoring without changing their current semantics. Alternatively, launch checklists can build on Readiness when project workflow value is prioritized over background execution.

## Validation gate

Merge only after Rust workspace tests, `cargo fmt --all -- --check`, and the web TypeScript check pass, with no temporary workflow files left on the branch.
