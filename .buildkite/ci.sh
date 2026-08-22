#!/usr/bin/env bash
set -euo pipefail

mode="${1:-}"
[[ -n "$mode" ]] || { echo "usage: $0 <rust|web|content|installer|database|cms|acceptance|deployment>" >&2; exit 2; }

ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"
CACHE_DIR="${ARGUS_BUILDKITE_CACHE_DIR:-${HOME}/.cache/argus-buildkite}"
mkdir -p "$CACHE_DIR/cargo" "$CACHE_DIR/pnpm" "$CACHE_DIR/npm"
job_suffix="${BUILDKITE_JOB_ID:-local}"
job_suffix="${job_suffix//[^a-zA-Z0-9_.-]/-}"

command -v docker >/dev/null 2>&1 || { echo "Docker is required on the Buildkite agent" >&2; exit 1; }

rust_cmd() {
  local scope="$1"
  shift
  mkdir -p "$CACHE_DIR/target-$scope"
  docker run --rm \
    -e CARGO_HOME=/cache/cargo \
    -e CARGO_TARGET_DIR="/cache/target-$scope" \
    -v "$ROOT:/workspace" \
    -v "$CACHE_DIR:/cache" \
    -w /workspace \
    rust:bookworm bash -lc "$*"
}

node_cmd() {
  docker run --rm \
    --user "$(id -u):$(id -g)" \
    -e HOME=/tmp \
    -e npm_config_cache=/cache/npm \
    -v "$ROOT:/workspace" \
    -v "$CACHE_DIR:/cache" \
    -w /workspace \
    node:22-bookworm \
    bash -lc 'npx --yes pnpm@9 config set store-dir /cache/pnpm >/dev/null && npx --yes pnpm@9 install --frozen-lockfile && '"$*"
}

start_postgres() {
  local network="$1" container="$2"
  docker run -d --name "$container" \
    --network "$network" --network-alias postgres \
    -e POSTGRES_USER=argus \
    -e POSTGRES_PASSWORD=argus-test-password \
    -e POSTGRES_DB=argus \
    postgres:16 >/dev/null
  for _ in $(seq 1 60); do
    docker exec "$container" pg_isready -U argus -d argus >/dev/null 2>&1 && return 0
    sleep 1
  done
  docker logs "$container" >&2 || true
  return 1
}

