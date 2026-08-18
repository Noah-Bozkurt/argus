# Jobs Administration V1

Jobs Administration makes the PostgreSQL-backed Argus worker observable without turning the queue into a second feature-configuration API.

## Scope

The workspace-level `/jobs` page is read-mostly. It shows:

- queued/running/dead counts;
- configured schedules;
- the latest 200 background jobs;
- attempts and maximum attempts;
- active worker lease metadata;
- bounded error code/message;
- project/workspace scope and resource key.

## Ownership boundary

Schedules remain configured by the feature that owns them.

Examples:

- Site Monitoring controls `site_monitor.check` enabled state and interval;
- Notifications owns its Organization materialization behavior.

Jobs Administration does not expose generic JSON payload editing or arbitrary schedule creation. This prevents `/jobs` from bypassing Site ownership, monitor configuration, event-rule, or other domain validation.

## Data minimization

The queue persists typed JSON payloads for execution, but the browser page intentionally does not render them. Operational inspection uses safe metadata such as job kind, Project, resource key, status, attempts and errors.

This avoids casually exposing IDs or future sensitive feature payload fields in an administrative dashboard.

## Dead-job retry

The only V1 mutation is an explicit retry of a `DEAD` job.

Retry:

- requires normal authenticated web identity;
- is Organization-scoped;
- is rejected unless the current state is exactly `DEAD`;
- preserves the existing job kind, Project, resource key and payload;
- resets attempts to zero;
- clears lease and previous error state;
- queues the same job immediately;
- records `background_job.retried` in the security audit log.

Retry does not create or modify a schedule and does not allow payload replacement.

## Running jobs

For troubleshooting, RUNNING jobs show lease owner and lease expiry. V1 deliberately has no force-unlock or cancel operation. A crashed worker is handled by the existing lease-expiry reclaim mechanism.

## History

The endpoint returns the latest 200 jobs for the Organization. V1 does not delete historical jobs or provide arbitrary retention controls.

## Non-goals

V1 does not implement:

- arbitrary job creation;
- payload editing;
- generic schedule editing;
- force-unlock/cancel of RUNNING jobs;
- bulk retry;
- automatic retry of DEAD jobs;
- queue metrics/time-series charts;
- retention/archival policies;
- worker autoscaling.

## Next phase

A conservative Monitoring-to-Incident Automation V1 can now use recurring Site checks while Jobs Administration provides visibility into failed scheduled executions. Automatic Incident creation should require repeated failures and must never automatically publish to a public Status Page.

## Validation gate

Merge only after Rust workspace tests, `cargo fmt --all -- --check`, and the web TypeScript check pass.
