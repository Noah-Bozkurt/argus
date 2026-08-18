# Firewall Enforcement V1

Argus can now explicitly enforce one safe desired-state transition: an inactive UFW firewall may be enabled when the saved policy says the firewall must be active.

Full security-policy `ENFORCE` mode remains disabled. SSH configuration changes, firewall disable, arbitrary rule editing and automatic-security-update enforcement remain monitor-only.

## Preconditions

- The Server must be under an active maintenance window.
- Desired State must contain `firewall_enabled = true`.
- The agent must advertise `security.firewall.v1`.
- UFW, OpenSSH server configuration inspection and systemd transient timers must be available.
- Effective SSH port(s) must be detectable with `sshd -T`.

The UI queues the command as HIGH risk. The normal server command conflict, history and audit path remains in use.

## Connectivity preflight

Before enabling UFW, the privileged helper reads effective OpenSSH configuration with `sshd -T` and extracts every effective SSH port. If no valid port can be determined, enforcement fails closed.

For every detected SSH port Argus adds an explicit TCP allow rule before changing firewall state.

No SSH authentication setting is changed in this phase.

## Timed rollback

If UFW is currently inactive, the helper first arms a transient systemd timer that will run:

```text
/usr/sbin/ufw --force disable
```

120 seconds later.

Only after the rollback is armed does the helper run `ufw --force enable`, then verify UFW reports `active`.

The agent submits the successful command result to the Control API while the rollback timer remains armed. Only when the Control API acknowledges that result does the agent ask the helper to cancel the timer.

Consequences:

- if UFW activation fails, the operation fails;
- if the agent cannot deliver the result to the Control API, rollback stays armed;
- if the agent/helper cannot disarm rollback after acknowledgement, rollback stays armed and Argus logs a warning;
- an already-active firewall is left active and no rollback timer is created.

This is intentionally fail-safe rather than trying to report success while connectivity confirmation is ambiguous.

## Protocol

Protocol version: 1.7.

New typed command:

- `security.firewall.enable`

New capability:

- `security.firewall.v1`

Internal helper-only requests:

- `security.firewall.enable { rollback_id }`
- `security.firewall.commit { rollback_id }`

The rollback identifier is the command UUID and is validated before it is incorporated into a transient systemd unit name.

## Non-goals

V1 does not:

- disable UFW;
- reset or replace existing UFW rules;
- expose arbitrary firewall-rule commands;
- modify `sshd_config`;
- enable full Desired State reconciliation;
- claim that an external SSH client has successfully reconnected.

The next Desired State phase can build reconciliation and richer enforcement on top of this guarded primitive rather than adding unrestricted privileged configuration writes.
