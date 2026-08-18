# Development

## Workspace layout

- Rust: Cargo workspace (`services/control-api`, `crates/*`)
- JS: pnpm workspace (`apps/*`, `packages/*`)

## Run control API

```bash
cargo run -p control-api
```

Optional PostgreSQL migration bootstrap:

```bash
DATABASE_URL=******localhost/argus cargo run -p control-api
```

## Run CLI

```bash
cargo run -p cli -- status
cargo run -p cli -- system info
```

## Tests

```bash
cargo test --workspace
```

## Notes

- `Project.client_id` is optional.
- Helper only supports allowlisted systemd restart operations.
- Queue enforces TTL, conflict, and idempotency behavior.
