# Real infrastructure vertical slice

This milestone replaces the first in-memory prototype path with a persistent Linux control path.

## Runtime topology

```text
Next.js server
  -> Rust control API
  -> PostgreSQL
  -> authenticated argus-agent (outbound polling)
  -> /run/argus/helper.sock
  -> root argus-helper
  -> systemctl restart <allowlisted.service>
```

The browser never receives the Control API backend credential. Next.js reads `ARGUS_WEB_API_TOKEN`, `ARGUS_ORG_ID`, and `ARGUS_USER_ID` on the server and the Control API verifies that the user belongs to that organization before accessing resources.

## Control API configuration

Required:

- `DATABASE_URL`
- `ARGUS_WEB_API_TOKEN` (minimum 32 characters)

Optional:

- `ARGUS_CONTROL_API_BIND` (default `0.0.0.0:8080`)

The Control API runs SQL migrations on startup. PostgreSQL is authoritative for servers, enrollment tokens, agents, heartbeats, commands, command history, audit records, and domain events.

## Web configuration

Required:

- `ARGUS_CONTROL_API_URL`
- `ARGUS_WEB_API_TOKEN`
- `ARGUS_ORG_ID`
- `ARGUS_USER_ID`

`ARGUS_WEB_API_TOKEN` must match the Control API. The organization and user IDs must already represent a valid membership in PostgreSQL.

This is deliberately a minimal authenticated backend boundary for this milestone. Full interactive Payload authentication and the complete RBAC matrix remain future work.

## Agent enrollment

First start requires:

- `ARGUS_CONTROL_PLANE_URL`
- `ARGUS_SERVER_ID`
- `ARGUS_ENROLLMENT_TOKEN`

The enrollment token is short-lived, single use, server bound, organization bound, and stored hashed in PostgreSQL. After successful enrollment the agent receives a high-entropy device credential and stores it in `/etc/argus/agent.json` with mode `0600`.

The plaintext device credential is not stored by the control plane; only its SHA-256 digest is persisted. Because the credential itself contains high entropy, this is suitable as a lookup verifier for this milestone. Public-key identity/mTLS remains the intended future upgrade.

## Agent runtime

`argus-agent` is a long-running Tokio process. It:

1. loads or creates local enrollment configuration;
2. sends system/service heartbeats;
3. polls for a command;
4. validates capability and expiry;
5. talks to the helper through its Unix socket;
6. submits the terminal result;
7. backs off exponentially (up to 60 seconds) when the control plane is unavailable.

Server online state is derived from `last_seen_at`, not a trusted boolean.

## Command semantics

Commands are persisted before delivery. Claiming uses a PostgreSQL transaction plus `FOR UPDATE SKIP LOCKED`, preventing two consumers from claiming the same row.

The queue enforces:

- TTL;
- per-server idempotency keys;
- conflict groups;
- organization ownership;
- agent/server ownership for results.

If an agent reconnects after being absent for more than 90 seconds, commands left in `ACCEPTED`/`RUNNING` are marked `UNKNOWN` rather than retried automatically.

## Privileged helper

`argus-helper` is a separate process. It listens only on `/run/argus/helper.sock`, with mode `0660`, and should run as `root:argus`. The agent runs as the unprivileged `argus` user/group.

Only typed helper requests are accepted. The current allowlist contains systemd service restarts. Service names are validated and passed to `systemctl` as direct arguments; no shell is used.

The systemd unit templates live in `deploy/systemd/`.

## argusctl

Implemented commands now use local state:

```bash
argusctl status
argusctl health
argusctl connection
argusctl system info
argusctl version
```

`connection` checks the stored device credential against `/agent/identity` without claiming a command.

## Development checks

```bash
cargo fmt --all -- --check
cargo test --workspace
pnpm install --no-frozen-lockfile
pnpm --filter @argus/web exec tsc --noEmit
```

The pull-request CI runs these checks on Ubuntu.

## Remaining limitations

This milestone intentionally does not implement Payload login/session UI, fine-grained RBAC, mTLS, automatic installer/user/group creation, WebSocket command push, Docker, firewall management, status pages, or cloud provisioning.

Command polling is intentionally used instead of WebSockets for the first reliable slice. The protocol and persistence model keep a later push transport possible without changing command semantics.
