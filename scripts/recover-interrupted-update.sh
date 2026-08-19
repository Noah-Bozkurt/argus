#!/usr/bin/env bash
set -Eeuo pipefail
umask 077

INSTALL_DIR="${ARGUS_INSTALL_DIR:-/opt/argus}"
STATE_DIR="${ARGUS_STATE_DIR:-/var/lib/argus}"
BACKUP_ROOT="$STATE_DIR/update-backups"
LOCK_FILE="/run/lock/argus-update.lock"
ENV_FILE="$INSTALL_DIR/.env"
COMPOSE_FILE="$INSTALL_DIR/compose.yaml"
CADDY_FILE="$INSTALL_DIR/Caddyfile"

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
  install -d -m 0755 /run/lock
  exec 9>"$LOCK_FILE"
  if ! flock -n 9; then
    # A live updater owns the same lock. This is expected when the Helper is
    # restarted near the end of a normal update transaction.
    log "active update owns the lifecycle lock; skipping boot recovery"
    exit 0
  fi
}

find_incomplete_transaction() {
  [[ -d "$BACKUP_ROOT" ]] || return 1

  local entry dir result
  while IFS= read -r entry; do
    dir="${entry#* }"
    [[ -f "$dir/metadata.env" && -d "$dir/files" ]] || continue
    result="$(cat "$dir/result" 2>/dev/null || true)"
    case "$result" in
      SUCCEEDED|ROLLED_BACK) continue ;;
    esac
    printf '%s\n' "$dir"
    return 0
  done < <(find "$BACKUP_ROOT" -mindepth 1 -maxdepth 1 -type d -printf '%T@ %p\n' | sort -nr)

  return 1
}

load_transaction_metadata() {
  local transaction="$1"
  # shellcheck disable=SC1090
  . "$transaction/metadata.env"
  : "${FROM_REVISION:?transaction is missing FROM_REVISION}"
  : "${TO_REVISION:?transaction is missing TO_REVISION}"
  valid_revision "$FROM_REVISION" || die "invalid FROM_REVISION in $transaction"
  valid_revision "$TO_REVISION" || die "invalid TO_REVISION in $transaction"
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

  if [[ -f "$ENV_FILE" && -f "$COMPOSE_FILE" ]]; then
    load_installed_env_if_available
    compose_current stop caddy worker web content control-api >/dev/null 2>&1 || true
  fi
}

restore_installed_files() {
  local transaction="$1"
  local backup="$transaction/files"

  for required in \
    "$backup/install/.env" \
    "$backup/install/compose.yaml" \
    "$backup/install/Caddyfile" \
    "$backup/install/Caddyfile.template" \
    "$backup/bin/argus-agent" \
    "$backup/bin/argus-helper" \
    "$backup/bin/argusctl" \
    "$backup/systemd/argus-agent.service" \
    "$backup/systemd/argus-helper.service"
  do
    [[ -f "$required" ]] || die "rollback snapshot is incomplete: $required"
  done

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

database_snapshot_is_complete() {
  local transaction="$1"
  local dump="$transaction/argus.dump"
  [[ -s "$dump" ]] || return 1
  compose_current exec -T postgres pg_restore --list <"$dump" >/dev/null 2>&1
}

restore_database_if_needed() {
  local transaction="$1"

  wait_postgres || die "PostgreSQL did not become ready during interrupted-update recovery"

  if ! database_snapshot_is_complete "$transaction"; then
    # update-first-test.sh only installs/starts the target after pg_dump exits
    # successfully. An absent or incomplete archive therefore means the target
    # could not yet have migrated the database, so restoring it would add risk.
    log "no complete pre-update database snapshot; leaving the existing database intact"
    return
  fi

  log "restoring complete pre-update PostgreSQL snapshot"
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

recover_transaction() {
  local transaction="$1"
  log "recovering interrupted transaction $transaction"
  load_transaction_metadata "$transaction"

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
  verify_rollback_revision

  printf 'ROLLED_BACK\n' >"$transaction/result"
  chmod 0600 "$transaction/result"
  log "interrupted update rolled back to $FROM_REVISION"
  log "Helper/Agent startup may now continue; run 'sudo argusctl smoke' after boot for full verification"
}

main() {
  require_root
  command -v docker >/dev/null || die "docker is required"
  command -v flock >/dev/null || die "flock is required"
  acquire_recovery_lock

  local transaction
  if ! transaction="$(find_incomplete_transaction)"; then
    exit 0
  fi

  recover_transaction "$transaction"
}

main "$@"
