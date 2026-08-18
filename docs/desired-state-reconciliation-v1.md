# Desired State Reconciliation V1

Argus now supports a deliberately narrow `ENFORCE` mode for the one security transition that has a rollback-safe primitive: making an inactive UFW firewall active.

## Supported ENFORCE shape

V1 accepts `ENFORCE` only when all of the following are true:

- `firewall_enabled = true`
- `ssh_password_auth` is unset
- `ssh_root_login` is unset
- `automatic_security_updates` is unset

Any broader `ENFORCE` policy is rejected with `ENFORCEMENT_UNAVAILABLE` instead of silently treating unsupported fields as enforced.

`MONITOR` mode keeps the existing behavior and may contain any supported observation-only policy fields.

## Scheduling

Saving a server policy also maintains one existing Jobs/Worker schedule:

- job kind: `desired_state.reconcile`
- resource key: Server UUID
- interval: 60 seconds
- enabled only while the policy mode is `ENFORCE`
- Project scope is derived from the Server
- the operator who saved the policy is retained in the typed payload for provenance

Switching back to `MONITOR` disables the reconciliation schedule without deleting policy history.

## Reconciliation decision

Each run reads the current policy and latest authenticated agent snapshot.

It returns without mutation when:

- policy is no longer `ENFORCE`;
- no snapshot exists yet;
- security inspection is unavailable;
- UFW already reports active.

If firewall drift exists, Argus checks the existing Server maintenance state.

Without an active maintenance window the job succeeds with `FIREWALL_DRIFT_BLOCKED_MAINTENANCE` and performs no mutation. This avoids retry storms and makes maintenance an explicit operational gate rather than a UI-only convention.

During maintenance, the job queues the existing HIGH-risk `security.firewall.enable` command. It does not call UFW directly. The command therefore retains the rollback-safe SSH preflight and 120-second firewall rollback introduced in Firewall Enforcement V1.

If an equivalent security mutation is already queued/running, the queue conflict is treated as `FIREWALL_RECONCILIATION_ALREADY_QUEUED` instead of creating a duplicate.

## Ownership and safety

The job payload includes Server and actor UUIDs. Before reconciling, Argus resolves the Server through the existing authenticated storage path and verifies its Project matches the scheduled Project.

No arbitrary command, firewall rule, SSH configuration, package update or Docker operation is introduced by this phase.

## UI

Server Desired State now exposes `MONITOR` and `ENFORCE` modes and explains the supported shape. In `ENFORCE`, firewall drift is evaluated every minute. A maintenance window is still required before an actual firewall change can be queued.

The explicit `Enable firewall safely` action remains available in `MONITOR` mode for operators who want drift monitoring without recurring reconciliation.

## Next phase

Restore / Disaster Recovery remains the next high-priority incomplete safety capability. Broader Desired State enforcement should only be added field-by-field after each field has equivalent transactional preflight, rollback and post-change verification semantics.
