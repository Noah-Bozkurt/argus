#!/usr/bin/env bash
set -Eeuo pipefail

INSTALL_DIR="${ARGUS_INSTALL_DIR:-/opt/argus}"
CONFIG_DIR="${ARGUS_CONFIG_DIR:-/etc/argus}"
STATE_DIR="${ARGUS_STATE_DIR:-/var/lib/argus}"
ENV_FILE="$INSTALL_DIR/.env"
COMPOSE_FILE="$INSTALL_DIR/compose.yaml"

PASS_COUNT=0

log() { printf '[argus-smoke] %s\n' "$*"; }
pass() { PASS_COUNT=$((PASS_COUNT + 1)); printf '[argus-smoke] ok: %s\n' "$*"; }
die() { printf '[argus-smoke] FAIL: %s\n' "$*" >&2; exit 1; }

require_root() {
  [[ "${EUID}" -eq 0 ]] || die "run as root so the smoke test can read the root-only Argus environment"
}

require_file() {
  [[ -f "$1" ]] || die "required file is missing: $1"
}

compose() {
  docker compose --project-directory "$INSTALL_DIR" --env-file "$ENV_FILE" -f "$COMPOSE_FILE" "$@"
}

psql_scalar() {
  compose exec -T postgres psql -v ON_ERROR_STOP=1 -U argus -d argus -Atc "$1"
}

verify_mode() {
  local path="$1" expected="$2" actual
  actual="$(stat -c '%a' "$path")"
  [[ "$actual" == "$expected" ]] || die "$path has mode $actual; expected $expected"
  pass "$path permissions are $expected"
}

verify_compose_service() {
  local service="$1" require_health="${2:-1}" cid running health
  cid="$(compose ps -q "$service")"
  [[ -n "$cid" ]] || die "Compose service '$service' has no container"
  running="$(docker inspect -f '{{.State.Running}}' "$cid")"
  [[ "$running" == "true" ]] || die "Compose service '$service' is not running"
  if [[ "$require_health" == "1" ]]; then
    health="$(docker inspect -f '{{if .State.Health}}{{.State.Health.Status}}{{else}}missing{{end}}' "$cid")"
    [[ "$health" == "healthy" ]] || die "Compose service '$service' health is '$health'"
    pass "Compose service $service is running and healthy"
  else
    pass "Compose service $service is running"
  fi
}

wait_until() {
  local description="$1" attempts="$2" delay="$3"
  shift 3
  for _ in $(seq 1 "$attempts"); do
    if "$@"; then
      pass "$description"
      return 0
    fi
    sleep "$delay"
  done
  die "$description did not become true in time"
}

agent_heartbeat_fresh() {
  [[ "$(psql_scalar "SELECT EXISTS(SELECT 1 FROM agents WHERE server_id='${ARGUS_SERVER_ID}'::uuid AND last_seen_at > NOW() - INTERVAL '3 minutes')")" == "t" ]]
}

required_schedules_exist() {
  [[ "$(psql_scalar "SELECT COUNT(*) FROM job_schedules WHERE organization_id='${ARGUS_ORG_ID}'::uuid AND enabled AND job_kind IN ('notifications.materialize','domains.lifecycle_evaluate','content.projects.sync')")" == "3" ]]
}

content_project_synced() {
  local table_exists
  table_exists="$(psql_scalar "SELECT to_regclass('argus_content.project_spaces') IS NOT NULL")"
  [[ "$table_exists" == "t" ]] || return 1
  [[ "$(psql_scalar "SELECT EXISTS(SELECT 1 FROM argus_content.project_spaces WHERE argus_project_id='${ARGUS_BOOTSTRAP_PROJECT_ID}')")" == "t" ]]
}

content_sync_job_succeeded() {
  [[ "$(psql_scalar "SELECT EXISTS(SELECT 1 FROM background_jobs WHERE organization_id='${ARGUS_ORG_ID}'::uuid AND job_kind='content.projects.sync' AND status='SUCCEEDED')")" == "t" ]]
}

