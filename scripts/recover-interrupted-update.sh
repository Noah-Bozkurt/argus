#!/usr/bin/env bash
set -Eeuo pipefail
umask 077

INSTALL_DIR="${ARGUS_INSTALL_DIR:-/opt/argus}"
STATE_DIR="${ARGUS_STATE_DIR:-/var/lib/argus}"
BACKUP_ROOT="$STATE_DIR/update-backups"
LOCK_FILE="$STATE_DIR/update.lock"
ENV_FILE="$INSTALL_DIR/.env"
COMPOSE_FILE="$INSTALL_DIR/compose.yaml"
CADDY_FILE="$INSTALL_DIR/Caddyfile"
PRESTART_MODE="${ARGUS_UPDATE_RECOVERY_PRESTART:-0}"
RETRY_FAILED="${ARGUS_UPDATE_RECOVERY_RETRY_FAILED:-0}"
PRE_RECOVERY_VERSION=""
TRANSACTION_FORMAT=1
FROM_REVISION=""
TO_REVISION=""

log() { printf '[argus-recovery] %s\n' "$*"; }
warn() { printf '[argus-recovery] warning: %s\n' "$*" >&2; }
die() { printf '[argus-recovery] error: %s\n' "$*" >&2; exit 1; }

require_root() {
  [[ "${EUID}" -eq 0 ]] || die "interrupted update recovery must run as root"
}

valid_revision() {
  [[ "$1" =~ ^[0-9a-f]{40}$ ]]
}

acquire_recovery_lock() {
  [[ -d "$STATE_DIR" ]] || die "Argus state directory is missing: $STATE_DIR"
  exec 9>"$LOCK_FILE"
  if ! flock -n 9; then
    # A live updater owns the same lock. This is expected when the Helper is
    # restarted near the end of a normal update transaction.
    log "active update owns the lifecycle lock; skipping recovery"
    exit 0
  fi
}

durable_write_text() {
  local path="$1"
  local value="$2"
  local tmp="${path}.tmp.$$"
  printf '%s\n' "$value" >"$tmp"
  chmod 0600 "$tmp"
  sync -f "$tmp"
  mv "$tmp" "$path"
  sync -f "$(dirname "$path")"
}

write_transaction_result() {
  local transaction="$1" result="$2"
  durable_write_text "$transaction/result" "$result"
}

find_recovery_transaction() {
  [[ -d "$BACKUP_ROOT" ]] || return 1

  local entry dir result
  while IFS= read -r entry; do
    dir="${entry#* }"
    [[ -f "$dir/metadata.env" ]] || continue
    result="$(cat "$dir/result" 2>/dev/null || true)"
    case "$result" in
      SUCCEEDED|ROLLED_BACK|ABORTED_PRE_MUTATION)
        continue
        ;;
      ROLLBACK_FAILED)
        if [[ "$RETRY_FAILED" == "1" ]]; then
          printf '%s\n' "$dir"
          return 0
        fi
        printf '[argus-recovery] error: unresolved failed update rollback blocks automatic startup: %s\n' "$dir" >&2
        printf '[argus-recovery] error: inspect the transaction, then run sudo argusctl recover-update --retry-failed to explicitly retry it\n' >&2
        return 2
        ;;
      *)
        printf '%s\n' "$dir"
        return 0
        ;;
    esac
  done < <(find "$BACKUP_ROOT" -mindepth 1 -maxdepth 1 -type d -printf '%T@ %p\n' | sort -nr)

  return 1
}

load_transaction_metadata() {
  local transaction="$1"
  TRANSACTION_FORMAT=1
  FROM_REVISION=""
  TO_REVISION=""
  # shellcheck disable=SC1090
  . "$transaction/metadata.env"
  : "${FROM_REVISION:?transaction is missing FROM_REVISION}"
  : "${TO_REVISION:?transaction is missing TO_REVISION}"
  valid_revision "$FROM_REVISION" || die "invalid FROM_REVISION in $transaction"
  valid_revision "$TO_REVISION" || die "invalid TO_REVISION in $transaction"
  case "$TRANSACTION_FORMAT" in
    1|2) ;;
    *) die "unsupported update transaction format '$TRANSACTION_FORMAT' in $transaction" ;;
  esac
}

capture_pre_recovery_version() {
  [[ -f "$ENV_FILE" ]] || die "installed environment is missing during interrupted update recovery"
  PRE_RECOVERY_VERSION="$(awk -F= '$1 == "ARGUS_VERSION" { print $2; exit }' "$ENV_FILE")"
  case "$PRE_RECOVERY_VERSION" in
    "$FROM_REVISION"|"$TO_REVISION") ;;
    *) die "installed ARGUS_VERSION '$PRE_RECOVERY_VERSION' matches neither transaction revision" ;;
  esac
  log "interrupted runtime persisted revision: $PRE_RECOVERY_VERSION"
}

file_snapshot_paths() {
  cat <<'EOF'
files/install/.env
files/install/compose.yaml
files/install/Caddyfile
files/install/Caddyfile.template
files/bin/argus-agent
files/bin/argus-helper
files/bin/argusctl
files/systemd/argus-agent.service
files/systemd/argus-helper.service
EOF
}

