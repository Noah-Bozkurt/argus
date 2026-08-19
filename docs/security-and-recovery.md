# Security & Recovery

Argus performs privileged infrastructure actions, so its design treats authorization, local privilege and rollback as separate boundaries rather than assuming a trusted dashboard is enough.

## Trust boundaries

### Web

The browser/UI is unprivileged. Backend service credentials stay on the Next.js server side. The browser cannot directly send commands to a root Helper.

### Control API

The Control API authenticates operator/backend requests, scopes resources by organization/project and applies command policy such as maintenance requirements, capability checks, conflict rules and risk classification.

### Agent

The Agent authenticates to the Control API, reports actual host state and executes only typed commands supported by its advertised capabilities.

### Helper

The Helper is the local privileged boundary. It listens on a Unix socket, validates inputs and executes fixed system utilities directly. It intentionally has no generic network API or arbitrary shell command endpoint.

Compromise of the Helper should be treated as host-level compromise; its attack surface is therefore kept much smaller than the web/control application.

## Enrollment and command safety

Enrollment tokens are short-lived, single-use and bound to the intended server/organization. Successful enrollment produces a longer-lived device credential stored locally with restrictive permissions.

Commands include:

- server identity;
- explicit typed operation;
- TTL/expiry;
- idempotency key;
- conflict group;
- risk level;
- persisted execution/result status.

Agent reconnects do not blindly replay ambiguous disruptive commands. Operations that were in an uncertain execution state can be recorded as `UNKNOWN` rather than assuming a retry is safe.

## Maintenance policy

Operations such as package upgrades, reboot, firewall enforcement and live restore are maintenance-gated where applicable. The Control API independently enforces this; UI button state is not the security boundary.

## Security inspection

Agents can report a baseline security view including effective SSH settings, UFW state/rules and automatic security-update configuration. Findings identify conditions such as password SSH authentication, direct root login, inactive firewall or disabled automatic security updates.

These findings are observations. Argus does not automatically rewrite every security setting.

## Desired state

Desired state lets an operator declare selected expected security values and compare them with the latest authenticated snapshot.

`MONITOR` can report drift across supported observed fields. `ENFORCE` is currently restricted to the safe firewall-on shape. Unsupported combinations are rejected instead of pretending they are reconciled.

Reconciliation is performed by the persisted worker/job path and still requires maintenance before a firewall mutation can be queued.

## Rollback-safe firewall activation

Firewall enablement is intentionally one-way in the current implementation: Argus may safely enable UFW when desired, but does not expose automatic firewall disable or arbitrary rule editing.

Before enabling an inactive firewall the Helper:

1. validates UFW availability;
2. reads effective OpenSSH configuration with `sshd -T`;
3. determines effective SSH TCP port(s);
4. adds explicit SSH allow rules;
5. arms a transient systemd rollback timer;
6. enables UFW and verifies it reports active.

The rollback remains armed while the Agent submits the successful command result. Only after the Control API acknowledges the result does the Agent ask the Helper to disarm the rollback. If connectivity/result acknowledgement fails, the local timer disables UFW again.

This proves Argus can keep talking to the Agent; it is not a guarantee that every external SSH client/network path has been independently tested.

## Backups

The current system-configuration backup profile captures the security-related host configuration managed by the recovery flow, including SSH, UFW and automatic-update configuration. Agent credentials are intentionally not included.

Backup artifacts include a SHA-256 sidecar and can be explicitly verified. Verification recalculates the checksum and tests archive readability rather than trusting a database flag.

The backup directory is private on the host. Broader volume/database/project backup policies and remote/S3 storage remain future work.

## Restore preflight

Before a backup can be considered a recovery candidate, restore preflight runs without changing live `/etc` state.

It:

- recalculates SHA-256;
- rejects invalid backup/restore identifiers;
- inspects archive entries before extraction;
- rejects absolute paths, traversal, symlinks and special files;
- limits extracted paths to the known system-config profile;
- extracts into a private staging directory;
- validates staged SSH configuration;
- validates staged APT configuration;
- tests staged UFW IPv4/IPv6 rule syntax when present;
- removes staging after success or failure.

A checksum-valid archive is therefore not automatically treated as safe to restore.

## Transactional live restore

Live restore is a CRITICAL, maintenance-gated operation.

The Helper performs the transaction in this order:

1. run the restore preflight again;
2. snapshot the current live managed configuration as a rollback archive;
3. arm a local timed systemd rollback before live writes;
4. apply only allowlisted configuration paths;
5. validate the resulting live SSH/APT/UFW configuration;
6. verify required SSH listener state;
7. return success to the Agent while rollback remains armed;
8. Agent submits the result to the Control API;
9. only after Control API acknowledgement does the Agent disarm rollback.

If the control path breaks after applying configuration, rollback remains a local responsibility and does not depend on the now-broken remote connection.

The UI additionally requires explicit operator confirmation of the selected backup. The Helper does not trust that UI confirmation as its safety check; it reruns its own validation.

## Transactional control-plane self-update

The first-test single-server updater is intentionally an out-of-band local root operation rather than a normal remote managed command. Updating the control plane through the same API/Agent path it is about to stop would create a circular failure dependency.

`argusctl update` treats `main` only as a registry discovery alias. Before mutation it verifies that:

- the currently running Web, Control API, Worker and Content images all carry the same valid full-commit revision label;
- their current local image IDs are pinned under that immutable SHA for rollback;
- the target host-tools alias resolves to a valid full commit SHA;
- all five target Argus images exist under that same SHA and carry the expected revision label;
- the complete target native/deployment bundle can be extracted before downtime starts.

Once preflight is complete, the updater saves root-only copies of deployment files, systemd units and native binaries, stops the Agent/Helper and control-plane writers, and takes a custom-format PostgreSQL dump while PostgreSQL remains local and isolated. That dump covers both the normal Argus control schema and Payload's `argus_content` schema.

Only after that snapshot succeeds does the updater install the target assets and allow target startup/migrations. A target update is successful only when service health, Caddy reload, native Agent/Helper startup and the full `argusctl smoke` verification pass.

If a mutation-stage step fails, automatic rollback restores the previous files/binaries. If the database snapshot had completed, rollback terminates database clients, recreates the Argus database from the pre-update dump and starts the previous SHA-pinned control plane. Rollback itself must pass service health and `argusctl smoke` before it is reported as recovered.

The pre-update snapshot is retained after success for investigation/recovery, but V1 does not expose an unrestricted later restore button. A later database rollback can destroy data created after an otherwise successful update, so that capability needs an explicit data-loss confirmation model and retention policy first.

This mechanism is a first-test single-node safety boundary. It is not yet a production rolling-upgrade protocol, HA database migration strategy or general point-in-time recovery system.

## Secrets and identity limitations

Not yet complete:

- first-class encrypted secret objects and rotation;
- mTLS/public-key Agent identity;
- 2FA/passkeys/OIDC/SSO;
- a complete fine-grained RBAC model across every future business/client feature;
- dedicated security-event response workflows.

Until a first-class secrets subsystem exists, environment/service credentials should be treated as deployment secrets and kept out of source control, logs and public APIs.

## Recovery limitations

Current system-config transactional restore and control-plane update rollback are bounded recovery flows. They are not yet a full disaster-recovery engine for arbitrary Docker volumes, external application databases, uploaded application media or whole-project reconstruction.

Future backup expansion should preserve the same rule used here: a backup capability is not complete until restore, validation and failure recovery are tested.
