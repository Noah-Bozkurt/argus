# Argus Protocol v1

## Versioning

- `protocol_version` is validated independently from agent version.
- Capabilities are versioned independently (`systemd.v1`, `system.metrics.v1`).

## Handshake

Handshake includes:

- `agent_id`
- `server_id`
- `agent_version`
- `protocol_version`
- `hostname`
- `os`
- `architecture`
- `capabilities[]`

## Enrollment

1. Control API issues short-lived enrollment token.
2. Agent sends token + handshake.
3. Control API validates token/expiry/protocol and returns permanent auth token.

## Heartbeats

Agent sends heartbeat with latest `SystemSnapshot` and `ServiceState[]`.
Heartbeats drive server online/offline state.

## Commands

Command fields:

- `id`, `server_id`
- `command_type`
- `created_at`, `expires_at`
- `status`
- `idempotency_key`
- `risk_level`

Statuses:

- `QUEUED`, `ACCEPTED`, `RUNNING`, `SUCCEEDED`, `FAILED`, `UNKNOWN`, `EXPIRED`

## Results

Agent returns `CommandResult` with:

- `command_id`
- final `status`
- `finished_at`
- optional typed error (`code`, `message`)
