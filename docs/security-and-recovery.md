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
- the complete target native/deployment bundle can be extracted before downtime starts;
- enough free snapshot space remains after the target images have actually been pulled.

The updater then creates a format-2 transaction. It copies the pre-update deployment files, native binaries and systemd units and seals that fixed file set with a SHA-256 manifest before the rollback boundary is armed. Agent/Helper and control-plane writers are only stopped after that manifest is durably present and verifies successfully.

With writers quiesced, the updater takes a custom-format PostgreSQL dump while PostgreSQL remains local and isolated. That dump covers both the normal Argus control schema and Payload's `argus_content` schema. Before target files may be installed, `pg_restore --list` must accept the archive and a separate SHA-256 manifest is persisted for the dump.

Target deployment files/binaries may then replace the installed copies. Immediately before any target Compose service may start, the updater writes and syncs a `target-start-armed` marker containing the exact target revision. That marker is the durable boundary that says target startup/migrations may now have happened. A target update is successful only when service health, Caddy reload, native Agent/Helper startup and the full `argusctl smoke` verification pass.

If a normal mutation-stage step fails, automatic rollback first verifies the file snapshot checksum and restores the previous files/binaries. The PostgreSQL database is restored only when target start had already been armed; a failure before that marker cannot have launched target migrations through the updater, so recreating the database would add unnecessary risk. When database rollback is required, the stored dump checksum must still validate before the previous SHA-pinned control plane is started. Rollback itself must pass service health and `argusctl smoke` before it is reported as recovered.

Transaction result files and phase markers are written atomically with an explicit filesystem sync. The pre-update snapshot is retained after success for investigation/recovery, but V1 does not expose an unrestricted later restore button. A later database rollback can destroy data created after an otherwise successful update, so that capability still needs an explicit data-loss confirmation model.

### Interrupted update / reboot recovery

A hard process loss or reboot cannot execute the updater's normal error trap. The privileged Helper therefore has a local `ExecStartPre` recovery hook that invokes hidden `argusctl recover-update` before Helper startup. Recovery and the live updater use the same lifecycle `flock`: during a normal update the lock is already held and recovery is a no-op; after a reboot the lock is free and the newest unfinished transaction can be evaluated before the Helper and dependent Agent resume normal operation.

Format-2 recovery uses durable transaction phases instead of inferring every crash boundary from the current `.env`:

- metadata without a sealed file-snapshot manifest means the updater had not armed live mutation yet; when the installed revision is still `FROM_REVISION`, the transaction is closed as `ABORTED_PRE_MUTATION` without copying a partial snapshot back over the host;
- a sealed file snapshot must pass its SHA-256 manifest before any recovery copy occurs;
- a missing `target-start-armed` marker means target Compose startup/migrations were never permitted, so recovery restores the pre-update files but deliberately leaves PostgreSQL intact;
- when `target-start-armed` exists, it must contain the expected `TO_REVISION`, the database checksum must validate and `pg_restore --list` must accept the dump before database rollback proceeds.

This also covers a reboot during target-file installation: the rollback file snapshot is already sealed, but because the start marker has not yet been written, recovery restores the mixed/partial installed files without unnecessarily replacing an otherwise untouched database.

Older format-1 transactions remain recoverable through the previous conservative revision/dump logic. A `ROLLBACK_FAILED` result is preserved and is not automatically re-executed forever; subsequent lifecycle work still has to pass the normal installation/smoke checks.

After file/database recovery, the previous SHA-pinned Compose services are started, the Control API and Caddy are validated and each custom container's OCI revision label must match the stored `FROM_REVISION`. When recovery is run manually before a retry, Helper and Agent are also restarted and the full `argusctl smoke` must pass. During boot-time Helper pre-start, systemd continues native service startup after the core rollback; a post-boot `sudo argusctl smoke` remains the final operator verification.

The crash-recovery hook only protects updates performed after a version containing this hook is installed. An older updater cannot retroactively guarantee recovery for instruction-level failure windows before the new Helper unit/CLI have first landed on disk.

This mechanism remains a first-test single-node safety boundary. It is not yet a production rolling-upgrade protocol, HA database migration strategy or general point-in-time recovery system.

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
