# Transactional Restore V1

Argus can now apply a verified `system-config` backup through a guarded live restore transaction.

This is a CRITICAL operation. It is intentionally narrower than arbitrary filesystem recovery and remains limited to the fixed backup profile managed by Argus.

## Preconditions

A live restore can only be queued when:

- the Server has an active maintenance window;
- the selected backup has been integrity-verified in the current inventory;
- the operator types the exact backup filename in the UI;
- the agent supports protocol 1.9 and `backup.v1`.

The privileged Helper does not trust those UI checks. It validates identifiers and re-runs the complete Restore Preflight V1 immediately before touching live configuration.

## Managed paths

Only these paths participate in the transaction:

- `/etc/ssh/sshd_config`
- `/etc/ssh/sshd_config.d`
- `/etc/ufw`
- `/etc/apt/apt.conf.d/20auto-upgrades`

Argus agent credentials and arbitrary filesystem paths remain out of scope.

Live restore additionally requires UFW configuration to be present in the candidate backup. A backup without UFW data may still pass read-only preflight, but it cannot be applied live because deleting an active firewall configuration without a replacement is not a safe transaction.

## Transaction sequence

1. Re-run checksum, archive allowlist, staged extraction and staged SSH/APT/UFW validation.
2. Create a rollback archive from the current live managed paths.
3. Record whether UFW is currently active.
4. Arm a 120-second transient systemd rollback timer.
5. Remove only the fixed managed paths and extract the candidate backup to `/`.
6. Validate the actual live SSH, APT and UFW files again.
7. Resolve effective restored SSH port(s).
8. If UFW was active before restore, add explicit TCP safety rules for every restored SSH port and keep UFW active. If it was inactive, keep it inactive.
9. Reload OpenSSH (`ssh.service`, falling back to `sshd.service`).
10. Verify every effective SSH port is listening locally.
11. Return success to the Agent while the rollback timer is still armed.
12. The Agent submits the successful command result to the Control API.
13. Only after the Control API acknowledges the result does the Agent call the Helper commit operation to stop the rollback timer and delete rollback state.

If result submission fails, the timer stays armed.

## Immediate and timed rollback

If any live apply or validation step fails, the Helper immediately attempts rollback. The independent timer is deliberately not disarmed before rollback succeeds, so a failed immediate rollback retains another recovery attempt.

Rollback:

- removes the managed candidate paths;
- restores the pre-change archive;
- restores the previous UFW runtime active/inactive state;
- restores persistent files again after UFW state commands so `ufw.conf` matches its exact pre-change content;
- validates and reloads SSH;
- only then disarms the timer and removes transaction state.

The transient systemd timer invokes an internal one-shot `argus-helper --restore-rollback <command-uuid>` mode. The rollback UUID is validated and comes from the typed command ID; the browser never supplies a helper executable, filesystem path or rollback command.

## Firewall safety override

When the firewall was active before the restore, Argus intentionally adds explicit UFW allow rules for restored SSH ports. Those safety rules remain in the restored configuration after commit. This means the applied UFW files can differ from the archive by those safety rules; avoiding remote lockout takes precedence over byte-for-byte restoration.

The restore preserves the pre-restore *runtime* firewall state: it does not use a backup as an implicit request to enable or disable UFW.

## Failure semantics

A restore is not considered committed merely because local extraction succeeded.

- Local validation failure → immediate rollback.
- Agent cannot report success → timed rollback remains armed.
- Control API acknowledges success but Helper commit fails → timed rollback remains armed.
- Only Control API acknowledgement followed by successful Helper commit finalizes the transaction.

## Non-goals

V1 does not provide:

- arbitrary file or directory restore;
- restore of Argus credentials;
- arbitrary shell commands;
- bypass of maintenance mode;
- backup-supplied firewall activation/deactivation;
- application/database data restore;
- remote SSH login testing from an external host.

A future disaster-recovery phase can add additional typed restore profiles, but each profile should define its own allowlist, preflight, rollback and post-apply verification instead of expanding this command into unrestricted archive extraction.