verify_public_routing() {
  if [[ "${ARGUS_SMOKE_SKIP_PUBLIC_HTTPS:-0}" == "1" ]]; then
    log "public HTTPS checks skipped by ARGUS_SMOKE_SKIP_PUBLIC_HTTPS=1"
    return
  fi

  curl -fsS --connect-timeout 10 \
    -u "${ARGUS_BASIC_AUTH_USER}:${ARGUS_BASIC_AUTH_PASSWORD}" \
    "https://${ARGUS_DOMAIN}/healthz" >/dev/null \
    || die "authenticated Web HTTPS health failed"
  pass "operator Web is reachable over authenticated HTTPS"

  curl -fsS --connect-timeout 10 \
    -u "${ARGUS_BASIC_AUTH_USER}:${ARGUS_BASIC_AUTH_PASSWORD}" \
    "https://${ARGUS_CONTENT_DOMAIN}/healthz" >/dev/null \
    || die "authenticated Payload HTTPS health failed"
  pass "Payload is reachable over authenticated HTTPS"

  local status_code
  status_code="$(curl -sS --connect-timeout 10 -o /dev/null -w '%{http_code}' \
    "https://${ARGUS_DOMAIN}/public/status/argus-smoke-does-not-exist" || true)"
  [[ "$status_code" == "404" ]] \
    || die "public status route returned HTTP $status_code; expected unauthenticated 404 for a missing slug"
  pass "public status route bypasses operator basic auth"
}

