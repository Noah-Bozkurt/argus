#!/usr/bin/env bash
set -euo pipefail

run_rust="${ARGUS_CI_RUN_RUST:-false}"
run_database="${ARGUS_CI_RUN_DATABASE:-false}"

if [[ "$run_rust" == "true" ]]; then
  cargo fmt --all -- --check
  cargo test --workspace --locked
fi

if [[ "$run_database" != "true" ]]; then
  exit 0
fi

container="argus-ci-postgres-${GITHUB_RUN_ID}-${GITHUB_RUN_ATTEMPT}-${GITHUB_JOB}"
docker rm -f "$container" >/dev/null 2>&1 || true
cleanup() {
  docker rm -f "$container" >/dev/null 2>&1 || true
}
trap cleanup EXIT

docker run -d --name "$container" \
  -e POSTGRES_USER=argus \
  -e POSTGRES_PASSWORD=argus-test-password \
  -e POSTGRES_DB=argus \
  -p 127.0.0.1::5432 \
  postgres:16 >/dev/null

ready=false
for _ in $(seq 1 60); do
  if docker exec "$container" pg_isready -U argus -d argus >/dev/null 2>&1; then
    ready=true
    break
  fi
  if ! docker inspect "$container" --format '{{.State.Running}}' 2>/dev/null | grep -qx true; then
    docker logs "$container" >&2 || true
    exit 1
  fi
  sleep 1
done
if [[ "$ready" != "true" ]]; then
  docker logs "$container" >&2 || true
  exit 1
fi

export DATABASE_URL="postgresql://argus:argus-test-password@127.0.0.1:5432/argus"
export ARGUS_CONTROL_API_BIND=127.0.0.1:18080
export ARGUS_CONTROL_API_URL=http://127.0.0.1:18080
export ARGUS_WEB_API_TOKEN=0123456789abcdef0123456789abcdef
export ARGUS_WORKER_TOKEN=abcdef0123456789abcdef0123456789
export ARGUS_CONTENT_URL=http://127.0.0.1:9
export ARGUS_CONTENT_SYNC_TOKEN=11111111111111111111111111111111
export ARGUS_WORKER_POLL_SECONDS=1
export RUST_LOG=info

control_log="$RUNNER_TEMP/argus-control-api.log"
worker_log="$RUNNER_TEMP/argus-worker.log"
cargo run --locked -p control-api >"$control_log" 2>&1 &
control_pid=$!
worker_pid=''
cleanup_processes() {
  kill "$control_pid" ${worker_pid:-} >/dev/null 2>&1 || true
  cleanup
}
trap cleanup_processes EXIT

for _ in $(seq 1 300); do
  if curl -fsS http://127.0.0.1:18080/health >/dev/null 2>&1; then
    break
  fi
  if ! kill -0 "$control_pid" >/dev/null 2>&1; then
    cat "$control_log"
    exit 1
  fi
  sleep 1
done
curl -fsS http://127.0.0.1:18080/health >/dev/null || { cat "$control_log"; exit 1; }

psql() {
  docker exec -e PGPASSWORD=argus-test-password "$container" \
    psql -v ON_ERROR_STOP=1 -U argus -d argus "$@"
}

[[ "$(psql -Atc "SELECT indexprs IS NULL AND indpred IS NULL FROM pg_index WHERE indexrelid='background_jobs_dedupe_idx'::regclass")" == "t" ]]
psql -c "INSERT INTO organizations(id,name) VALUES('00000000-0000-4000-8000-000000000001','CI')"

cargo run --locked -p argus-worker >"$worker_log" 2>&1 &
worker_pid=$!
for _ in $(seq 1 60); do
  if [[ "$(psql -Atc "SELECT EXISTS(SELECT 1 FROM background_jobs WHERE dedupe_key IS NOT NULL)")" == "t" ]]; then
    exit 0
  fi
  if ! kill -0 "$worker_pid" >/dev/null 2>&1; then
    cat "$worker_log"
    exit 1
  fi
  sleep 1
done
cat "$worker_log"
exit 1
