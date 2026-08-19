#!/usr/bin/env bash
set -Eeuo pipefail
umask 077

INSTALL_DIR="${ARGUS_INSTALL_DIR:-/opt/argus}"
STATE_DIR="${ARGUS_STATE_DIR:-/var/lib/argus}"
ENV_FILE="$INSTALL_DIR/.env"
COMPOSE_FILE="$INSTALL_DIR/compose.yaml"
CADDY_FILE="$INSTALL_DIR/Caddyfile"
BACKUP_ROOT="$STATE_DIR/update-backups"
LOCK_FILE="$STATE_DIR/update.lock"

DOCKER_CONFIG_DIR=""
TARGET_BUNDLE_CONTAINER=""
TARGET_TMP=""
TRANSACTION_DIR=""
ROLLBACK_READY=0
DATABASE_BACKUP_READY=0
ROLLBACK_IN_PROGRESS=0
CURRENT_REVISION=""
TARGET_REVISION=""
REQUESTED_VERSION="${ARGUS_TARGET_VERSION:-main}"

log() { printf '[argus-update] %s\n' "$*"; }
warn() { printf '[argus-update] warning: %s\n' "$*" >&2; }
die() { printf '[argus-update] error: %s\n' "$*" >&2; exit 1; }

cleanup() {
  if [[ -n "$TARGET_BUNDLE_CONTAINER" ]]; then
    docker rm -f "$TARGET_BUNDLE_CONTAINER" >/dev/null 2>&1 || true
  fi
  if [[ -n "$TARGET_TMP" ]]; then
    rm -rf "$TARGET_TMP"
  fi
  if [[ -n "$DOCKER_CONFIG_DIR" ]]; then
    rm -rf "$DOCKER_CONFIG_DIR"
  fi
}
trap cleanup EXIT

require_root() {
  [[ "${EUID}" -eq 0 ]] || die "run as root (sudo argusctl update)"
}

require_file() {
  [[ -f "$1" ]] || die "required installed file is missing: $1"
}

validate_version_tag() {
  [[ "$1" =~ ^[A-Za-z0-9_][A-Za-z0-9_.-]{0,127}$ ]] \
    || die "invalid image tag/version: $1"
}

validate_revision() {
  [[ "$1" =~ ^[0-9a-f]{40}$ ]] \
    || die "image is missing a valid immutable org.opencontainers.image.revision label"
}

compose() {
  docker compose --project-directory "$INSTALL_DIR" --env-file "$ENV_FILE" -f "$COMPOSE_FILE" "$@"
}

acquire_update_lock() {
  exec 9>"$LOCK_FILE"
  flock -n 9 || die "another Argus update is already running"
}

registry_login() {
  [[ -n "${ARGUS_REGISTRY_USERNAME:-}" ]] \
    || die "ARGUS_REGISTRY_USERNAME is required for updates"
  [[ -n "${ARGUS_REGISTRY_TOKEN:-}" ]] \
    || die "ARGUS_REGISTRY_TOKEN is required for updates; use a read-only package token"

  DOCKER_CONFIG_DIR="$(mktemp -d)"
  chmod 0700 "$DOCKER_CONFIG_DIR"
  export DOCKER_CONFIG="$DOCKER_CONFIG_DIR"
  local registry_host="${ARGUS_REGISTRY%%/*}"
  printf '%s' "$ARGUS_REGISTRY_TOKEN" \
    | docker login "$registry_host" -u "$ARGUS_REGISTRY_USERNAME" --password-stdin >/dev/null
}

image_revision() {
  local image="$1"
  docker image inspect "$image" \
    --format '{{ index .Config.Labels "org.opencontainers.image.revision" }}'
}

running_service_image_id() {
  local service="$1" cid
  cid="$(compose ps -q "$service")"
  [[ -n "$cid" ]] || return 1
  docker inspect -f '{{.Image}}' "$cid"
}

