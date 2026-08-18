# Restore Preflight V1

Argus can now validate a `system-config` backup for recovery without writing anything to live system configuration.

## Flow

`backup.restore.preflight` is a typed LOW-risk command using the existing authenticated Control API → Agent → privileged Helper path.

The Helper independently:

1. validates the backup and restore identifiers;
2. recalculates SHA-256 and compares it with the stored sidecar;
3. inspects the tar archive before extraction;
4. rejects absolute paths, traversal, symlinks, devices and other special entries;
5. allows only the fixed `system-config` paths under SSH, UFW and `20auto-upgrades`;
6. extracts to a private temporary staging directory;
7. validates staged configuration;
8. removes staging again after success or failure.

No live `/etc` file is changed by this command.

## Required archive contents

A candidate must contain at least:

- `etc/ssh/sshd_config`
- `etc/apt/apt.conf.d/20auto-upgrades`

Optional files remain limited to:

- `etc/ssh/sshd_config.d/*`
- `etc/ufw/*`

Anything outside that allowlist fails closed.

## Validation

### SSH

The staged `sshd_config` is checked with `sshd -t`. Standard absolute Includes under `/etc/ssh/sshd_config.d/` are rewritten only in a temporary validation copy so they resolve to the staged include directory. Other Include roots are rejected rather than accidentally validating against live configuration.

### APT

The staged `20auto-upgrades` file is parsed through `apt-config -c <staged-file> dump`.

### UFW

When present:

- `user.rules` is validated with `iptables-restore --test`;
- `user6.rules` is validated with `ip6tables-restore --test`.

The validators receive staged rule content through stdin. Rules are never applied.

## Staging safety

The restore workspace defaults next to the backup directory under a private `restores` directory and can be overridden with `ARGUS_RESTORE_DIR`. Staging directories use restrictive permissions and are removed after the preflight finishes.

Archive inspection is capped at 512 entries.

## Non-goals

V1 deliberately does not:

- overwrite live configuration;
- reload SSH or UFW;
- create a rollback transaction;
- claim that a backup is safe to apply merely because its checksum is valid;
- restore Argus agent credentials or arbitrary paths.

A live restore will only be added after a pre-change rollback archive, timed rollback, post-apply syntax/listener checks and successful Agent → Control API acknowledgement are part of one transaction.