case "$mode" in
  rust)
    rust_cmd rust 'rustup component add rustfmt >/dev/null && cargo fmt --all -- --check && cargo test --workspace --locked'
    ;;

  web)
    node_cmd 'npx --yes pnpm@9 --filter @argus/web exec tsc --noEmit'
    ;;

  content)
    node_cmd 'npx --yes pnpm@9 --filter @argus/content run typecheck && npx --yes pnpm@9 --filter @argus/content run test:cms-contract'
    ;;

  installer)
    node_cmd 'npx --yes pnpm@9 --filter @argus/installer test'
    ;;

  database)
    network="argus-db-${job_suffix}"
    postgres="argus-postgres-${job_suffix}"
    control="argus-control-${job_suffix}"
    worker="argus-worker-${job_suffix}"
    control_log="$(mktemp)"
    worker_log="$(mktemp)"
    cleanup() {
      docker rm -f "$worker" "$control" "$postgres" >/dev/null 2>&1 || true
      docker network rm "$network" >/dev/null 2>&1 || true
      rm -f "$control_log" "$worker_log"
    }
    trap cleanup EXIT

    docker network create "$network" >/dev/null
    start_postgres "$network" "$postgres"
    database_url='postgresql://argus:argus-test-password@postgres:5432/argus'
    mkdir -p "$CACHE_DIR/target-database"

    docker run --rm --name "$control" \
      --network "$network" --network-alias control \
      -e CARGO_HOME=/cache/cargo -e CARGO_TARGET_DIR=/cache/target-database \
      -e DATABASE_URL="$database_url" \
      -e ARGUS_CONTROL_API_BIND=0.0.0.0:18080 \
      -e ARGUS_CONTROL_API_URL=http://control:18080 \
      -e ARGUS_WEB_API_TOKEN=0123456789abcdef0123456789abcdef \
      -e ARGUS_WORKER_TOKEN=abcdef0123456789abcdef0123456789 \
      -e ARGUS_CONTENT_URL=http://127.0.0.1:9 \
      -e ARGUS_CONTENT_SYNC_TOKEN=11111111111111111111111111111111 \
      -e ARGUS_WORKER_POLL_SECONDS=1 -e RUST_LOG=info \
      -v "$ROOT:/workspace" -v "$CACHE_DIR:/cache" -w /workspace \
      rust:bookworm bash -lc 'cargo run --locked -p control-api' >"$control_log" 2>&1 &
    control_pid=$!

    for _ in $(seq 1 300); do
      if docker run --rm --network "$network" curlimages/curl:8.10.1 -fsS http://control:18080/health >/dev/null 2>&1; then break; fi
      if ! kill -0 "$control_pid" >/dev/null 2>&1; then cat "$control_log"; exit 1; fi
      sleep 1
    done
    docker run --rm --network "$network" curlimages/curl:8.10.1 -fsS http://control:18080/health >/dev/null || { cat "$control_log"; exit 1; }

    psql() {
      docker run --rm --network "$network" -e PGPASSWORD=argus-test-password postgres:16 \
        psql -v ON_ERROR_STOP=1 -h postgres -U argus -d argus "$@"
    }
    [[ "$(psql -Atc "SELECT indexprs IS NULL AND indpred IS NULL FROM pg_index WHERE indexrelid='background_jobs_dedupe_idx'::regclass")" == "t" ]]
    psql -c "INSERT INTO organizations(id,name) VALUES('00000000-0000-4000-8000-000000000001','CI')" >/dev/null

    docker run --rm --name "$worker" --network "$network" \
      -e CARGO_HOME=/cache/cargo -e CARGO_TARGET_DIR=/cache/target-database \
      -e DATABASE_URL="$database_url" \
      -e ARGUS_CONTROL_API_URL=http://control:18080 \
      -e ARGUS_WEB_API_TOKEN=0123456789abcdef0123456789abcdef \
      -e ARGUS_WORKER_TOKEN=abcdef0123456789abcdef0123456789 \
      -e ARGUS_CONTENT_URL=http://127.0.0.1:9 \
      -e ARGUS_CONTENT_SYNC_TOKEN=11111111111111111111111111111111 \
      -e ARGUS_WORKER_POLL_SECONDS=1 -e RUST_LOG=info \
      -v "$ROOT:/workspace" -v "$CACHE_DIR:/cache" -w /workspace \
      rust:bookworm bash -lc 'cargo run --locked -p argus-worker' >"$worker_log" 2>&1 &
    worker_pid=$!

    for _ in $(seq 1 60); do
      [[ "$(psql -Atc "SELECT EXISTS(SELECT 1 FROM background_jobs WHERE dedupe_key IS NOT NULL)")" == "t" ]] && exit 0
      if ! kill -0 "$worker_pid" >/dev/null 2>&1; then cat "$worker_log"; exit 1; fi
      sleep 1
    done
    cat "$worker_log"
    exit 1
    ;;

  cms)
    network="argus-cms-${job_suffix}"
    postgres="argus-cms-postgres-${job_suffix}"
    content_container="argus-content-${job_suffix}"
    cleanup() {
      docker rm -f "$content_container" "$postgres" >/dev/null 2>&1 || true
      docker network rm "$network" >/dev/null 2>&1 || true
    }
    trap cleanup EXIT

    docker network create "$network" >/dev/null
    start_postgres "$network" "$postgres"

    docker run -d --name "$content_container" \
      --user "$(id -u):$(id -g)" \
      --network "$network" --network-alias content \
      -e HOME=/tmp -e npm_config_cache=/cache/npm \
      -e DATABASE_URL=postgresql://argus:argus-test-password@postgres:5432/argus \
      -e ARGUS_CONTENT_DB_SCHEMA=argus_content \
      -e PAYLOAD_SECRET=payload-runtime-secret-0000000000 \
      -e PAYLOAD_PUBLIC_URL=http://content:3000 \
      -e ARGUS_CONTENT_SYNC_TOKEN=content-sync-runtime-000000000000 \
      -e ARGUS_MEDIA_DIR=/tmp/argus-content-media -e PAYLOAD_DB_PUSH=false \
      -v "$ROOT:/workspace" -v "$CACHE_DIR:/cache" -w /workspace \
      node:22-bookworm bash -lc '
        set -euo pipefail
        npx --yes pnpm@9 config set store-dir /cache/pnpm >/dev/null
        npx --yes pnpm@9 install --frozen-lockfile
        npx --yes pnpm@9 --filter @argus/content exec payload migrate
        exec npx --yes pnpm@9 --filter @argus/content dev
      ' >/dev/null

    for _ in $(seq 1 180); do
      if docker run --rm --network "$network" curlimages/curl:8.10.1 -fsS http://content:3000/healthz >/dev/null 2>&1; then
        docker exec "$content_container" bash scripts/test-native-cms-runtime.sh
        docker exec "$content_container" bash scripts/test-media-runtime.sh
        docker exec "$content_container" bash scripts/test-forms-runtime.sh
        docker exec \
          -e ARGUS_TEST_PROJECT_ID=00000000-0000-4000-8000-000000000003 \
          -e ARGUS_TEST_ORG_ID=00000000-0000-4000-8000-000000000001 \
          -e ARGUS_TEST_USER_ID=00000000-0000-4000-8000-000000000002 \
          "$content_container" node scripts/first-server-content-acceptance.mjs >/dev/null
        exit 0
      fi
      if [[ "$(docker inspect -f '{{.State.Running}}' "$content_container" 2>/dev/null || true)" != "true" ]]; then
        docker logs "$content_container" >&2 || true
        exit 1
      fi
      sleep 1
    done
    docker logs "$content_container" >&2 || true
    exit 1
    ;;

  acceptance)
    for script in \
      scripts/first-server-acceptance.sh \
      scripts/first-server-product-acceptance.sh \
      scripts/first-server-restore-acceptance.sh \
      scripts/first-server-reset-reinstall-acceptance.sh \
      scripts/reset-first-test.sh \
      scripts/test-first-server-acceptance.sh \
      scripts/test-first-server-restore-acceptance.sh \
      scripts/test-first-server-reset-reinstall-acceptance.sh \
      scripts/test-reset-first-test.sh; do
      bash -n "$script"
    done
    docker run --rm -v "$ROOT:/workspace" -w /workspace node:22-bookworm node --check scripts/first-server-content-acceptance.mjs
    bash scripts/test-first-server-acceptance.sh
    bash scripts/test-first-server-restore-acceptance.sh
    bash scripts/test-first-server-reset-reinstall-acceptance.sh
    bash scripts/test-reset-first-test.sh
    docker run --rm -v "$ROOT:/workspace" -w /workspace node:22-bookworm node --test scripts/first-server-content-acceptance.test.mjs
    ;;

  deployment)
    for script in install.sh scripts/first-server-smoke.sh scripts/test-native-cms-runtime.sh scripts/test-media-runtime.sh scripts/test-forms-runtime.sh scripts/update-first-test.sh scripts/recover-interrupted-update.sh scripts/registry-login.sh scripts/uninstall.sh; do
      bash -n "$script"
    done
    grep -Fq 'argus-installer' install.sh
    grep -Fq 'pg_dump' scripts/update-first-test.sh
    grep -Fq 'pg_restore' scripts/update-first-test.sh
    grep -Fq 'ROLLBACK_READY=1' scripts/update-first-test.sh
    grep -Fq 'TRANSACTION_FORMAT_VERSION=2' scripts/update-first-test.sh
    grep -Fq 'file-snapshot.sha256' scripts/update-first-test.sh
    grep -Fq 'database-snapshot.sha256' scripts/update-first-test.sh
    grep -Fq 'target-start-armed' scripts/update-first-test.sh
    grep -Fq 'ABORTED_PRE_MUTATION' scripts/recover-interrupted-update.sh
    grep -Fq 'ARGUS_UPDATE_DELEGATED_REVISION' crates/cli/src/main.rs
    grep -Fq 'org.argus.update-runner-protocol' docker-bake.hcl

    bash scripts/test-first-server-acceptance.sh
    bash scripts/test-first-server-restore-acceptance.sh
    bash scripts/test-first-server-reset-reinstall-acceptance.sh
    bash scripts/test-reset-first-test.sh

    tmp="$(mktemp -d)"
    trap 'rm -rf "$tmp"' EXIT
    cp deploy/compose/Caddyfile.template deploy/compose/Caddyfile
    cat >"$tmp/argus.env" <<'ENV'
