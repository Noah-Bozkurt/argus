# Server Management V1

This milestone builds on the persistent infrastructure vertical slice and intentionally stays small.

## Implemented

### Read-only server update inventory

Each agent heartbeat includes an `UpdateState` in the system snapshot:

- whether APT inventory is supported on the host;
- number of packages currently visible as upgradeable from the host's existing APT metadata;
- whether `/var/run/reboot-required` exists.

Argus uses `apt-get -s -o Debug::NoLocking=1 upgrade` for inventory. It does **not** run `apt update`, install packages, or mutate the server during heartbeat collection. The agent caches this inventory and refreshes it every five minutes rather than executing APT simulation on every five-second heartbeat.

This means the count reflects the server's current package metadata and may be stale until the package lists are refreshed by the administrator or a future explicit Argus update-check job.

### Controlled systemd actions

The typed command protocol now supports:

- `service.start`
- `service.stop`
- `service.restart`

All three operations:

1. are queued and persisted by the Control API;
2. require the agent's `systemd.v1` capability;
3. travel through the agent/helper Unix socket;
4. are restricted to the helper's configured service allowlist;
5. invoke `systemctl` directly without a shell;
6. return their result through the existing command history and audit path.

`service.stop` is submitted as a HIGH-risk operation in the current web UI. Start and restart remain MEDIUM-risk.

### Server UI

The server detail page now shows:

- pending APT package updates;
- reboot-required state;
- start/stop/restart controls for managed services;
- existing command history and command results.

## Deliberate non-goals

This milestone does not yet perform package upgrades, refresh APT metadata, reboot servers, stream journald logs, manage arbitrary systemd units, schedule maintenance, or send notifications.

Those mutating operations should be added only after explicit typed commands, risk/authorization handling, and tests are defined for them.

## Next recommended milestone

The next server-management slice should add:

1. explicit APT metadata refresh / update-check as a background job;
2. typed security-only and full package upgrade operations;
3. typed reboot operation with HIGH/CRITICAL policy;
4. maintenance windows so expected disconnects do not create noisy incidents;
5. journald read APIs before any browser terminal work.