resolve_current_revision() {
  local control_image
  control_image="$(running_service_image_id control-api)" \
    || die "Control API container is not running; restore the installation before updating"
  CURRENT_REVISION="$(image_revision "$control_image")"
  validate_revision "$CURRENT_REVISION"

  local service image image_id revision
  while read -r service image; do
    image_id="$(running_service_image_id "$service")" \
      || die "Compose service '$service' is not running"
    revision="$(image_revision "$image_id")"
    [[ "$revision" == "$CURRENT_REVISION" ]] \
      || die "installed Argus services are on mixed revisions; refusing transactional update"
    docker tag "$image_id" "${ARGUS_REGISTRY}/${image}:${CURRENT_REVISION}"
  done <<'EOF'
web argus-web
control-api argus-control-api
worker argus-worker
content argus-content
EOF

  log "current installed revision: $CURRENT_REVISION"
}

pull_and_verify_target() {
  validate_version_tag "$REQUESTED_VERSION"
  local discovery_image="${ARGUS_REGISTRY}/argus-host-tools:${REQUESTED_VERSION}"
  log "resolving target '$REQUESTED_VERSION' through $discovery_image"
  docker pull "$discovery_image" >/dev/null
  TARGET_REVISION="$(image_revision "$discovery_image")"
  validate_revision "$TARGET_REVISION"

  local image ref revision
  for image in argus-web argus-control-api argus-worker argus-content argus-host-tools; do
    ref="${ARGUS_REGISTRY}/${image}:${TARGET_REVISION}"
    log "pre-fetching $ref"
    docker pull "$ref" >/dev/null
    revision="$(image_revision "$ref")"
    [[ "$revision" == "$TARGET_REVISION" ]] \
      || die "$ref does not identify the expected revision $TARGET_REVISION"
  done

  log "resolved target revision: $TARGET_REVISION"
}

prepare_target_bundle() {
  TARGET_TMP="$(mktemp -d)"
  chmod 0700 "$TARGET_TMP"
  local image="${ARGUS_REGISTRY}/argus-host-tools:${TARGET_REVISION}"
  TARGET_BUNDLE_CONTAINER="$(docker create "$image")"

  docker cp "$TARGET_BUNDLE_CONTAINER:/out/." "$TARGET_TMP/"
  docker cp "$TARGET_BUNDLE_CONTAINER:/deploy/compose.yaml" "$TARGET_TMP/compose.yaml"
  docker cp "$TARGET_BUNDLE_CONTAINER:/deploy/Caddyfile.template" "$TARGET_TMP/Caddyfile.template"
  docker cp "$TARGET_BUNDLE_CONTAINER:/deploy/systemd/argus-agent.service" "$TARGET_TMP/argus-agent.service"
  docker cp "$TARGET_BUNDLE_CONTAINER:/deploy/systemd/argus-helper.service" "$TARGET_TMP/argus-helper.service"

  for path in \
    "$TARGET_TMP/argus-agent" \
    "$TARGET_TMP/argus-helper" \
    "$TARGET_TMP/argusctl" \
    "$TARGET_TMP/compose.yaml" \
    "$TARGET_TMP/Caddyfile.template" \
    "$TARGET_TMP/argus-agent.service" \
    "$TARGET_TMP/argus-helper.service"
  do
    [[ -s "$path" ]] || die "target host-tools bundle is incomplete: $path"
  done

  docker rm "$TARGET_BUNDLE_CONTAINER" >/dev/null
  TARGET_BUNDLE_CONTAINER=""
}

set_env_version() {
  local path="$1" revision="$2" tmp
  tmp="$(mktemp "${path}.XXXXXX")"
  awk -v revision="$revision" '
    BEGIN { replaced = 0 }
    /^ARGUS_VERSION=/ {
      print "ARGUS_VERSION=" revision
      replaced = 1
      next
    }
    { print }
    END {
      if (!replaced) print "ARGUS_VERSION=" revision
    }
  ' "$path" >"$tmp"
  chmod 0600 "$tmp"
  mv "$tmp" "$path"
}

