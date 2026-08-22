# AGENTS.md

Guidance for coding agents working in this repository.

## Priorities

1. Preserve Argus security and transactional recovery guarantees.
2. Prefer the smallest maintainable change that solves the current problem.
3. Keep public interfaces typed and backward-compatible unless the change explicitly requires a break.
4. Update tests and canonical documentation with behavior changes.

## Repository map

- `apps/web`: Next.js operator application.
- `apps/content`: Payload/Next.js content service.
- `apps/installer`: public installer portal.
- `services/control-api`: Rust/Axum API and SQL migrations.
- `services/worker`: persisted background-job worker.
- `crates/agent`: unprivileged managed-host agent.
- `crates/helper`: privileged host helper and Unix-socket boundary.
- `crates/cli`: `argusctl` and native installer.
- `crates/protocol`: shared Agent/Control/Helper types.
- `deploy`: Compose, Caddy, systemd, and image definitions.
- `scripts`: lifecycle, smoke, update, rollback, and CI validation scripts.

## Working rules

- Read the relevant canonical document in `docs/` before changing a subsystem.
- Do not weaken Agent/Helper separation or add generic remote shell execution.
- Never print, commit, or place secrets in command-line arguments, public environment variables, logs, fixtures, or snapshots.
- Preserve update ordering: validate and prefetch before mutation; snapshot before destructive work; retain the original failure when rollback runs.
- Treat Payload schema changes as migrations. Review generated SQL and test the full migration chain against PostgreSQL 16.
- Keep Compose images coordinated by immutable Git revision.
- Do not hand-edit generated files unless their owning workflow requires it.
- Do not add a new framework or abstraction when an existing local pattern is sufficient.

## Required validation

Run the checks relevant to the files changed. The baseline is:

```bash
cargo fmt --all -- --check
cargo test --workspace
pnpm install --no-frozen-lockfile
pnpm --filter @argus/web exec tsc --noEmit
pnpm --filter @argus/content run typecheck
node --test apps/installer/test.mjs
bash -n install.sh scripts/*.sh
```

For deployment or lifecycle changes, also run the targeted CI/runtime validation documented in `docs/development.md`. If a required tool or service is unavailable, state exactly which check was not run and why.

## Change hygiene

- Keep commits and PRs focused.
- Add an entry under `CHANGELOG.md` → `Unreleased` for operator-visible changes.
- Update `README.md`, `DESIGN.md`, or the relevant `docs/` page when behavior or architecture changes.
- Never claim a test passed unless it was executed successfully.
