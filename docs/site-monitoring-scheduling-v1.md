# Site Monitoring Scheduling V1

Site Monitoring scheduling is the first project-scoped feature built on the Argus Jobs / Worker foundation.

## Principle

Scheduling changes **when** a check runs, not **how** it runs.

Both manual and scheduled checks use the existing `SiteMonitoringStore::run_check` path. This preserves the existing:

- Site/domain ownership validation;
- HTTP/HTTPS-only target rules;
- port 80/443 restriction;
- DNS resolution before request;
- private, loopback, link-local, CGNAT, benchmark and documentation IP blocking;
- pinned validated IP request behavior;
- disabled redirects;
- TLS/HTTP/robots/sitemap signals;
- immutable check history;
- Site health updates and project events.

The worker therefore never implements its own network probe.

## Schedule model

A configured Site may have one `site_monitor.check` schedule in the shared `job_schedules` table.

The schedule stores:

- Organization and Project;
- Site UUID as `resource_key`;
- typed payload containing `site_id` and the user who configured the schedule;
- enabled state;
- interval between 60 seconds and 24 hours;
- next-run and last-enqueued timestamps;
- standard worker retry policy.

A Site without a monitor configuration cannot enable a schedule.

## Attribution

V1 records the user who last configured the schedule in the job payload. Scheduled executions use that identity when calling the existing monitoring operation, so check history and audit records keep a concrete operator provenance instead of inventing an unauthenticated browser user.

Updating a schedule updates that actor to the user making the change.

## Execution

The background worker materializes a due schedule into a deduplicated `background_jobs` row, claims it under a lease, and sends the typed job to the Control API.

The Control API accepts `site_monitor.check` only when:

- the job has a Project scope;
- `site_id` is a valid UUID;
- `actor_user_id` is a valid UUID;
- the Site still belongs to the Organization/Project;
- an active monitor configuration still exists.

If the Site or configuration disappears, the job fails through the normal bounded retry/dead-job policy rather than falling back to an arbitrary target.

## UI

The Project workspace shows a Monitoring schedules section with:

- active Sites;
- whether Site Monitoring is configured;
- enabled/manual-only state;
- interval;
- next scheduled run;
- last enqueue timestamp.

Scheduling is opt-in per Site. Existing monitors remain manual-only until explicitly enabled.

## Non-goals

V1 does not implement:

- cron expressions;
- sub-minute checks;
- external monitoring regions;
- automatic Incident creation;
- public Status Page publication;
- synthetic browser journeys;
- catch-up storms after worker downtime;
- a second monitoring engine in the worker.

## Next phase

The next useful operations slice is either automatic Incident thresholds from repeated monitoring failures, or a Jobs administration view for queued/running/dead jobs. Incident thresholds should remain conservative so one transient failed check does not automatically create a public-facing incident.

## Validation gate

Merge only after Rust workspace tests, `cargo fmt --all -- --check`, and the web TypeScript check pass.
