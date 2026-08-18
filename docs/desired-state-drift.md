# Desired State & Drift

Argus can record a desired security baseline per managed server and compare it with the latest authenticated agent snapshot.

## V1 policy fields

- UFW firewall active/inactive
- SSH password authentication enabled/disabled
- SSH root login (`no`, `prohibit-password`, or `yes`)
- automatic security updates enabled/disabled

Every field is optional. An unset field is intentionally ignored rather than treated as `false`.

## Modes

`MONITOR` is the only executable mode in this phase. Argus stores the policy, computes drift, and shows HIGH/MEDIUM findings without changing the host.

`ENFORCE` is represented in the schema for forward compatibility but the Control API rejects attempts to enable it with `ENFORCEMENT_UNAVAILABLE`.

This is deliberate. SSH and firewall changes must not be automatically applied until Argus has transactional safety including configuration backup, syntax validation, connectivity preflight, a timed rollback guard, post-change health verification, and an explicit ownership model.

## Drift severity

Security/network drift is currently classified as:

- HIGH: firewall state, SSH password authentication, SSH root login
- MEDIUM: automatic security updates

No synthetic security score is calculated.

## Audit

Policy updates create both an audit event and the `server.desired_state.updated` domain event. Desired-state changes therefore use the same traceability principles as server commands.

## UI

The server detail page exposes tri-state selectors for each policy field:

- do not manage
- desired enabled/allowed
- desired disabled/forbidden

The page also shows the current drift list alongside the separately collected security findings.

## Next safety step

Before any security/network policy can move from Monitor to Enforce, implement transactional changes with connectivity preflight and timed rollback. A failed or unconfirmed connectivity check must roll the candidate configuration back automatically.
