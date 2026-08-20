#!/usr/bin/env bash
set -Eeuo pipefail
umask 077

INSTALL_DIR="${ARGUS_INSTALL_DIR:-/opt/argus}"
STATE_DIR="${ARGUS_STATE_DIR:-/var/lib/argus}"
ENV_FILE="$INSTALL_DIR/.env"
ACCEPTANCE_DIR="${ARGUS_ACCEPTANCE_DIR:-$STATE_DIR/acceptance/first-server}"
PRODUCT_CHECKPOINT="$ACCEPTANCE_DIR/product.env"
CHECKPOINT_FILE="$ACCEPTANCE_DIR/restore.env"
CONTROL_API_URL="${ARGUS_CONTROL_API_URL:-http://127.0.0.1:8080}"
DENIAL_FILE=""
MAINTENANCE_ACTIVE=0

log() { printf '[argus-restore-acceptance] %s\n' "$*"; }
die() { printf '[argus-restore-acceptance] error: %s\n' "$*" >&2; exit 1; }

require_root() { [[ "${EUID}" -eq 0 ]] || die "run as root (sudo -E ...)"; }

cleanup() {
  [[ -z "$DENIAL_FILE" ]] || rm -f "$DENIAL_FILE"
  if [[ "$MAINTENANCE_ACTIVE" == 1 ]]; then
    api POST "/servers/$ARGUS_SERVER_ID/maintenance/end" >/dev/null || true
  fi
}

