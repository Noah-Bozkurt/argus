#!/usr/bin/env bash
set -Eeuo pipefail
umask 077

INSTALL_DIR="${ARGUS_INSTALL_DIR:-/opt/argus}"
STATE_DIR="${ARGUS_STATE_DIR:-/var/lib/argus}"
ENV_FILE="$INSTALL_DIR/.env"
COMPOSE_FILE="$INSTALL_DIR/compose.yaml"
ACCEPTANCE_DIR="${ARGUS_ACCEPTANCE_DIR:-$STATE_DIR/acceptance/first-server}"
CHECKPOINT_FILE="$ACCEPTANCE_DIR/product.env"
POST_REBOOT_CHECKPOINT="$ACCEPTANCE_DIR/post-reboot.env"
CONTROL_API_URL="${ARGUS_CONTROL_API_URL:-http://127.0.0.1:8080}"

log() { printf '[argus-product-acceptance] %s\n' "$*"; }
die() { printf '[argus-product-acceptance] error: %s\n' "$*" >&2; exit 1; }

require_root() { [[ "${EUID}" -eq 0 ]] || die "run as root (sudo -E ...)"; }

compose() {
  docker compose --project-directory "$INSTALL_DIR" --env-file "$ENV_FILE" -f "$COMPOSE_FILE" "$@"
}

psql_scalar() {
  compose exec -T postgres psql -v ON_ERROR_STOP=1 -U argus -d argus -Atc "$1"
}

api() {
  local method="$1" path="$2" body="${3:-}"
  local args=(-fsS -X "$method" -H "Authorization: Bearer $ARGUS_WEB_API_TOKEN" -H "X-Argus-Org-Id: $ARGUS_ORG_ID" -H "X-Argus-User-Id: $ARGUS_USER_ID")
  if [[ -n "$body" ]]; then
    args+=(-H 'Content-Type: application/json' --data "$body")
  fi
  curl "${args[@]}" "$CONTROL_API_URL$path"
}

wait_for_command() {
  local command_id="$1" expected="$2" attempts="${3:-60}" history status
  for _ in $(seq 1 "$attempts"); do
    history="$(api GET "/servers/$ARGUS_SERVER_ID/commands")"
    status="$(jq -r --arg id "$command_id" '.[] | select(.command.id == $id) | .command.status' <<<"$history")"
    if [[ "$status" == "$expected" ]]; then
      jq -c --arg id "$command_id" '.[] | select(.command.id == $id)' <<<"$history"
      return 0
    fi
    if [[ "$status" =~ ^(FAILED|EXPIRED|UNKNOWN)$ && "$status" != "$expected" ]]; then
      die "command $command_id reached $status while waiting for $expected"
    fi
    sleep 2
  done
  die "command $command_id did not reach $expected"
}

queue_command() {
  local command_type="$1" risk="$2" body
  body="$(jq -nc --arg server "$ARGUS_SERVER_ID" --argjson command "$command_type" --arg risk "$risk" --arg key "acceptance-$(cat /proc/sys/kernel/random/uuid)" '{server_id:$server,command_type:$command,ttl_seconds:300,idempotency_key:$key,risk_level:$risk}')"
  api POST /commands "$body"
}

file_checkpoint_value() {
  local file="$1" key="$2"
  [[ -f "$file" ]] || die "required acceptance checkpoint is missing: $file"
  (
    set +u
    # Root-owned acceptance checkpoint written with shell escaping.
    # shellcheck disable=SC1090
    . "$file"
    printf '%s\n' "${!key:-}"
  )
}

wait_for_job() {
  local kind="$1" resource="$2" not_before="${3:-}" attempts="${4:-90}" jobs
  for _ in $(seq 1 "$attempts"); do
    jobs="$(api GET /background-jobs)"
    if jq -e --arg kind "$kind" --arg resource "$resource" --arg not_before "$not_before" '
      .jobs | any(.job_kind == $kind and .resource_key == $resource and .status == "SUCCEEDED" and
        ($not_before == "" or (.completed_at != null and .completed_at >= $not_before)))
    ' <<<"$jobs" >/dev/null; then
      jq -cer --arg kind "$kind" --arg resource "$resource" --arg not_before "$not_before" '
        .jobs | map(select(.job_kind == $kind and .resource_key == $resource and .status == "SUCCEEDED" and
          ($not_before == "" or (.completed_at != null and .completed_at >= $not_before)))) | first
      ' <<<"$jobs"
      return 0
    fi
    sleep 2
  done
  die "scheduled job $kind/$resource did not succeed in time"
}

