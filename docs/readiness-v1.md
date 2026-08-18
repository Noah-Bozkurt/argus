# Release / Launch Readiness V1

Readiness is a read-only assessment layer over existing Argus project, development and operations signals. It answers whether the currently modeled project looks ready to release or launch without performing corrective actions itself.

## Read-only boundary

`GET /projects/:project_id/readiness` recalculates an assessment from current data on every request.

Readiness does **not**:

- deploy or roll back;
- install package updates;
- reboot Servers;
- create or verify backups;
- alter security settings;
- change repository state;
- resolve Incidents;
- modify Site monitoring.

A blocked or warning check provides evidence only. Any remediation remains an explicit operation through the relevant Argus module.

## Result model

Each check is one of:

- `PASS` — the modeled signal is ready;
- `WARN` — attention is recommended, but Argus does not make it a hard blocker;
- `BLOCKED` — the current modeled state contains a release/launch blocker;
- `SKIPPED` — the check is not applicable to this project shape.

Overall status is:

- any `BLOCKED` -> `BLOCKED`;
- otherwise any `WARN` -> `ATTENTION`;
- otherwise -> `READY`.

## Project-shape awareness

Argus does not require every project to run on an Argus-managed VPS.

A static Astro/Cloudflare Pages project may legitimately have no production Server. In that case Server health, Agent security, package-update and system-config backup checks are `SKIPPED`, not failures.

A project that **does** contain production Servers receives the full Server, backup, security and update checks.

## V1 checks

### Production Environment

- BLOCKED when no production Environment exists;
- WARN when active Services are not assigned to any Environment;
- PASS otherwise.

### Production Deployments

For each active Service assigned to production, the latest Deployment for that Service/Environment must be `SUCCEEDED`.

- missing/latest unsuccessful deployment -> BLOCKED;
- no active production Services -> SKIPPED.

### Repository CI

Repositories linked to active production Services are evaluated from the existing GitHub metadata snapshot.

- repository sync error or CI failure -> BLOCKED;
- CI unavailable/not successful or sync not current -> WARN;
- synced + successful -> PASS;
- no linked repository -> WARN.

### Site Monitoring

For every active Site:

- latest HEALTHY check -> ready signal;
- latest DEGRADED check -> WARN;
- DOWN/ERROR -> BLOCKED;
- no monitor or no check -> WARN;
- no active Sites -> SKIPPED.

### Production Servers

For production Servers:

- offline or missing Agent snapshot -> BLOCKED;
- otherwise PASS;
- no production Servers -> SKIPPED.

### Verified Backup

Every production Server must have at least one backup artifact marked verified.

- missing verified artifact -> BLOCKED;
- no production Servers -> SKIPPED.

This V1 signal verifies archive integrity, not a full restore rehearsal.

### Security Findings

For production Server snapshots:

- CRITICAL/HIGH finding -> BLOCKED;
- MEDIUM finding or unavailable security snapshot -> WARN;
- otherwise PASS;
- no production Servers -> SKIPPED.

This is a release-readiness signal, not a compliance certification or synthetic security score.

### Pending Updates

- pending package updates or required reboot -> WARN;
- otherwise PASS;
- no production Servers -> SKIPPED.

V1 does not automatically install those updates.

### Open Incidents

- unresolved MAJOR/CRITICAL Incident -> BLOCKED;
- unresolved MINOR Incident -> WARN;
- no unresolved Incidents -> PASS.

### Release Record

Latest Release:

- READY/RELEASED -> PASS;
- FAILED/ROLLED_BACK -> BLOCKED;
- DRAFT or no Release -> WARN.

## Evidence

Checks return concise evidence strings such as Service deployment status, repository CI state, Site health, Server backup availability and Incident severity/status. They do not return raw logs, command output or secrets.

## Interpretation

Readiness describes the state Argus can currently observe. A `READY` result means all applicable V1 checks passed; it is not a guarantee that a deployment or launch cannot fail and is not a substitute for organizational approval, compliance review or manual testing.

## Future work

Possible later additions:

- explicit approval gates for client projects;
- launch checklist templates;
- migration readiness;
- dependency-change review;
- restore-test freshness rather than only verified archive integrity;
- stale-secret/credential checks;
- required custom checks per project.

Those should extend this assessment rather than turn Readiness into an automatic remediation engine.

## Validation gate

Merge only after Rust workspace tests, `cargo fmt --all -- --check`, and the web TypeScript check pass, with no temporary workflow files left on the branch.
