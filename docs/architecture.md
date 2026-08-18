# Argus Architecture

Argus is a modular monolith with explicit trust boundaries:

- **Web** (`apps/web`): Next.js UI for projects and infrastructure actions.
- **Control API** (`services/control-api`): Axum API, authorization, command queue, audit/events, PostgreSQL schema bootstrap.
- **Agent** (`crates/agent`): outbound authenticated worker that executes typed commands only.
- **Helper** (`crates/helper`): privileged local executor with strict allowlist and no external network logic.
- **PostgreSQL**: persistent model for organizations, users, projects, servers, services, commands, and audits.

Flow for restart operation:

1. Web requests `service.restart` command.
2. Control API authorizes and enqueues typed command with TTL/idempotency.
3. Agent polls authenticated queue and receives command.
4. Agent calls helper allowlisted restart method.
5. Helper executes `systemctl restart <service>`.
6. Agent submits result; Control API persists status and writes audit/event records.