checkpoint_value() {
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

api() {
  local method="$1" path="$2" body="${3:-}"
  local args=(-fsS -X "$method" -H "Authorization: Bearer $ARGUS_WEB_API_TOKEN" -H "X-Argus-Org-Id: $ARGUS_ORG_ID" -H "X-Argus-User-Id: $ARGUS_USER_ID")
  if [[ -n "$body" ]]; then
    args+=(-H 'Content-Type: application/json' --data "$body")
  fi
  curl "${args[@]}" "$CONTROL_API_URL$path"
}

api_status() {
  local method="$1" path="$2" body="$3" output="$4"
  curl -sS -o "$output" -w '%{http_code}' -X "$method" \
    -H "Authorization: Bearer $ARGUS_WEB_API_TOKEN" \
    -H "X-Argus-Org-Id: $ARGUS_ORG_ID" -H "X-Argus-User-Id: $ARGUS_USER_ID" \
    -H 'Content-Type: application/json' --data "$body" "$CONTROL_API_URL$path"
}

command_request() {
  local backup="$1" key="$2"
  jq -nc --arg server "$ARGUS_SERVER_ID" --arg backup "$backup" --arg key "$key" \
    '{server_id:$server,command_type:{kind:"backup.restore.apply",backup:$backup},ttl_seconds:600,idempotency_key:$key,risk_level:"CRITICAL"}'
}

wait_for_command() {
  local command_id="$1" attempts="${2:-180}" history status
  for _ in $(seq 1 "$attempts"); do
    history="$(api GET "/servers/$ARGUS_SERVER_ID/commands")"
    status="$(jq -r --arg id "$command_id" '.[] | select(.command.id == $id) | .command.status' <<<"$history")"
    if [[ "$status" == SUCCEEDED ]]; then
      return 0
    fi
    if [[ "$status" =~ ^(FAILED|EXPIRED|UNKNOWN)$ ]]; then
      die "restore command $command_id reached $status"
    fi
    sleep 2
  done
  die "restore command $command_id did not succeed in time"
}

wait_for_commit() {
  local restore_id="$1" attempts="${2:-60}" transaction
  transaction="$STATE_DIR/restores/$restore_id"
  for _ in $(seq 1 "$attempts"); do
    if [[ ! -e "$transaction" ]] && ! systemctl is-active --quiet "argus-restore-rollback-$restore_id.timer"; then
      return 0
    fi
    sleep 2
  done
  die "restore $restore_id succeeded but its timed rollback was not disarmed"
}

write_checkpoint() {
  local backup="$1" command_id="$2" maintenance_id="$3"
  install -d -m 0700 "$ACCEPTANCE_DIR"
  local tmp="$CHECKPOINT_FILE.tmp.$$"
  {
    printf 'COMPLETED_AT=%q\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
    printf 'BACKUP_NAME=%q\n' "$backup"
    printf 'RESTORE_COMMAND_ID=%q\n' "$command_id"
    printf 'MAINTENANCE_WINDOW_ID=%q\n' "$maintenance_id"
    printf 'MAINTENANCE_GATE_VERIFIED=yes\n'
    printf 'TIMED_ROLLBACK_DISARMED=yes\n'
    printf 'POST_RESTORE_SMOKE=yes\n'
  } >"$tmp"
  chmod 0600 "$tmp"
  sync -f "$tmp"
  mv "$tmp" "$CHECKPOINT_FILE"
  sync -f "$ACCEPTANCE_DIR"
}

main() {
  require_root
  [[ "${ARGUS_CONFIRM_TRANSACTIONAL_RESTORE:-}" == "RESTORE-DISPOSABLE-HOST" ]] \
    || die "set ARGUS_CONFIRM_TRANSACTIONAL_RESTORE=RESTORE-DISPOSABLE-HOST on a disposable host"
  [[ -f "$ENV_FILE" ]] || die "installed environment is missing: $ENV_FILE"
  set -a
  # shellcheck disable=SC1090
  . "$ENV_FILE"
  set +a
  : "${ARGUS_WEB_API_TOKEN:?missing ARGUS_WEB_API_TOKEN}"
  : "${ARGUS_ORG_ID:?missing ARGUS_ORG_ID}"
  : "${ARGUS_USER_ID:?missing ARGUS_USER_ID}"
  : "${ARGUS_SERVER_ID:?missing ARGUS_SERVER_ID}"

  local backup request denial_status maintenance maintenance_id command command_id
  backup="$(checkpoint_value "$PRODUCT_CHECKPOINT" BACKUP_NAME)"
  [[ "$backup" =~ ^[0-9a-f-]{36}\.tar\.gz$ ]] || die "product backup evidence is invalid"
  request="$(command_request "$backup" "acceptance-restore-denied-$(cat /proc/sys/kernel/random/uuid)")"
  DENIAL_FILE="$(mktemp)"
  trap cleanup EXIT

  log "proving transactional restore is rejected outside maintenance"
  denial_status="$(api_status POST /commands "$request" "$DENIAL_FILE")"
  [[ "$denial_status" == 409 ]] || die "restore without maintenance returned HTTP $denial_status"
  jq -e '.code == "MAINTENANCE_REQUIRED"' "$DENIAL_FILE" >/dev/null \
    || die "restore without maintenance did not return MAINTENANCE_REQUIRED"

  maintenance="$(api POST "/servers/$ARGUS_SERVER_ID/maintenance/start" \
    '{"duration_minutes":15,"reason":"Disposable first-server transactional restore acceptance"}')"
  maintenance_id="$(jq -er .id <<<"$maintenance")"
  MAINTENANCE_ACTIVE=1

  log "applying the verified backup through the typed restore transaction"
  request="$(command_request "$backup" "acceptance-restore-$(cat /proc/sys/kernel/random/uuid)")"
  command="$(api POST /commands "$request")"
  command_id="$(jq -er .id <<<"$command")"
  wait_for_command "$command_id" 180
  wait_for_commit "$command_id" 60
  /usr/local/bin/argusctl smoke
  api POST "/servers/$ARGUS_SERVER_ID/maintenance/end" >/dev/null
  MAINTENANCE_ACTIVE=0
  write_checkpoint "$backup" "$command_id" "$maintenance_id"
  log "transactional restore acceptance passed for $backup"
}

if [[ "${1:-}" != "--internal-test-library" ]]; then
  main "$@"
fi