normalize_installed_version() {
  if [[ "${ARGUS_VERSION:-}" != "$CURRENT_REVISION" ]]; then
    log "pinning legacy/mutable installed version '${ARGUS_VERSION:-unknown}' to $CURRENT_REVISION"
    set_env_version "$ENV_FILE" "$CURRENT_REVISION"
    ARGUS_VERSION="$CURRENT_REVISION"
    export ARGUS_VERSION
  fi
}

backup_installed_files() {
  local backup="$TRANSACTION_DIR/files"
  mkdir -p "$backup/bin" "$backup/systemd" "$backup/install"
  chmod 0700 "$backup" "$backup/bin" "$backup/systemd" "$backup/install"

  cp -a "$ENV_FILE" "$backup/install/.env"
  cp -a "$COMPOSE_FILE" "$backup/install/compose.yaml"
  cp -a "$CADDY_FILE" "$backup/install/Caddyfile"
  cp -a "$INSTALL_DIR/Caddyfile.template" "$backup/install/Caddyfile.template"
  cp -a /usr/local/bin/argus-agent "$backup/bin/argus-agent"
  cp -a /usr/local/bin/argus-helper "$backup/bin/argus-helper"
  cp -a /usr/local/bin/argusctl "$backup/bin/argusctl"
  cp -a /etc/systemd/system/argus-agent.service "$backup/systemd/argus-agent.service"
  cp -a /etc/systemd/system/argus-helper.service "$backup/systemd/argus-helper.service"
}

quiesce_argus() {
  log "quiescing native Agent/Helper and control-plane writers"
  systemctl stop argus-agent.service
  systemctl stop argus-helper.service
  compose stop worker web content control-api
}

create_database_backup() {
  log "creating consistent PostgreSQL backup"
  compose exec -T postgres pg_dump \
    -U argus \
    -d argus \
    --format=custom \
    --no-owner \
    --no-privileges >"$TRANSACTION_DIR/argus.dump"
  chmod 0600 "$TRANSACTION_DIR/argus.dump"
  [[ -s "$TRANSACTION_DIR/argus.dump" ]] || return 1
  DATABASE_BACKUP_READY=1
}

render_target_caddyfile() {
  local hash
  hash="$(docker run --rm caddy:2-alpine caddy hash-password --plaintext "$ARGUS_BASIC_AUTH_PASSWORD")"
  cp "$TARGET_TMP/Caddyfile.template" "$CADDY_FILE"
  sed -i \
    -e "s|__ARGUS_DOMAIN__|${ARGUS_DOMAIN}|g" \
    -e "s|__ARGUS_CONTENT_DOMAIN__|${ARGUS_CONTENT_DOMAIN}|g" \
    -e "s|__BASIC_AUTH_USER__|${ARGUS_BASIC_AUTH_USER}|g" \
    -e "s|__BASIC_AUTH_HASH__|${hash}|g" \
    "$CADDY_FILE"
  chmod 0640 "$CADDY_FILE"

  docker run --rm \
    -v "$CADDY_FILE:/etc/caddy/Caddyfile:ro" \
    caddy:2-alpine caddy validate --config /etc/caddy/Caddyfile >/dev/null
}

