# Monitoring → Incident Automation V1

Site Incident Automation turns repeated Site Monitoring failures into an internal Incident while preserving Argus's explicit public/private boundary.

## Default

Automation is disabled for every Site until an operator explicitly enables a policy.

A Site must already have a Site Monitoring configuration before automation can be enabled.

## Failure threshold

A policy contains:

- enabled state;
- consecutive failure threshold from 2 through 10 checks;
- Incident severity: MINOR, MAJOR or CRITICAL;
- the user who last configured the policy;
- the currently linked automatic Incident, when one exists.

Only `DOWN` and `ERROR` checks count as failures.

`DEGRADED` does not count and cannot automatically create an Incident in V1.

The default threshold is 3 consecutive failures. This prevents a single transient network/DNS/HTTP failure from immediately becoming an Incident.

## Queue integration

After a Site Monitoring check is persisted, a database trigger does one small infrastructure action: when an enabled policy exists, it enqueues a deduplicated `site_incident.evaluate` background job for that exact check.

The trigger does **not** evaluate thresholds or create Incidents. Business logic remains in the typed Control API handler.

This makes manual and scheduled Site checks behave identically and avoids a second monitoring implementation.

## Stale checks

The evaluator verifies that its `check_id` is still the latest check for the Site.

If a later check has already completed, the older evaluation job returns `STALE_CHECK` and does nothing. This prevents a delayed worker from opening an Incident for an outage that already recovered.

## Duplicate prevention

A Site policy tracks its linked automatic Incident.

If that Incident is still unresolved, later failed checks do not create another Incident for the same Site.

When the linked Incident has been resolved, only failures that occurred after its resolution are considered for a new automatic Incident.

The policy row is locked during evaluation, preventing concurrent evaluation jobs for the same Site from racing to create duplicate Incidents.

## Incident creation

When the threshold is reached, the evaluator calls the existing `IncidentStore::create` path with the Site as the Incident source.

The generated internal Incident therefore receives:

- the normal INVESTIGATING lifecycle;
- the configured severity;
- a frozen Dependency Graph impact snapshot;
- normal Incident timeline/audit/domain-event behavior;
- a title identifying the unavailable Site;
- an internal summary explaining the consecutive-failure threshold.

The user who configured the policy is recorded as the operator provenance for the automatic Incident.

## No automatic resolution

Recovery does not automatically resolve an Incident.

A healthy check stops future threshold accumulation, but the existing Incident remains open until an operator explicitly moves it through the Incident lifecycle and resolves it.

This avoids hiding incidents merely because a service temporarily recovered.

## No automatic publication

Automatic Incidents are internal only.

Site Incident Automation never:

- creates a Status Page publication;
- changes a Status Page to PUBLIC;
- reuses the internal Incident title/summary as public text;
- sends customer-facing updates.

The existing Status Pages explicit-publication boundary remains unchanged.

## Jobs Administration

Evaluation jobs use the standard worker queue, leases, retries and DEAD state. `/jobs` can therefore show failed automation evaluations and supports explicit retry of a DEAD evaluation job without exposing or editing its payload.

## UI

The Project workspace shows an Incident automation section per active Site:

- monitoring configuration availability;
- enabled/disabled state;
- threshold;
- severity;
- link to the currently tracked Incident.

## Non-goals

V1 does not implement:

- automatic Incident resolution;
- automatic Status Page publication;
- DEGRADED-only incidents;
- time-window percentage/SLO policies;
- multi-region quorum logic;
- paging/escalation policies;
- AI root-cause attribution;
- cross-Site aggregation into one Incident.

## Next phase

A useful next operations slice is Domain / TLS lifecycle monitoring: expiration and certificate warnings can feed the existing Notifications, Jobs and Incident systems without requiring a new event architecture.

## Validation gate

Merge only after Rust workspace tests, `cargo fmt --all -- --check`, and the web TypeScript check pass.
