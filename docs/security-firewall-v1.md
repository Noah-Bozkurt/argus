# Security & Firewall V1

This phase adds a read-only baseline before Argus is allowed to rewrite network or SSH configuration.

## Inspection

Every five minutes the privileged helper inspects:

- effective OpenSSH settings via `sshd -T`;
- UFW status and up to 100 numbered rules;
- whether unattended security upgrades are enabled.

The resulting structured state is returned to the unprivileged agent over the existing Unix socket and included in the authenticated heartbeat.

## Findings

Initial findings are explicit rather than collapsed into a fake score:

- HIGH `SSH_PASSWORD_AUTH` when password authentication is enabled;
- HIGH `SSH_ROOT_LOGIN` when direct root login is enabled;
- MEDIUM `FIREWALL_INACTIVE` when UFW is not active;
- MEDIUM `AUTO_SECURITY_UPDATES_DISABLED` when unattended upgrades are not enabled.

## Firewall ownership

V1 is observation-only. Existing firewall rules are treated as externally/unmanaged state. Argus does not silently adopt or rewrite them.

## Why no firewall writes yet

Changing firewall/SSH rules can lock the operator out of the host. Write support should only arrive with:

1. explicit ownership (`Unmanaged`, `Argus-managed`, `External`);
2. candidate configuration/preflight;
3. timed rollback;
4. verification that the agent and expected management path remain reachable;
5. maintenance/risk authorization.

That belongs with the Desired State & Drift phase, not a raw `ufw allow` button.
