# Contributing to Argus

Thank you for helping improve Argus. Argus is licensed under the GNU Affero General Public License v3.0 only. Contributions accepted into the repository are distributed under the same license; see [`LICENSE`](LICENSE).

## Before starting

- Search existing issues and pull requests.
- Use an issue for substantial features, architectural changes, or behavior with migration impact.
- Report vulnerabilities privately as described in [`SECURITY.md`](SECURITY.md), not in a public issue.
- Read [`DESIGN.md`](DESIGN.md), [`AGENTS.md`](AGENTS.md), and the relevant page under `docs/`.

## Development setup

Argus uses stable Rust, Node.js 20+, pnpm 9, PostgreSQL 16, Docker/Buildx, and Linux for privileged Agent/Helper behavior.

```bash
pnpm install --no-frozen-lockfile
cargo fetch --locked
```

See [`docs/development.md`](docs/development.md) for service-specific environment variables, database setup, and migration workflows.

## Making changes

1. Create a focused branch from current `main`.
2. Follow established patterns and avoid unrelated refactors.
3. Add or update tests for behavior changes and failure paths.
4. Update canonical documentation and the `Unreleased` section of [`CHANGELOG.md`](CHANGELOG.md).
5. Never include credentials, production data, private host details, or generated secret values.

Database and Payload changes must include reviewed migrations. Deployment changes must preserve immutable image coordination, pre-mutation validation, rollback snapshots, and smoke verification.

## Validation

Run the checks relevant to your change. A complete baseline is:

```bash
cargo fmt --all -- --check
cargo test --workspace
pnpm --filter @argus/web exec tsc --noEmit
pnpm --filter @argus/content run typecheck
node --test apps/installer/test.mjs
bash -n install.sh scripts/*.sh
```

CI performs additional PostgreSQL, migration, Compose, installer, and lifecycle checks based on the changed paths. Describe any check you could not run in the pull request.

## Pull requests

A reviewable PR:

- explains the problem and chosen approach;
- links its issue when one exists;
- identifies security, migration, compatibility, and rollback impact;
- includes exact validation performed;
- includes screenshots for visible UI changes;
- stays focused enough to review and revert safely;
- passes required CI before merge.

By submitting a contribution, you represent that you have the right to license it and agree to provide it under the GNU Affero General Public License v3.0 only.