wait_for_backup() {
  local backup="$1" verified="$2" attempts="${3:-60}" servers
  for _ in $(seq 1 "$attempts"); do
    servers="$(api GET /servers)"
    if jq -e --arg server "$ARGUS_SERVER_ID" --arg backup "$backup" --argjson verified "$verified" '
      .[] | select(.server_id == $server) | .snapshot.backups.artifacts[]? |
      select(.name == $backup and .profile == "system-config" and .size_bytes > 0 and .sha256 != "" and .verified == $verified)
    ' <<<"$servers" >/dev/null; then
      return 0
    fi
    sleep 2
  done
  die "backup $backup did not reach verified=$verified in Agent inventory"
}

write_checkpoint() {
  local project_id="$1" environment_id="$2" service_id="$3" site_id="$4" safe_command_id="$5" protected_command_id="$6"
  local monitor_job_id="$7" backup_name="$8" backup_command_id="$9" verify_command_id="${10}" preflight_command_id="${11}"
  install -d -m 0700 "$ACCEPTANCE_DIR"
  local tmp="$CHECKPOINT_FILE.tmp.$$"
  {
    printf 'COMPLETED_AT=%q\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
    printf 'PROJECT_ID=%q\n' "$project_id"
    printf 'ENVIRONMENT_ID=%q\n' "$environment_id"
    printf 'SERVICE_ID=%q\n' "$service_id"
    printf 'SITE_ID=%q\n' "$site_id"
    printf 'SAFE_COMMAND_ID=%q\n' "$safe_command_id"
    printf 'PROTECTED_COMMAND_ID=%q\n' "$protected_command_id"
    printf 'MONITOR_JOB_ID=%q\n' "$monitor_job_id"
    printf 'BACKUP_NAME=%q\n' "$backup_name"
    printf 'BACKUP_COMMAND_ID=%q\n' "$backup_command_id"
    printf 'VERIFY_COMMAND_ID=%q\n' "$verify_command_id"
    printf 'PREFLIGHT_COMMAND_ID=%q\n' "$preflight_command_id"
  } >"$tmp"
  chmod 0600 "$tmp"
  sync -f "$tmp"
  mv "$tmp" "$CHECKPOINT_FILE"
  sync -f "$ACCEPTANCE_DIR"
}