legacy_file_snapshot_complete() {
  local transaction="$1" path
  while IFS= read -r path; do
    [[ -f "$transaction/$path" ]] || return 1
  done < <(file_snapshot_paths)
}

verify_file_snapshot() {
  local transaction="$1"
  local manifest="$transaction/file-snapshot.sha256"
  [[ -s "$manifest" ]] || return 1

  local expected actual
  expected="$(file_snapshot_paths | LC_ALL=C sort)"
  actual="$(awk '{ print $2 }' "$manifest" | LC_ALL=C sort)"
  [[ "$actual" == "$expected" ]] || return 1

  (
    cd "$transaction"
    sha256sum -c file-snapshot.sha256 >/dev/null 2>&1
  )
}

prepare_transaction_recovery() {
  local transaction="$1"

  if [[ "$TRANSACTION_FORMAT" == "2" ]]; then
    if [[ ! -f "$transaction/file-snapshot.sha256" ]]; then
      capture_pre_recovery_version
      [[ "$PRE_RECOVERY_VERSION" == "$FROM_REVISION" ]] \
        || die "format-2 transaction lacks a sealed file snapshot after the target revision was persisted"
      write_transaction_result "$transaction" ABORTED_PRE_MUTATION
      log "transaction ended before its rollback snapshot was sealed; no live mutation had been armed"
      return 1
    fi

    verify_file_snapshot "$transaction" \
      || die "sealed pre-update file snapshot failed checksum verification: $transaction"
    return 0
  fi

  if legacy_file_snapshot_complete "$transaction"; then
    return 0
  fi

  capture_pre_recovery_version
  [[ "$PRE_RECOVERY_VERSION" == "$FROM_REVISION" ]] \
    || die "legacy transaction has an incomplete rollback snapshot after the target revision was persisted"
  write_transaction_result "$transaction" ABORTED_PRE_MUTATION
  log "legacy transaction ended before a complete rollback snapshot existed; no target revision was persisted"
  return 1
}

load_installed_env_if_available() {
  if [[ -f "$ENV_FILE" ]]; then
    set -a
    # shellcheck disable=SC1090
    . "$ENV_FILE"
    set +a
  fi
}

compose_current() {
  docker compose --project-directory "$INSTALL_DIR" --env-file "$ENV_FILE" -f "$COMPOSE_FILE" "$@"
}

stop_current_runtime_best_effort() {
  systemctl stop argus-agent.service >/dev/null 2>&1 || true
  if [[ "$PRESTART_MODE" != "1" ]]; then
    systemctl stop argus-helper.service >/dev/null 2>&1 || true
  fi

  if [[ -f "$ENV_FILE" && -f "$COMPOSE_FILE" ]]; then
    load_installed_env_if_available
    compose_current stop caddy worker web content control-api >/dev/null 2>&1 || true
  fi
}

restore_installed_files() {
  local transaction="$1"
  local backup="$transaction/files"

  if [[ "$TRANSACTION_FORMAT" == "2" ]]; then
    verify_file_snapshot "$transaction" \
      || die "pre-update file snapshot changed before restore"
  else
    legacy_file_snapshot_complete "$transaction" \
      || die "legacy rollback snapshot is incomplete"
  fi

  log "restoring pre-update deployment files and native binaries"
  cp -a "$backup/install/.env" "$ENV_FILE"
  cp -a "$backup/install/compose.yaml" "$COMPOSE_FILE"
  cp -a "$backup/install/Caddyfile" "$CADDY_FILE"
  cp -a "$backup/install/Caddyfile.template" "$INSTALL_DIR/Caddyfile.template"
  cp -a "$backup/bin/argus-agent" /usr/local/bin/argus-agent
  cp -a "$backup/bin/argus-helper" /usr/local/bin/argus-helper
  cp -a "$backup/bin/argusctl" /usr/local/bin/argusctl
  cp -a "$backup/systemd/argus-agent.service" /etc/systemd/system/argus-agent.service
  cp -a "$backup/systemd/argus-helper.service" /etc/systemd/system/argus-helper.service
  chmod 0600 "$ENV_FILE"
  systemctl daemon-reload

  set -a
  # shellcheck disable=SC1090
  . "$ENV_FILE"
  set +a

  [[ "${ARGUS_VERSION:-}" == "$FROM_REVISION" ]] \
    || die "rollback snapshot version '${ARGUS_VERSION:-missing}' does not match $FROM_REVISION"
}

wait_postgres() {
  compose_current up -d postgres >/dev/null
  for _ in $(seq 1 90); do
    if compose_current exec -T postgres pg_isready -U argus -d postgres >/dev/null 2>&1; then
      return 0
    fi
    sleep 1
  done
  return 1
}

