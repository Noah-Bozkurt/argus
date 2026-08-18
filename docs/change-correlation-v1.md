# Change Correlation V1

Change Correlation helps investigate Incidents by showing tracked changes close to Incident start. It does not claim that a nearby change caused the Incident.

## Window

V1 uses a fixed window of ±120 minutes around `incident.started_at`.

A fixed window keeps the first version deterministic and avoids introducing correlation-settings infrastructure before there is evidence that per-project tuning is needed.

## Sources

V1 combines existing Argus data rather than creating a second event database:

- Deployments;
- Releases;
- server Commands;
- project resource events for Services, Environments, Repositories, Sites, Domains and Dependencies.

Monitoring checks and Incident events themselves are excluded from generic project changes to reduce obvious noise.

## Impact-related changes

Incident creation already stores a historical blast-radius snapshot. Change Correlation uses that snapshot, not the current Dependency Graph, to decide whether a change is impact-related.

Examples:

- a Deployment touching an affected Service;
- a Deployment targeting an affected Environment;
- a server Command on an affected Server;
- a repository/site/domain/service event carrying an affected resource ID.

This keeps an old Incident investigation stable even when the current topology is later edited.

## Ordering

Results prioritize:

1. changes related to the Incident root/blast radius;
2. smallest absolute time distance from Incident start;
3. event time.

Signed time distance remains visible so operators can distinguish a change before the Incident from activity after it.

## Interpretation

The UI must describe these records as nearby or correlated changes, never as proven causes.

A deployment five minutes before an outage may be highly relevant, but Argus cannot infer causality merely from timing. Dependency impact and correlation are investigation aids.

## Data minimization

V1 returns concise summaries rather than raw build logs, command output or arbitrary event payloads. Server command summaries include command kind, server name and command status; project events are summarized by event type and a safe name/version when available.

## Non-goals

V1 does not implement:

- automatic root-cause scoring;
- machine-learning causality claims;
- configurable correlation windows;
- external change sources;
- full audit-log rendering;
- automatic Incident resolution.

## Next phase

Status Pages V1 can now expose explicitly selected, public-safe Incident information while keeping the internal Incident timeline, graph snapshot and correlation data private.

## Validation gate

Merge only after Rust workspace tests, `cargo fmt --all -- --check`, and the web TypeScript check pass, with no temporary workflow files left on the branch.
