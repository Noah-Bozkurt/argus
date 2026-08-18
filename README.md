# argus

Argus is a project-centric development and infrastructure platform.

## Implemented first-slice foundation

- Monorepo with pnpm + Cargo workspaces
- Minimal Next.js web slice (`apps/web`) for dashboard/projects/servers pages
- Rust control API (`services/control-api`) with:
  - enrollment token + permanent agent identity flow
  - authenticated heartbeat
  - typed command queue with TTL, idempotency, conflict checks
  - command result + audit/event recording
- Rust helper (`crates/helper`) allowlisted systemd restart action
- Rust agent runtime (`crates/agent`) typed command execution against helper boundary
- Shared protocol/domain/system crates
- `argusctl` local diagnostics CLI
- Documentation in `docs/`

## Quick start

```bash
cargo test --workspace
cargo run -p control-api
```
