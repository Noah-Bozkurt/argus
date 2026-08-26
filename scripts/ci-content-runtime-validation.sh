#!/usr/bin/env bash
set -euo pipefail

run_content="${ARGUS_CI_RUN_CONTENT:-false}"
run_cms="${ARGUS_CI_RUN_CMS:-false}"

if [[ "$run_content" == "true" ]]; then
  pnpm --filter @argus/content run typecheck
  pnpm --filter @argus/content run test:cms-contract
fi

if [[ "$run_cms" != "true" ]]; then
  exit 0
fi

container="argus-ci-postgres-${GITHUB_RUN_ID}-${GITHUB_RUN_ATTEMPT}-${GITHUB_JOB}"
docker rm -f "$container" >/dev/null 2>&1 || true
cleanup_postgres() {
  docker rm -f "$container" >/dev/null 2>&1 || true
}
trap cleanup_postgres EXIT

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
export ARGUS_CONTENT_DB_SCHEMA=argus_content
export PAYLOAD_SECRET=payload-runtime-secret-0000000000
export PAYLOAD_PUBLIC_URL=http://127.0.0.1:3000
export ARGUS_CONTENT_SYNC_TOKEN=content-sync-runtime-000000000000
export ARGUS_MEDIA_DIR="$RUNNER_TEMP/argus-content-media"
export PAYLOAD_DB_PUSH=false
export ARGUS_ORG_ID=00000000-0000-4000-8000-000000000001
export ARGUS_USER_ID=00000000-0000-4000-8000-000000000002
export ARGUS_OPERATOR_EMAIL=owner@argus.test
export ARGUS_OPERATOR_PASSWORD=argus-runtime-owner-password

pnpm --filter @argus/content exec payload migrate
content_log="$RUNNER_TEMP/argus-content.log"
pnpm --filter @argus/content dev >"$content_log" 2>&1 &
pid=$!
cleanup_all() {
  kill "$pid" >/dev/null 2>&1 || true
  cleanup_postgres
}
trap cleanup_all EXIT

for _ in $(seq 1 180); do
  if curl -fsS http://127.0.0.1:3000/healthz >/dev/null 2>&1; then
    bash scripts/test-native-cms-runtime.sh
    bash scripts/test-media-runtime.sh
    bash scripts/test-forms-runtime.sh
    ARGUS_TEST_PROJECT_ID=00000000-0000-4000-8000-000000000003 \
      ARGUS_TEST_ORG_ID="$ARGUS_ORG_ID" \
      ARGUS_TEST_USER_ID="$ARGUS_USER_ID" \
      node scripts/first-server-content-acceptance.mjs >/dev/null
    exit 0
  fi
  if ! kill -0 "$pid" >/dev/null 2>&1; then
    cat "$content_log"
    exit 1
  fi
  sleep 1
done
cat "$content_log"
exit 1