main() {
  require_root
  command -v docker >/dev/null || die "docker is missing"
  command -v curl >/dev/null || die "curl is missing"
  command -v argusctl >/dev/null || die "argusctl is missing"
  require_file "$ENV_FILE"
  require_file "$COMPOSE_FILE"
  require_file "$CONFIG_DIR/agent.env"
  require_file "$CONFIG_DIR/helper.env"
  require_file "$STATE_DIR/agent.json"

  set -a
  # shellcheck disable=SC1090
  . "$ENV_FILE"
  set +a

  : "${ARGUS_ORG_ID:?missing ARGUS_ORG_ID}"
  : "${ARGUS_USER_ID:?missing ARGUS_USER_ID}"
  : "${ARGUS_BOOTSTRAP_PROJECT_ID:?missing ARGUS_BOOTSTRAP_PROJECT_ID}"
  : "${ARGUS_BOOTSTRAP_ENVIRONMENT_ID:?missing ARGUS_BOOTSTRAP_ENVIRONMENT_ID}"
  : "${ARGUS_SERVER_ID:?missing ARGUS_SERVER_ID}"
  : "${ARGUS_DOMAIN:?missing ARGUS_DOMAIN}"
  : "${ARGUS_CONTENT_DOMAIN:?missing ARGUS_CONTENT_DOMAIN}"
  : "${ARGUS_BASIC_AUTH_USER:?missing ARGUS_BASIC_AUTH_USER}"
  : "${ARGUS_BASIC_AUTH_PASSWORD:?missing ARGUS_BASIC_AUTH_PASSWORD}"

  log "validating deployed configuration"
  compose config >/dev/null
  pass "Compose configuration resolves"

  verify_mode "$ENV_FILE" 600
  verify_mode "$CONFIG_DIR/agent.env" 640
  verify_mode "$CONFIG_DIR/helper.env" 640
  verify_mode "$STATE_DIR/agent.json" 600

  [[ "$(stat -c '%G' "$CONFIG_DIR/agent.env")" == "argus" ]] || die "agent.env group is not argus"
  [[ "$(stat -c '%G' "$CONFIG_DIR/helper.env")" == "argus" ]] || die "helper.env group is not argus"
  pass "native service environment files are group-scoped to argus"

  log "validating runtime services"
  verify_compose_service postgres
  verify_compose_service control-api
  verify_compose_service worker
  verify_compose_service web
  verify_compose_service content
  verify_compose_service caddy 0

  systemctl is-active --quiet argus-helper.service || die "argus-helper.service is not active"
  pass "argus-helper.service is active"
  systemctl is-active --quiet argus-agent.service || die "argus-agent.service is not active"
  pass "argus-agent.service is active"

  [[ -S /run/argus/helper.sock ]] || die "Helper Unix socket is missing"
  [[ "$(stat -c '%a:%G' /run/argus/helper.sock)" == "660:argus" ]] \
    || die "Helper socket must be mode 660 and group argus"
  pass "Helper Unix socket boundary is correct"

  curl -fsS http://127.0.0.1:8080/health >/dev/null || die "Control API loopback health failed"
  pass "Control API loopback health responds"

  ss -ltnH | awk '{print $4}' | grep -Eq '^127\.0\.0\.1:8080$' \
    || die "Control API is not bound to host loopback on 127.0.0.1:8080"
  local postgres_cid postgres_host_bindings
  postgres_cid="$(compose ps -q postgres)"
  postgres_host_bindings="$(
    docker inspect -f '{{with index .HostConfig.PortBindings "5432/tcp"}}{{json .}}{{end}}' \
      "$postgres_cid"
  )"
  if [[ -n "$postgres_host_bindings" && "$postgres_host_bindings" != "null" ]]; then
    die "Argus PostgreSQL has a host port binding ($postgres_host_bindings); remove its Compose port mapping"
  fi
  pass "Control API is loopback-only and PostgreSQL is not host-exposed"

  argusctl health >/dev/null || die "argusctl health failed"
  pass "argusctl local health succeeds"
  argusctl connection >/dev/null || die "argusctl authenticated control-plane connection failed"
  pass "Agent credential authenticates to Control API"

  log "validating bootstrap data and background work"
  [[ "$(psql_scalar "SELECT EXISTS(SELECT 1 FROM organizations WHERE id='${ARGUS_ORG_ID}'::uuid)")" == "t" ]] \
    || die "bootstrap organization is missing"
  [[ "$(psql_scalar "SELECT EXISTS(SELECT 1 FROM users WHERE id='${ARGUS_USER_ID}'::uuid AND organization_id='${ARGUS_ORG_ID}'::uuid)")" == "t" ]] \
    || die "bootstrap operator user is missing"
  [[ "$(psql_scalar "SELECT EXISTS(SELECT 1 FROM projects WHERE id='${ARGUS_BOOTSTRAP_PROJECT_ID}'::uuid AND organization_id='${ARGUS_ORG_ID}'::uuid AND client_id IS NULL)")" == "t" ]] \
    || die "bootstrap project is missing or unexpectedly requires a Client"
  [[ "$(psql_scalar "SELECT EXISTS(SELECT 1 FROM environments WHERE id='${ARGUS_BOOTSTRAP_ENVIRONMENT_ID}'::uuid AND project_id='${ARGUS_BOOTSTRAP_PROJECT_ID}'::uuid AND is_protected)")" == "t" ]] \
    || die "protected bootstrap environment is missing"
  [[ "$(psql_scalar "SELECT EXISTS(SELECT 1 FROM servers WHERE id='${ARGUS_SERVER_ID}'::uuid AND project_id='${ARGUS_BOOTSTRAP_PROJECT_ID}'::uuid)")" == "t" ]] \
    || die "bootstrap server is missing"
  pass "project-centric bootstrap records exist without a Client"

  wait_until "Agent heartbeat is fresh" 30 2 agent_heartbeat_fresh
  wait_until "default background schedules exist" 10 2 required_schedules_exist
  wait_until "Payload project sync job succeeds" 60 2 content_sync_job_succeeded
  wait_until "bootstrap Project is mirrored into Payload" 30 2 content_project_synced

  verify_public_routing

  printf '\nArgus first-server smoke test passed: %d checks.\n' "$PASS_COUNT"
  printf 'Version: %s\n' "${ARGUS_VERSION:-unknown}"
  printf 'Server:  %s\n' "$ARGUS_SERVER_ID"
}

main "$@"