ARGUS_REGISTRY=ghcr.io/noah-bozkurt
ARGUS_VERSION=test
ARGUS_DOMAIN=argus.example.test
ARGUS_CONTENT_DOMAIN=content.argus.example.test
ARGUS_POSTGRES_PASSWORD=test-postgres-password
ARGUS_WEB_API_TOKEN=0123456789abcdef0123456789abcdef
ARGUS_WORKER_TOKEN=abcdef0123456789abcdef0123456789
ARGUS_CONTENT_SYNC_TOKEN=11111111111111111111111111111111
PAYLOAD_SECRET=22222222222222222222222222222222
ARGUS_ORG_ID=00000000-0000-4000-8000-000000000001
ARGUS_USER_ID=00000000-0000-4000-8000-000000000002
ARGUS_SERVER_ID=00000000-0000-4000-8000-000000000003
ARGUS_GITHUB_TOKEN=
ARGUS_RUST_LOG=info
ENV
    docker compose --project-directory deploy/compose --env-file "$tmp/argus.env" -f deploy/compose/compose.yaml config >/dev/null

    hash="$(docker run --rm caddy:2-alpine caddy hash-password --plaintext test-password)"
    cp deploy/compose/Caddyfile.template "$tmp/Caddyfile"
    sed -i -e 's|__ARGUS_GLOBAL_OPTIONS__||g' -e 's|__ARGUS_DOMAIN__|argus.example.test|g' -e 's|__ARGUS_CONTENT_DOMAIN__|content.argus.example.test|g' -e 's|__ARGUS_TLS__|tls internal|g' -e 's|__BASIC_AUTH_USER__|argus|g' -e "s|__BASIC_AUTH_HASH__|${hash}|g" "$tmp/Caddyfile"
    grep -Fq 'path /public/status/*' "$tmp/Caddyfile"
    grep -Fq 'path /public/* /api/media/file/*' "$tmp/Caddyfile"
    docker run --rm -v "$tmp/Caddyfile:/etc/caddy/Caddyfile:ro" caddy:2-alpine caddy validate --config /etc/caddy/Caddyfile
    ;;

  *)
    echo "unknown CI mode: $mode" >&2
    exit 2
    ;;
esac
