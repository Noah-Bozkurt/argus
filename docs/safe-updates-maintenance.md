# Safe Updates & Maintenance

This milestone adds explicit maintenance windows and typed package/reboot operations on top of the existing persistent command path.

## Maintenance windows

Maintenance windows are persisted per server and organization. A window records its start/end time, reason, creator, creation time, and optional early end time.

The Control API — not only the UI — requires an active maintenance window before accepting:

- `packages.upgrade.security`
- `packages.upgrade.all`
- `system.reboot`

`packages.refresh` does not require maintenance because it only refreshes package metadata.

## Package operations

All package mutations still follow:

`Web -> Control API -> PostgreSQL command -> Agent -> privileged Helper`

The helper executes fixed argument vectors directly; it never invokes a shell.

- `packages.refresh` runs `apt-get update`.
- `packages.upgrade.all` runs a non-interactive `apt-get upgrade` while preserving existing config files.
- `packages.upgrade.security` uses `unattended-upgrade`, which applies the host's configured unattended-upgrades origins (normally security updates on Ubuntu/Debian). If the utility is unavailable the command fails with `CAPABILITY_UNAVAILABLE` rather than falling back to an unsafe approximation.

After package operations the agent immediately refreshes its package inventory instead of waiting for the normal five-minute inventory interval.

## Reboot

`system.reboot` is a typed CRITICAL operation and requires active maintenance. The privileged helper invokes `systemctl reboot` directly.

Current limitation: this milestone treats successful acceptance by systemd as command success. A later reliability slice should correlate the following agent reconnect and uptime reset to produce a separately verified reboot state. Do not use the command result alone as proof that the machine completed a reboot.

## UI

The server page now supports:

- starting a 30 or 60 minute maintenance window;
- ending active maintenance early;
- viewing maintenance history;
- refreshing APT metadata;
- installing security updates;
- installing all available upgrades;
- requesting a reboot.

The disruptive buttons are disabled without maintenance, but the Control API independently enforces the same policy.

## Safety boundaries

This milestone does not add arbitrary shell execution, browser terminal access, automatic upgrades, scheduled reboots, firewall changes, or incident suppression yet. Maintenance windows are now persisted so later incident/notification logic can consume them.

## Next milestone

The next recommended milestone is Logs & Diagnostics:

1. read-only journald access with service/time/severity filters;
2. failed systemd unit inventory;
3. disk/memory/process diagnostics;
4. a redacted diagnostic bundle;
5. explicit reboot verification using disconnect/reconnect + uptime correlation.