install_target_files() {
  log "installing target deployment assets and native binaries"
  install -m 0644 "$TARGET_TMP/compose.yaml" "$COMPOSE_FILE"
  install -m 0644 "$TARGET_TMP/Caddyfile.template" "$INSTALL_DIR/Caddyfile.template"
  install -m 0755 "$TARGET_TMP/argus-agent" /usr/local/bin/argus-agent
  install -m 0755 "$TARGET_TMP/argus-helper" /usr/local/bin/argus-helper
  install -m 0755 "$TARGET_TMP/argusctl" /usr/local/bin/argusctl
  install -m 0644 "$TARGET_TMP/argus-agent.service" /etc/systemd/system/argus-agent.service
  install -m 0644 "$TARGET_TMP/argus-helper.service" /etc/systemd/system/argus-helper.service
  render_target_caddyfile
  set_env_version "$ENV_FILE" "$TARGET_REVISION"
  ARGUS_VERSION="$TARGET_REVISION"
  export ARGUS_VERSION
  systemctl daemon-reload
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

wait_service_healthy() {
  local service="$1" cid running health
  for _ in $(seq 1 60); do
    cid="$(compose ps -q "$service")"
    if [[ -n "$cid" ]]; then
      running="$(docker inspect -f '{{.State.Running}}' "$cid" 2>/dev/null || true)"
      health="$(docker inspect -f '{{if .State.Health}}{{.State.Health.Status}}{{else}}missing{{end}}' "$cid" 2>/dev/null || true)"
      if [[ "$running" == "true" && "$health" == "healthy" ]]; then
        return 0
      fi
    fi
    sleep 2
  done
  return 1
}

wait_control_plane_health() {
  wait_control_api || return 1
  local service
  for service in postgres control-api worker web content; do
    wait_service_healthy "$service" || return 1
  done
}

start_target() {
  log "starting target control plane $TARGET_REVISION"
  compose config >/dev/null
  compose up -d --remove-orphans

  if ! wait_control_plane_health; then
    compose ps || true
    compose logs --tail=160 control-api worker web content postgres || true
    return 1
  fi

  compose exec -T caddy caddy validate --config /etc/caddy/Caddyfile >/dev/null
  compose exec -T caddy caddy reload --config /etc/caddy/Caddyfile >/dev/null

  systemctl enable --now argus-helper.service
  systemctl enable --now argus-agent.service

  /usr/local/bin/argusctl smoke
}

restore_database() {
  log "restoring PostgreSQL snapshot"
  compose up -d postgres || return 1
  local ready=0
  for _ in $(seq 1 60); do
    if compose exec -T postgres pg_isready -U argus -d postgres >/dev/null 2>&1; then
      ready=1
      break
    fi
    sleep 1
  done
  [[ "$ready" == "1" ]] || return 1

  compose exec -T postgres psql -v ON_ERROR_STOP=1 -U argus -d postgres \
    -c "SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname='argus' AND pid <> pg_backend_pid();" >/dev/null \
    || return 1
  compose exec -T postgres dropdb -U argus --if-exists argus || return 1
  compose exec -T postgres createdb -U argus argus || return 1
  compose exec -T postgres pg_restore \
    -U argus \
    -d argus \
    --no-owner \
    --no-privileges <"$TRANSACTION_DIR/argus.dump" \
    || return 1
}

restore_installed_files() {
  local backup="$TRANSACTION_DIR/files"
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
}

rollback_transaction() {
  ROLLBACK_IN_PROGRESS=1
  trap - ERR INT TERM
  set +e
  warn "update failed; automatically rolling back transaction $TRANSACTION_DIR"

  systemctl stop argus-agent.service >/dev/null 2>&1 || true
  systemctl stop argus-helper.service >/dev/null 2>&1 || true
  compose stop worker web content control-api >/dev/null 2>&1 || true

  restore_installed_files
  local files_status=$?

  local db_status=0
  if [[ "$DATABASE_BACKUP_READY" == "1" ]]; then
    restore_database
    db_status=$?
  else
    warn "database was not mutated; skipping database restore"
  fi

  compose up -d --remove-orphans
  local compose_status=$?
  local health_status=1
  if [[ "$compose_status" -eq 0 ]]; then
    wait_control_plane_health
    health_status=$?
    compose exec -T caddy caddy reload --config /etc/caddy/Caddyfile >/dev/null 2>&1 || true
  fi

  systemctl enable --now argus-helper.service
  local helper_status=$?
  systemctl enable --now argus-agent.service
  local agent_status=$?

  local smoke_status=1
  if [[ "$files_status" -eq 0 && "$db_status" -eq 0 && "$compose_status" -eq 0 && "$health_status" -eq 0 && "$helper_status" -eq 0 && "$agent_status" -eq 0 ]]; then
    /usr/local/bin/argusctl smoke
    smoke_status=$?
  fi

  if [[ "$files_status" -eq 0 && "$db_status" -eq 0 && "$compose_status" -eq 0 && "$health_status" -eq 0 && "$helper_status" -eq 0 && "$agent_status" -eq 0 && "$smoke_status" -eq 0 ]]; then
    printf 'ROLLED_BACK\n' >"$TRANSACTION_DIR/result"
    warn "rollback completed successfully; restored revision $CURRENT_REVISION"
    return 0
  fi

  printf 'ROLLBACK_FAILED\n' >"$TRANSACTION_DIR/result"
  warn "automatic rollback did not fully recover Argus"
  warn "transaction backup is preserved at $TRANSACTION_DIR"
  return 1
}

on_error() {
  local rc=$?
  if [[ "$ROLLBACK_IN_PROGRESS" == "1" ]]; then
    exit "$rc"
  fi
  if [[ "$ROLLBACK_READY" == "1" ]]; then
    rollback_transaction || true
  fi
  exit "$rc"
}

on_signal() {
  local signal="$1"
  warn "received $signal during update"
  if [[ "$ROLLBACK_READY" == "1" && "$ROLLBACK_IN_PROGRESS" != "1" ]]; then
    rollback_transaction || true
  fi
  exit 130
}

trap on_error ERR
trap 'on_signal SIGINT' INT
trap 'on_signal SIGTERM' TERM

main() {
  require_root
  require_file "$ENV_FILE"
  require_file "$COMPOSE_FILE"
  require_file "$CADDY_FILE"
  require_file "$INSTALL_DIR/Caddyfile.template"
  require_file /usr/local/bin/argus-agent
  require_file /usr/local/bin/argus-helper
  require_file /usr/local/bin/argusctl
  require_file /etc/systemd/system/argus-agent.service
  require_file /etc/systemd/system/argus-helper.service
  command -v docker >/dev/null || die "docker is required"
  command -v flock >/dev/null || die "flock is required"
  acquire_update_lock

  set -a
  # shellcheck disable=SC1090
  . "$ENV_FILE"
  set +a

  : "${ARGUS_REGISTRY:?missing ARGUS_REGISTRY in installed environment}"
  : "${ARGUS_BASIC_AUTH_USER:?missing ARGUS_BASIC_AUTH_USER}"
  : "${ARGUS_BASIC_AUTH_PASSWORD:?missing ARGUS_BASIC_AUTH_PASSWORD}"
  : "${ARGUS_DOMAIN:?missing ARGUS_DOMAIN}"
  : "${ARGUS_CONTENT_DOMAIN:?missing ARGUS_CONTENT_DOMAIN}"

  registry_login
  resolve_current_revision
  normalize_installed_version

  log "verifying current installation before update"
  /usr/local/bin/argusctl smoke

  pull_and_verify_target
  if [[ "$TARGET_REVISION" == "$CURRENT_REVISION" ]]; then
    log "already running requested revision $CURRENT_REVISION"
    return
  fi

  prepare_target_bundle

  mkdir -p "$BACKUP_ROOT"
  chmod 0700 "$BACKUP_ROOT"
  TRANSACTION_DIR="$BACKUP_ROOT/$(date -u +%Y%m%dT%H%M%SZ)-${CURRENT_REVISION:0:12}-to-${TARGET_REVISION:0:12}"
  mkdir -p "$TRANSACTION_DIR"
  chmod 0700 "$TRANSACTION_DIR"
  cat >"$TRANSACTION_DIR/metadata.env" <<EOF
FROM_REVISION=${CURRENT_REVISION}
TO_REVISION=${TARGET_REVISION}
REQUESTED_VERSION=${REQUESTED_VERSION}
STARTED_AT=$(date -u +%Y-%m-%dT%H:%M:%SZ)
EOF
  chmod 0600 "$TRANSACTION_DIR/metadata.env"

  backup_installed_files
  ROLLBACK_READY=1
  quiesce_argus
  create_database_backup

  install_target_files
  start_target

  ROLLBACK_READY=0
  printf 'SUCCEEDED\n' >"$TRANSACTION_DIR/result"
  log "update succeeded: $CURRENT_REVISION -> $TARGET_REVISION"
  log "rollback snapshot retained at $TRANSACTION_DIR"
}

main "$@"