main() {
  require_root
  command -v curl >/dev/null || die "curl is missing"
  command -v jq >/dev/null || die "jq is missing"
  [[ -f "$ENV_FILE" ]] || die "installed environment is missing: $ENV_FILE"
  [[ -f "$COMPOSE_FILE" ]] || die "installed Compose file is missing: $COMPOSE_FILE"

  set -a
  # shellcheck disable=SC1090
  . "$ENV_FILE"
  set +a
  : "${ARGUS_WEB_API_TOKEN:?missing ARGUS_WEB_API_TOKEN}"
  : "${ARGUS_ORG_ID:?missing ARGUS_ORG_ID}"
  : "${ARGUS_USER_ID:?missing ARGUS_USER_ID}"
  : "${ARGUS_SERVER_ID:?missing ARGUS_SERVER_ID}"
  : "${ARGUS_DOMAIN:?missing ARGUS_DOMAIN}"

  local suffix project project_id environment environment_id service service_id site site_id workspace
  suffix="$(date -u +%Y%m%d%H%M%S)-${RANDOM}"
  log "creating a new personal project through the Control API"
  project="$(api POST /projects "$(jq -nc --arg name "Personal acceptance $suffix" '{name:$name,description:"Created by first-server product acceptance",preset:"website",tags:["acceptance","personal"]}')")"
  project_id="$(jq -er 'select(.client_id == null) | .id' <<<"$project")" || die "new Project requires or contains a Client"

  environment="$(api POST "/projects/$project_id/environments" '{"name":"Production","environment_type":"production","description":"Acceptance environment","is_protected":true}')"
  environment_id="$(jq -er '.id' <<<"$environment")"
  service="$(api POST "/projects/$project_id/services" "$(jq -nc --arg environment "$environment_id" '{name:"Acceptance web",description:"Acceptance service",service_type:"web",runtime:"container",environment_id:$environment}')")"
  service_id="$(jq -er '.id' <<<"$service")"
  site="$(api POST "/projects/$project_id/sites" "$(jq -nc --arg environment "$environment_id" --arg service "$service_id" '{name:"Acceptance site",description:"Acceptance site",service_id:$service,environment_id:$environment,framework:"other"}')")"
  site_id="$(jq -er '.id' <<<"$site")"

  local reboot_completed jobs monitor_job monitor_job_id
  reboot_completed="$(file_checkpoint_value "$POST_REBOOT_CHECKPOINT" COMPLETED_AT)"
  [[ "$reboot_completed" =~ ^[0-9]{4}-[0-9]{2}-[0-9]{2}T ]] || die "post-reboot checkpoint time is invalid"
  log "proving a persisted default schedule executed after the recorded reboot"
  wait_for_job notifications.materialize default "$reboot_completed" 90 >/dev/null

  api PUT "/projects/$project_id/sites/$site_id/monitor" "$(jq -nc --arg target "https://$ARGUS_DOMAIN/healthz" '{target_url:$target,check_robots:false,check_sitemap:false,timeout_seconds:10}')" >/dev/null
  api PUT "/projects/$project_id/sites/$site_id/monitor/schedule" '{"enabled":true,"interval_seconds":60}' >/dev/null
  monitor_job="$(wait_for_job site_monitor.check "$site_id" '' 90)"
  monitor_job_id="$(jq -er '.id' <<<"$monitor_job")"

  workspace="$(api GET "/projects/$project_id")"
  jq -e --arg id "$project_id" '.project.id == $id and .project.client_id == null and ([.activity[].event_type] | index("project.created") != null)' <<<"$workspace" >/dev/null || die "Project workspace does not expose its creation event"
  [[ "$(psql_scalar "SELECT EXISTS(SELECT 1 FROM audit_events WHERE organization_id='${ARGUS_ORG_ID}'::uuid AND resource='${project_id}' AND action='project.created')")" == t ]] || die "project.created audit event was not persisted"
  [[ "$(psql_scalar "SELECT EXISTS(SELECT 1 FROM domain_events WHERE organization_id='${ARGUS_ORG_ID}'::uuid AND resource_id='${project_id}'::uuid AND event_type='project.created')")" == t ]] || die "project.created domain event was not persisted"

  local synced=0
  for _ in $(seq 1 60); do
    if [[ "$(psql_scalar "SELECT EXISTS(SELECT 1 FROM argus_content.project_spaces WHERE argus_project_id='${project_id}' AND client_id IS NULL)")" == t ]]; then
      synced=1
      break
    fi
    sleep 2
  done
  [[ "$synced" == 1 ]] || die "new personal Project was not synchronized to Payload"

  local servers safe command_id
  servers="$(api GET /servers)"
  jq -e --arg id "$ARGUS_SERVER_ID" '.[] | select(.server_id == $id) | .snapshot.docker.available == true' <<<"$servers" >/dev/null || die "server inventory does not contain the managed server with Docker inventory"
  safe="$(queue_command '{"kind":"service.status","service":"argus-agent.service"}' LOW)"
  command_id="$(jq -er '.id' <<<"$safe")"
  wait_for_command "$command_id" SUCCEEDED >/dev/null

  local protected_container protected protected_id protected_result
  protected_container="$(compose ps -q control-api)"
  [[ -n "$protected_container" ]] || die "could not identify protected control-api container"
  protected="$(queue_command "$(jq -nc --arg container "$protected_container" '{kind:"docker.restart",container:$container}')" MEDIUM)"
  protected_id="$(jq -er '.id' <<<"$protected")"
  protected_result="$(wait_for_command "$protected_id" FAILED)"
  jq -e '.error_code == "PERMISSION_DENIED" and (.error_message | contains("protected"))' <<<"$protected_result" >/dev/null || die "protected control-plane Docker action failed without the expected protection error"
  [[ "$(docker inspect -f '{{.State.Running}}' "$protected_container")" == true ]] || die "protected control-api container is no longer running"

  local backup_create backup_command_id backup_name backup_verify verify_command_id preflight preflight_command_id
  log "creating and verifying a system-config backup through typed Agent commands"
  backup_create="$(queue_command '{"kind":"backup.create","profile":"system-config"}' MEDIUM)"
  backup_command_id="$(jq -er '.id' <<<"$backup_create")"
  wait_for_command "$backup_command_id" SUCCEEDED 120 >/dev/null
  backup_name="$backup_command_id.tar.gz"
  wait_for_backup "$backup_name" false 60

  backup_verify="$(queue_command "$(jq -nc --arg backup "$backup_name" '{kind:"backup.verify",backup:$backup}')" LOW)"
  verify_command_id="$(jq -er '.id' <<<"$backup_verify")"
  wait_for_command "$verify_command_id" SUCCEEDED 120 >/dev/null
  wait_for_backup "$backup_name" true 60

  log "running non-mutating restore preflight for the verified backup"
  preflight="$(queue_command "$(jq -nc --arg backup "$backup_name" '{kind:"backup.restore.preflight",backup:$backup}')" LOW)"
  preflight_command_id="$(jq -er '.id' <<<"$preflight")"
  wait_for_command "$preflight_command_id" SUCCEEDED 120 >/dev/null

  write_checkpoint "$project_id" "$environment_id" "$service_id" "$site_id" "$command_id" "$protected_id" "$monitor_job_id" "$backup_name" "$backup_command_id" "$verify_command_id" "$preflight_command_id"
  log "product acceptance passed for personal Project $project_id"
}

if [[ "${1:-}" != "--internal-test-library" ]]; then
  main "$@"
fi
