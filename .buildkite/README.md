# Buildkite setup for Argus

Argus uses Buildkite as the active CI/CD definition while the GitHub Actions workflows are kept disabled during the Buildkite trial.

## Pipeline configuration

Create a Buildkite pipeline for `Noah-Bozkurt/argus` and use a self-hosted queue with the key:

```text
argus
```

All Argus agents should join that queue. Use Buildkite Agent 3.109 or newer because this pipeline uses list-form `if_changed` filters.

In the Buildkite YAML steps editor, use only the uploader step:

```yaml
steps:
  - label: ":pipeline: Upload"
    command: buildkite-agent pipeline upload
    agents:
      queue: "argus"
```

The uploader loads `.buildkite/pipeline.yml` from the checked-out revision.

## GitHub trigger settings

Recommended GitHub settings for this public repository:

- Enable pull request builds.
- Build branch pushes only for `main`.
- Disable **Allow builds from third-party forked repositories**.
- Enable commit status updates.
- Do not enable fork builds on the self-hosted Argus queue.

Disabling third-party fork builds is a security boundary: an external fork can modify the pipeline definition, so it must not be allowed to dispatch work to the self-hosted agents.

## Agent requirements

The `argus` agents need:

- Linux x86_64
- Bash and standard GNU utilities
- Git
- Docker Engine
- Docker Compose v2
- Docker Buildx

Rust, Node.js, pnpm, PostgreSQL and Caddy validation run in containers and do not need to be installed on the host.

Mounting `/var/run/docker.sock` into a containerized Buildkite agent gives build jobs control over the Docker host. Only use that setup for trusted Argus builds.

## Release secrets

The main-branch release steps require:

```text
GHCR_USERNAME
GHCR_TOKEN
```

`GHCR_TOKEN` needs permission to push packages to `ghcr.io/noah-bozkurt`.

Installer deployment additionally uses:

```text
CLOUDFLARE_API_TOKEN
CLOUDFLARE_ACCOUNT_ID
```

If the Cloudflare variables are missing, the installer artifact is still built and validated but the Pages deployment is skipped.

## Pipeline behavior

Pull requests run only affected validation jobs. Those jobs are independent and can be scheduled across multiple agents in parallel.

A push to `main` does not repeat the full PR suite. It runs the release gate, builds the five immutable images in parallel, verifies them, deploys the installer pages, and finally promotes the verified image tags to `main`.

The Docker bake cache uses GHCR registry cache manifests rather than GitHub Actions' `type=gha` cache backend so it works from Buildkite.
