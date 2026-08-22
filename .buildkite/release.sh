#!/usr/bin/env bash
set -euo pipefail

action="${1:-}"
target="${2:-}"

ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"

release_sha="${BUILDKITE_COMMIT:-}"
if [[ "$action" != "gate" ]]; then
  [[ "$release_sha" =~ ^[0-9a-f]{40}$ ]] || {
    echo "BUILDKITE_COMMIT must be a full commit SHA for release actions" >&2
    exit 1
  }
fi

job_suffix="${BUILDKITE_JOB_ID:-local}"
job_suffix="${job_suffix//[^a-zA-Z0-9_.-]/-}"

command -v docker >/dev/null 2>&1 || { echo "Docker is required on the Buildkite agent" >&2; exit 1; }

ghcr_login() {
  : "${GHCR_USERNAME:?Set GHCR_USERNAME in Buildkite secrets/environment}"
  : "${GHCR_TOKEN:?Set GHCR_TOKEN in Buildkite secrets/environment}"
  printf '%s' "$GHCR_TOKEN" | docker login ghcr.io --username "$GHCR_USERNAME" --password-stdin >/dev/null
}

builder=""
cleanup_builder() {
  [[ -z "$builder" ]] || docker buildx rm "$builder" >/dev/null 2>&1 || true
}

create_builder() {
  builder="argus-buildkite-${job_suffix}"
  docker buildx rm "$builder" >/dev/null 2>&1 || true
  docker buildx create --name "$builder" --driver docker-container --use >/dev/null
  trap cleanup_builder EXIT
}

case "$action" in
  gate)
    docker run --rm -v "$ROOT:/workspace" -w /workspace rust:bookworm cargo metadata --locked --no-deps >/dev/null
    bash -n install.sh
    bash -n scripts/first-server-smoke.sh
    bash -n scripts/update-first-test.sh
    bash -n scripts/recover-interrupted-update.sh
    bash -n scripts/registry-login.sh
    bash -n scripts/uninstall.sh
    docker buildx bake --print >/dev/null

    cp deploy/compose/Caddyfile.template deploy/compose/Caddyfile
    envfile="$(mktemp)"
    trap 'rm -f "$envfile"' EXIT
    cat >"$envfile" <<'ENV'
ARGUS_REGISTRY=ghcr.io/noah-bozkurt
ARGUS_VERSION=test
ARGUS_DOMAIN=argus.example.test
ARGUS_CONTENT_DOMAIN=content.argus.example.test
ARGUS_POSTGRES_PASSWORD=x
ARGUS_WEB_API_TOKEN=x
ARGUS_WORKER_TOKEN=x
ARGUS_CONTENT_SYNC_TOKEN=x
PAYLOAD_SECRET=x
ARGUS_ORG_ID=00000000-0000-4000-8000-000000000001
ARGUS_USER_ID=00000000-0000-4000-8000-000000000002
ARGUS_SERVER_ID=00000000-0000-4000-8000-000000000003
ARGUS_GITHUB_TOKEN=
ARGUS_RUST_LOG=info
ENV
    docker compose --project-directory deploy/compose --env-file "$envfile" -f deploy/compose/compose.yaml config >/dev/null
    ;;

  build)
    case "$target" in
      web|content|control-api|worker|host-tools) ;;
      *) echo "usage: $0 build <web|content|control-api|worker|host-tools>" >&2; exit 2 ;;
    esac
    ghcr_login
    create_builder
    RELEASE_SHA="$release_sha" docker buildx bake --push "$target"
    ;;

  verify)
    ghcr_login
    for image in argus-web argus-content argus-control-api argus-worker argus-host-tools; do
      echo "Verifying ${image}:${release_sha}"
      docker buildx imagetools inspect "ghcr.io/noah-bozkurt/${image}:${release_sha}" >/dev/null
    done
    ;;

  deploy-installer)
    ghcr_login
    tmp="$(mktemp -d)"
    cid=""
    cleanup_installer() {
      [[ -z "$cid" ]] || docker rm -f "$cid" >/dev/null 2>&1 || true
      rm -rf "$tmp"
    }
    trap cleanup_installer EXIT

    image="ghcr.io/noah-bozkurt/argus-host-tools:${release_sha}"
    docker pull "$image" >/dev/null
    revision="$(docker image inspect "$image" --format '{{ index .Config.Labels "org.opencontainers.image.revision" }}')"
    [[ "$revision" == "$release_sha" ]]
    cid="$(docker create "$image")"
    docker cp "$cid:/out/argus-installer" "$tmp/argus-installer"
    chmod 0755 "$tmp/argus-installer"
    [[ -s "$tmp/argus-installer" ]]
    docker rm -f "$cid" >/dev/null
    cid=""

    docker run --rm \
      --user "$(id -u):$(id -g)" \
      -e HOME=/tmp \
      -e ARGUS_RELEASE_REVISION="$release_sha" \
      -e ARGUS_INSTALLER_BINARY=/input/argus-installer \
      -v "$ROOT:/workspace" -v "$tmp:/input:ro" -w /workspace \
      node:22-bookworm bash -lc '
        set -euo pipefail
        node apps/installer/build.mjs
        bash -n apps/installer/dist/install
        test -s apps/installer/dist/manifest.json
        test -s apps/installer/dist/bin/argus-installer-x86_64
      '

    if [[ -n "${CLOUDFLARE_API_TOKEN:-}" && -n "${CLOUDFLARE_ACCOUNT_ID:-}" ]]; then
      docker run --rm \
        --user "$(id -u):$(id -g)" \
        -e HOME=/tmp -e npm_config_cache=/tmp/npm-cache \
        -e CLOUDFLARE_API_TOKEN -e CLOUDFLARE_ACCOUNT_ID \
        -v "$ROOT:/workspace" -w /workspace \
        node:22-bookworm \
        npx --yes wrangler@3.90.0 pages deploy apps/installer/dist --project-name=argus-installer --branch=main
    else
      echo "Installer site built; Cloudflare credentials are not configured, so deployment was skipped."
    fi
    ;;

  promote)
    ghcr_login
    for image in argus-web argus-content argus-control-api argus-worker; do
      echo "Promoting ${image}:${release_sha} to main"
      docker buildx imagetools create --tag "ghcr.io/noah-bozkurt/${image}:main" "ghcr.io/noah-bozkurt/${image}:${release_sha}"
    done
    docker buildx imagetools create --tag ghcr.io/noah-bozkurt/argus-host-tools:main "ghcr.io/noah-bozkurt/argus-host-tools:${release_sha}"
    for image in argus-web argus-content argus-control-api argus-worker argus-host-tools; do
      docker buildx imagetools inspect "ghcr.io/noah-bozkurt/${image}:main" >/dev/null
    done
    ;;

  *)
    echo "usage: $0 <gate|build|verify|deploy-installer|promote> [target]" >&2
    exit 2
    ;;
esac
