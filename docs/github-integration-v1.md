# GitHub Integration V1

Argus integrates GitHub as a repository provider. GitHub remains the source of truth for code, pull requests, issues and CI; Argus stores project linkage plus a bounded metadata snapshot for project context.

## Project model

A repository link belongs to an organization and project and contains:

- provider (`github` in V1)
- owner and repository name
- canonical GitHub URL
- default branch and visibility
- latest default-branch commit
- open pull request and issue counts
- latest commit check summary
- sync warnings/status/timestamps

Projects and repository links do not require a Client.

## Provider boundary

GitHub HTTP behavior is isolated in `github_integration.rs` behind the `RepositoryProvider` interface. Other project/application code talks to repository links rather than scattering GitHub-specific API conditionals throughout Argus.

A future GitLab/Gitea adapter can implement the same conceptual provider boundary without changing the Project Workspace ownership model.

## Authentication

`ARGUS_GITHUB_TOKEN` is optional in V1:

- without it, only GitHub data available to unauthenticated requests can be synced;
- with it, the token is used only by the Control API process and is never written to PostgreSQL, repository snapshots, audit events or the browser;
- use a read-only credential scoped to only the repositories Argus needs.

The intended production follow-up is a GitHub App installation flow with short-lived installation access tokens instead of a long-lived shared token.

## Read permissions

The V1 provider only performs GET requests for repository metadata, the default branch, open pull requests, open issues and check runs. The intended GitHub App permission set is therefore read-only and limited to the corresponding repository metadata/contents, pull-request, issues and checks capabilities.

If optional PR/issues/check data is not permitted, the repository can still retain its core link and records a bounded warning in its snapshot. Repository/default-branch access is required for a successful initial link.

## Counts

V1 requests at most 100 open pull requests and 100 open issues per sync. If either list reaches that bound, `counts_truncated` is set and the UI displays the values with `+`. This avoids pretending a bounded snapshot is an exact total.

## CI summary

For the latest commit on the default branch:

- no checks -> `NONE`
- any queued/in-progress check -> `RUNNING`
- completed check with failure/timed-out/cancelled/action-required/stale conclusion -> `FAILURE`
- otherwise completed checks -> `SUCCESS`
- unavailable permission/API -> `UNAVAILABLE`

Argus does not reimplement GitHub Actions or retain build logs in this phase.

## Sync behavior

Linking performs an initial live metadata fetch before persisting the link. Manual sync updates the stored snapshot. A failed later sync preserves the previous snapshot and records `sync_status=ERROR` plus a bounded error string.

Every link, sync, failed sync and unlink operation emits both an audit entry and project-scoped domain event.

## Non-goals

- source browsing/editor
- branch creation
- PR/issue mutation
- workflow dispatch
- GitHub Actions log storage
- GitHub replacement
- OAuth/App installation UI (next authentication iteration)
