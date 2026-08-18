# Backups & Recovery V1

Argus V1 provides a small, typed backup path for managed Linux nodes. The goal of this phase is to establish trustworthy backup creation, inventory and integrity verification without prematurely exposing destructive restore operations.

## Capability

The agent advertises `backup.v1` and supports two typed commands:

- `backup.create` with the fixed `system-config` profile
- `backup.verify` for an existing Argus backup artifact

The agent itself does not receive arbitrary filesystem access. Backup filesystem work remains inside the privileged helper.

## System-config profile

The first profile contains only explicitly allowlisted recovery configuration:

- `/etc/ssh/sshd_config`
- `/etc/ssh/sshd_config.d`
- `/etc/ufw`
- `/etc/apt/apt.conf.d/20auto-upgrades`

The profile intentionally does **not** archive `/etc/argus/agent.json`, agent credentials, arbitrary application environment files, private keys or arbitrary user-supplied paths.

## Target

The privileged helper writes to:

`/var/lib/argus/backups`

by default. Operators can override this with `ARGUS_BACKUP_DIR`. The directory is created with mode `0700` on Unix. A mounted external filesystem can therefore be used without changing the protocol.

Local-only storage should not be considered protection against total host loss. A later object-storage adapter will provide off-host retention.

## Integrity

Every created archive receives a SHA-256 sidecar. `backup.verify`:

1. recalculates SHA-256;
2. compares it with the recorded digest;
3. runs `tar -tzf` to verify that the archive is readable;
4. records a successful verification marker.

The server heartbeat exposes backup name, profile, size, creation time, digest and the last successful verification state.

Verification is point-in-time. A backup should be reverified periodically and after transfer to another storage target.

## Safety limits

- no arbitrary backup paths;
- no shell execution;
- no backup deletion/pruning yet;
- inventory is capped at 50 artifacts;
- only the fixed `system-config` profile is accepted;
- restore is not implemented in V1.

## Restore policy

Argus must not expose a one-click restore until the restore path has:

- an active maintenance window;
- preflight validation;
- a backup of the current configuration;
- candidate extraction to a staging location;
- syntax validation for affected services;
- timed rollback protection;
- connectivity verification for SSH/network changes;
- post-restore service and agent health checks;
- audit events for request, commit, rollback and result.

Until those guarantees exist, the UI reports backups and integrity status but deliberately offers no restore action.