database_snapshot_checksum_valid() {
  local transaction="$1"
  local manifest="$transaction/database-snapshot.sha256"
  [[ -s "$transaction/argus.dump" && -s "$manifest" ]] || return 1
  [[ "$(awk '{ print $2 }' "$manifest")" == "argus.dump" ]] || return 1
  (
    cd "$transaction"
    sha256sum -c database-snapshot.sha256 >/dev/null 2>&1
  )
}

database_snapshot_is_readable() {
  local transaction="$1"
  [[ -s "$transaction/argus.dump" ]] || return 1

  if [[ "$TRANSACTION_FORMAT" == "2" ]]; then
    database_snapshot_checksum_valid "$transaction" || return 1
  fi

  compose_current exec -T postgres pg_restore --list <"$transaction/argus.dump" >/dev/null 2>&1
}

target_start_was_armed() {
  local transaction="$1"

  if [[ "$TRANSACTION_FORMAT" == "1" ]]; then
    [[ "$PRE_RECOVERY_VERSION" == "$TO_REVISION" ]]
    return
  fi

  [[ -f "$transaction/target-start-armed" ]] || return 1
  local armed_revision
  armed_revision="$(cat "$transaction/target-start-armed")"
  [[ "$armed_revision" == "$TO_REVISION" ]] \
    || die "target-start marker contains unexpected revision '$armed_revision'"
  return 0
}

restore_database_if_needed() {
  local transaction="$1"

  if ! target_start_was_armed "$transaction"; then
    log "target start was never durably armed; leaving the existing database intact"
    return
  fi

  wait_postgres || die "PostgreSQL did not become ready during interrupted-update recovery"
  database_snapshot_is_readable "$transaction" \
    || die "target start was armed but the required pre-update database snapshot is unreadable"

  log "restoring pre-update PostgreSQL snapshot"
  compose_current exec -T postgres psql -v ON_ERROR_STOP=1 -U argus -d postgres \
    -c "SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname='argus' AND pid <> pg_backend_pid();" >/dev/null
  compose_current exec -T postgres dropdb -U argus --if-exists argus
  compose_current exec -T postgres createdb -U argus argus
  compose_current exec -T postgres pg_restore \
    -U argus \
    -d argus \
    --no-owner \
    --no-privileges <"$transaction/argus.dump"
}

wait_control_api() {
  for _ in $(seq 1 90); do
    if curl -fsS http://127.0.0.1:8080/health >/dev/null 2>&1; then
      return 0
    fi
    sleep 2
  done
  return 1
}

verify_rollback_revision() {
  local service cid revision
  for service in web control-api worker content; do
    cid="$(compose_current ps -q "$service")"
    [[ -n "$cid" ]] || die "rollback service '$service' has no container"
    revision="$(docker inspect -f '{{ index .Config.Labels "org.opencontainers.image.revision" }}' "$cid")"
    [[ "$revision" == "$FROM_REVISION" ]] \
      || die "rollback service '$service' is on revision '$revision', expected $FROM_REVISION"
  done
}

finalize_native_runtime_if_manual() {
  if [[ "$PRESTART_MODE" == "1" ]]; then
    log "rollback core restored; systemd will continue Helper/Agent startup"
    return
  fi

  systemctl enable --now argus-helper.service
  systemctl enable --now argus-agent.service
  /usr/local/bin/argusctl smoke
}

recover_transaction() {
  local transaction="$1"
  log "examining interrupted transaction $transaction"
  load_transaction_metadata "$transaction"

  if ! prepare_transaction_recovery "$transaction"; then
    return 0
  fi

  capture_pre_recovery_version
  log "recovering interrupted transaction $transaction"

  stop_current_runtime_best_effort
  restore_installed_files "$transaction"
  restore_database_if_needed "$transaction"

  log "starting restored revision $FROM_REVISION"
  compose_current up -d
  wait_control_api || {
    compose_current ps || true
    compose_current logs --tail=160 control-api postgres || true
    die "restored Control API did not become healthy"
  }
  compose_current exec -T caddy caddy validate --config /etc/caddy/Caddyfile >/dev/null
  verify_rollback_revision
  finalize_native_runtime_if_manual

  write_transaction_result "$transaction" ROLLED_BACK
  log "interrupted update rolled back to $FROM_REVISION"
  if [[ "$PRESTART_MODE" == "1" ]]; then
    log "run 'sudo argusctl smoke' after boot for full native-service verification"
  fi
}

main() {
  require_root
  command -v docker >/dev/null || die "docker is required"
  command -v flock >/dev/null || die "flock is required"
  command -v sha256sum >/dev/null || die "sha256sum is required"
  command -v sync >/dev/null || die "sync is required"
  case "$RETRY_FAILED" in
    0|1) ;;
    *) die "ARGUS_UPDATE_RECOVERY_RETRY_FAILED must be 0 or 1" ;;
  esac
  acquire_recovery_lock

  local transaction status
  if transaction="$(find_recovery_transaction)"; then
    recover_transaction "$transaction"
    return
  else
    status=$?
  fi

  case "$status" in
    1) return 0 ;;
    2) return 2 ;;
    *) return "$status" ;;
  esac
}

if [[ "${1:-}" != "--internal-test-library" ]]; then
  main "$@"
fi
