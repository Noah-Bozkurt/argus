#!/usr/bin/env bash
set -Eeuo pipefail
umask 077

INSTALL_DIR="${ARGUS_INSTALL_DIR:-/opt/argus}"
STATE_DIR="${ARGUS_STATE_DIR:-/var/lib/argus}"
ENV_FILE="$INSTALL_DIR/.env"
COMPOSE_FILE="$INSTALL_DIR/compose.yaml"
CADDY_FILE="$INSTALL_DIR/Caddyfile"
REGISTRY_CREDENTIAL_FILE="${ARGUS_CONFIG_DIR:-/etc/argus}/registry.env"
BACKUP_ROOT="$STATE_DIR/update-backups"
LOCK_FILE="$STATE_DIR/update.lock"
COMPLETED_TRANSACTION_RETENTION=3
SNAPSHOT_FIXED_HEADROOM_BYTES=1073741824
TRANSACTION_FORMAT_VERSION=2
UPDATE_RUNNER_PROTOCOL_VERSION=1

DOCKER_CONFIG_DIR=""
TARGET_BUNDLE_CONTAINER=""
TARGET_TMP=""
TRANSACTION_DIR=""
ROLLBACK_READY=0
DATABASE_BACKUP_READY=0
TARGET_START_ARMED=0
ROLLBACK_IN_PROGRESS=0
CURRENT_REVISION=""
TARGET_REVISION=""
TARGET_RUNNER_PROTOCOL=""
REQUESTED_VERSION="${ARGUS_TARGET_VERSION:-main}"
PROGRESS_PID=""
PROGRESS_MESSAGE=""
PROGRESS_ENABLED=0
DELEGATED_REVISION="${ARGUS_UPDATE_DELEGATED_REVISION:-}"
DELEGATED_RUNNER="${ARGUS_UPDATE_DELEGATED_RUNNER:-}"
DELEGATED_RUNNER_SHA256="${ARGUS_UPDATE_DELEGATED_RUNNER_SHA256:-}"

if [[ ! -t 1 && -w /dev/tty && "${TERM:-}" != "dumb" ]]; then
  PROGRESS_ENABLED=1
fi

progress_stop() {
  if [[ -z "$PROGRESS_PID" ]]; then
    return
  fi
  kill "$PROGRESS_PID" >/dev/null 2>&1 || true
  wait "$PROGRESS_PID" 2>/dev/null || true
  PROGRESS_PID=""
  PROGRESS_MESSAGE=""
  printf '\r\033[2K' >/dev/tty 2>/dev/null || true
}

progress_start() {
  local message="$1"
  [[ "$PROGRESS_ENABLED" == "1" ]] || return
  progress_stop
  PROGRESS_MESSAGE="$message"
  (
    local frames=('⠋' '⠙' '⠹' '⠸' '⠼' '⠴' '⠦' '⠧' '⠇' '⠏') i=0 frame
    while :; do
      frame="${frames[$((i % ${#frames[@]}))]}"
      if [[ -z "${NO_COLOR:-}" ]]; then
        printf '\r\033[2K\033[36m  %s\033[0m %s' "$frame" "$message" >/dev/tty
      else
        printf '\r\033[2K  %s %s' "$frame" "$message" >/dev/tty
      fi
      i=$((i + 1))
      sleep 0.1
    done
  ) &
  PROGRESS_PID=$!
}

log() {
  case "$*" in
    "resolved target revision:"*|"installing target deployment assets and native binaries"|"starting target control plane "*|"update succeeded:"*) progress_stop ;;
  esac
  printf '[argus-update] %s\n' "$*"
  case "$*" in
    "pre-fetching "*) [[ -n "$PROGRESS_PID" ]] || progress_start "Downloading update" ;;
    "creating consistent PostgreSQL backup") progress_start "Creating rollback backup" ;;
    "installing target deployment assets and native binaries") progress_start "Installing update" ;;
    "starting target control plane "*) progress_start "Starting Argus services" ;;
  esac
}
warn() { progress_stop; printf '[argus-update] warning: %s\n' "$*" >&2; }
die() { progress_stop; printf '[argus-update] error: %s\n' "$*" >&2; exit 1; }

cleanup() {
  progress_stop
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

validate_acceptance_failure_hook() {
  local phase="${ARGUS_UPDATE_ACCEPTANCE_FAILURE:-}" confirmation="${ARGUS_UPDATE_ACCEPTANCE_CONFIRM_FAILURE:-}"
  if [[ -z "$phase" && -z "$confirmation" ]]; then
    return 0
  fi
  [[ "$phase" == "after-target-start-armed" && "$confirmation" == "ROLLBACK-TEST-ONLY" ]] \
    || die "invalid acceptance failure hook; both exact rollback-test values are required"
}

compose() {
  docker compose --project-directory "$INSTALL_DIR" --env-file "$ENV_FILE" -f "$COMPOSE_FILE" "$@"
}

acquire_update_lock() {
  if [[ -n "$DELEGATED_REVISION" ]]; then
    [[ -e /proc/$BASHPID/fd/9 && "$LOCK_FILE" -ef /proc/$BASHPID/fd/9 ]] \
      || die "target update runner did not inherit the active update lock"
    return
  fi
  exec 9>"$LOCK_FILE"
  flock -n 9 || die "another Argus update is already running"
}

validate_delegated_runner() {
  [[ -n "$DELEGATED_REVISION" ]] || return 0
  validate_revision "$DELEGATED_REVISION"
  [[ -n "$DELEGATED_RUNNER" && -n "$DELEGATED_RUNNER_SHA256" ]] \
    || die "target update runner handoff is incomplete"
  [[ "$DELEGATED_RUNNER_SHA256" =~ ^[0-9a-f]{64}$ ]] \
    || die "target update runner handoff has an invalid checksum"
  [[ -f "$DELEGATED_RUNNER" && -x "$DELEGATED_RUNNER" ]] \
    || die "target update runner is missing or not executable"
  [[ "$(stat -c %u "$DELEGATED_RUNNER")" == "0" ]] \
    || die "target update runner is not owned by root"
  [[ "$(stat -c %a "$DELEGATED_RUNNER")" == "700" ]] \
    || die "target update runner permissions are not 0700"
  [[ "$(sha256sum "$DELEGATED_RUNNER" | awk '{ print $1 }')" == "$DELEGATED_RUNNER_SHA256" ]] \
    || die "target update runner failed checksum verification"
  [[ "$DELEGATED_RUNNER" -ef "/proc/$PPID/exe" ]] \
    || die "update handoff is not executing the verified target runner"
}

delegate_to_target_runner() {
  local runner="$TARGET_TMP/argusctl" runner_sha256
  chmod 0700 "$runner"
  runner_sha256="$(sha256sum "$runner" | awk '{ print $1 }')"
  [[ "$runner_sha256" =~ ^[0-9a-f]{64}$ ]] \
    || die "could not checksum the target update runner"

  log "delegating update transaction to target runner $TARGET_REVISION"
  ARGUS_UPDATE_DELEGATED_REVISION="$TARGET_REVISION" \
  ARGUS_UPDATE_DELEGATED_RUNNER="$runner" \
  ARGUS_UPDATE_DELEGATED_RUNNER_SHA256="$runner_sha256" \
    "$runner" update --version "$TARGET_REVISION" --yes --verbose
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
  durable_write_text "$TRANSACTION_DIR/result" "$1"
}

managed_transaction_name() {
  [[ "$1" =~ ^[0-9]{8}T[0-9]{6}Z-[0-9a-f]{12}-to-[0-9a-f]{12}$ ]]
}

prune_completed_transactions() {
  mkdir -p "$BACKUP_ROOT"
  chmod 0700 "$BACKUP_ROOT"

  local retained=0 name dir result
  while IFS= read -r name; do
    managed_transaction_name "$name" || continue
    dir="$BACKUP_ROOT/$name"
    [[ -d "$dir" && -f "$dir/metadata.env" && -d "$dir/files" ]] || continue
    result="$(cat "$dir/result" 2>/dev/null || true)"
    case "$result" in
      SUCCEEDED|ROLLED_BACK) ;;
      *) continue ;;
    esac

    retained=$((retained + 1))
    if (( retained > COMPLETED_TRANSACTION_RETENTION )); then
      log "pruning old completed update snapshot $name"
      rm -rf -- "$dir"
    fi
  done < <(find "$BACKUP_ROOT" -mindepth 1 -maxdepth 1 -type d -printf '%f\n' | LC_ALL=C sort -r)
}

required_snapshot_space_bytes() {
  local database_bytes="$1"
  [[ "$database_bytes" =~ ^[0-9]+$ ]] || return 1
  printf '%s\n' "$((database_bytes * 2 + SNAPSHOT_FIXED_HEADROOM_BYTES))"
}

preflight_snapshot_space() {
  local database_bytes available_bytes required_bytes
  database_bytes="$(compose exec -T postgres psql \
    -v ON_ERROR_STOP=1 \
    -U argus \
    -d argus \
    -Atqc 'SELECT pg_database_size(current_database());' | tr -d '[:space:]')"
  [[ "$database_bytes" =~ ^[0-9]+$ ]] \
    || die "could not determine PostgreSQL database size before update"

  available_bytes="$(df -PB1 "$BACKUP_ROOT" | awk 'NR == 2 { print $4 }')"
  [[ "$available_bytes" =~ ^[0-9]+$ ]] \
    || die "could not determine free space for update snapshots"

  required_bytes="$(required_snapshot_space_bytes "$database_bytes")" \
    || die "could not calculate required update snapshot space"

  log "storage preflight: database=${database_bytes}B available=${available_bytes}B required=${required_bytes}B"
  if (( available_bytes < required_bytes )); then
    die "insufficient free space for a safe update snapshot: ${available_bytes} bytes available, ${required_bytes} required"
  fi
}

registry_login() {
  if [[ -f "$REGISTRY_CREDENTIAL_FILE" ]]; then
    [[ "$(stat -c %a "$REGISTRY_CREDENTIAL_FILE")" == "600" ]] \
      || die "$REGISTRY_CREDENTIAL_FILE must have mode 0600"
    set -a
    # shellcheck disable=SC1090
    . "$REGISTRY_CREDENTIAL_FILE"
    set +a
  fi
  [[ -n "${ARGUS_REGISTRY_USERNAME:-}" ]] \
    || die "registry credentials are missing; run 'sudo argusctl registry-login'"
  [[ -n "${ARGUS_REGISTRY_TOKEN:-}" ]] \
    || die "registry credentials are missing; run 'sudo argusctl registry-login'"

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

image_update_runner_protocol() {
  local image="$1"
  docker image inspect "$image" \
    --format '{{ index .Config.Labels "org.argus.update-runner-protocol" }}'
}

pull_image() {
  local ref="$1" output

  if [[ "${ARGUS_UPDATE_VERBOSE:-0}" == "1" ]] || [[ -t 1 ]]; then
    docker pull "$ref" || die "failed to pull $ref"
    return
  fi

  if ! output="$(docker pull "$ref" 2>&1)"; then
    [[ -z "$output" ]] || printf '%s\n' "$output" >&2
    die "failed to pull $ref"
  fi
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
  pull_image "$discovery_image"
  TARGET_REVISION="$(image_revision "$discovery_image")"
  validate_revision "$TARGET_REVISION"

  local image ref revision
  for image in argus-web argus-control-api argus-worker argus-content argus-host-tools; do
    ref="${ARGUS_REGISTRY}/${image}:${TARGET_REVISION}"
    log "pre-fetching $ref"
    pull_image "$ref"
    revision="$(image_revision "$ref")"
    [[ "$revision" == "$TARGET_REVISION" ]] \
      || die "$ref does not identify the expected revision $TARGET_REVISION"
  done

  TARGET_RUNNER_PROTOCOL="$(image_update_runner_protocol "${ARGUS_REGISTRY}/argus-host-tools:${TARGET_REVISION}")"

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

seal_file_snapshot() {
  local tmp="$TRANSACTION_DIR/file-snapshot.sha256.tmp.$$"
  local -a paths=()
  mapfile -t paths < <(file_snapshot_paths)

  (
    cd "$TRANSACTION_DIR"
    sha256sum "${paths[@]}" >"$tmp"
  )
  chmod 0600 "$tmp"
  (
    cd "$TRANSACTION_DIR"
    sha256sum -c "$tmp" >/dev/null
  )
  sync -f "$tmp"
  mv "$tmp" "$TRANSACTION_DIR/file-snapshot.sha256"
  sync -f "$TRANSACTION_DIR"
  verify_file_snapshot "$TRANSACTION_DIR" \
    || die "pre-update file snapshot failed checksum verification"
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

  seal_file_snapshot
}

quiesce_argus() {
  log "quiescing native Agent/Helper and control-plane writers"
  systemctl stop argus-agent.service
  systemctl stop argus-helper.service
  compose stop worker web content control-api
}

verify_database_snapshot_file() {
  local transaction="$1"
  local manifest="$transaction/database-snapshot.sha256"
  [[ -s "$transaction/argus.dump" && -s "$manifest" ]] || return 1
  [[ "$(awk '{ print $2 }' "$manifest")" == "argus.dump" ]] || return 1
  (
    cd "$transaction"
    sha256sum -c database-snapshot.sha256 >/dev/null 2>&1
  )
}

seal_database_snapshot() {
  compose exec -T postgres pg_restore --list <"$TRANSACTION_DIR/argus.dump" >/dev/null

  local tmp="$TRANSACTION_DIR/database-snapshot.sha256.tmp.$$"
  (
    cd "$TRANSACTION_DIR"
    sha256sum argus.dump >"$tmp"
  )
  chmod 0600 "$tmp"
  sync -f "$tmp"
  mv "$tmp" "$TRANSACTION_DIR/database-snapshot.sha256"
  sync -f "$TRANSACTION_DIR"
  verify_database_snapshot_file "$TRANSACTION_DIR" \
    || die "pre-update database snapshot failed checksum verification"
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
  seal_database_snapshot
  DATABASE_BACKUP_READY=1
}

arm_target_start() {
  [[ "$DATABASE_BACKUP_READY" == "1" ]] \
    || die "target start cannot be armed without a verified database snapshot"
  verify_file_snapshot "$TRANSACTION_DIR" \
    || die "target start cannot be armed with an invalid file snapshot"
  verify_database_snapshot_file "$TRANSACTION_DIR" \
    || die "target start cannot be armed with an invalid database snapshot"

  durable_write_text "$TRANSACTION_DIR/target-start-armed" "$TARGET_REVISION"
  TARGET_START_ARMED=1
}

render_target_caddyfile() {
  local hash rendered_caddyfile tls_mode acme_email
  hash="$(docker run --rm caddy:2-alpine caddy hash-password --plaintext "$ARGUS_BASIC_AUTH_PASSWORD")"
  rendered_caddyfile="$TARGET_TMP/Caddyfile.rendered"
  tls_mode="${ARGUS_TLS_MODE:-public-acme}"
  acme_email="${ARGUS_ACME_EMAIL:-operator@argus.local}"

  case "$tls_mode" in
    public-acme)
      awk -v email="$acme_email" '
        $0 == "__ARGUS_GLOBAL_OPTIONS__" {
          print "{"
          print "\temail " email
          print "\tcert_issuer acme https://acme-v02.api.letsencrypt.org/directory"
          print "\tcert_issuer acme https://acme.zerossl.com/v2/DV90"
          print "}"
          next
        }
        $0 == "__ARGUS_TLS__" { next }
        { print }
      ' "$TARGET_TMP/Caddyfile.template" >"$rendered_caddyfile"
      ;;
    cloudflare-origin)
      awk '
        $0 == "__ARGUS_GLOBAL_OPTIONS__" { next }
        $0 == "__ARGUS_TLS__" {
          print "\ttls /etc/caddy/argus-tls/origin.crt /etc/caddy/argus-tls/origin.key"
          next
        }
        { print }
      ' "$TARGET_TMP/Caddyfile.template" >"$rendered_caddyfile"
      ;;
    *) die "unsupported ARGUS_TLS_MODE in installed environment: $tls_mode" ;;
  esac

  sed -i \
    -e "s|__ARGUS_DOMAIN__|${ARGUS_DOMAIN}|g" \
    -e "s|__ARGUS_CONTENT_DOMAIN__|${ARGUS_CONTENT_DOMAIN}|g" \
    -e "s|__BASIC_AUTH_USER__|${ARGUS_BASIC_AUTH_USER}|g" \
    -e "s|__BASIC_AUTH_HASH__|${hash}|g" \
    "$rendered_caddyfile"

  if grep -Eq '__[A-Z0-9_]+__' "$rendered_caddyfile"; then
    die "rendered Caddyfile still contains an unresolved placeholder"
  fi

  docker run --rm \
    -v "$rendered_caddyfile:/etc/caddy/Caddyfile:ro" \
    caddy:2-alpine caddy validate --config /etc/caddy/Caddyfile >/dev/null

  # Copy the already-rendered content over the existing file. Unlike sed -i,
  # cp preserves the destination inode, so an existing Caddy bind mount never
  # observes the unrendered template or remains pinned to a stale inode.
  cp "$rendered_caddyfile" "$CADDY_FILE"
  chmod 0640 "$CADDY_FILE"
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
  [[ "$TARGET_START_ARMED" == "1" ]] \
    || die "target control plane cannot start before the transaction is durably armed"

  log "starting target control plane $TARGET_REVISION"
  compose config >/dev/null
  compose up -d --remove-orphans

  if ! wait_control_plane_health; then
    compose ps || true
    compose logs --tail=160 control-api worker web content postgres || true
    return 1
  fi

  # Recreate Caddy to pick up any deployment-level changes as well as the
  # newly validated configuration.
  compose up -d --no-deps --force-recreate caddy
  compose exec -T caddy caddy validate --config /etc/caddy/Caddyfile >/dev/null

  systemctl enable --now argus-helper.service
  systemctl enable --now argus-agent.service

  progress_stop
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

  verify_file_snapshot "$TRANSACTION_DIR"
  local snapshot_status=$?
  local files_status=1
  if [[ "$snapshot_status" -eq 0 ]]; then
    restore_installed_files
    files_status=$?
  else
    warn "pre-update file snapshot checksum verification failed"
  fi

  local db_status=0
  if [[ "$TARGET_START_ARMED" == "1" ]]; then
    verify_database_snapshot_file "$TRANSACTION_DIR"
    db_status=$?
    if [[ "$db_status" -eq 0 ]]; then
      restore_database
      db_status=$?
    else
      warn "target start was armed but the database snapshot checksum is invalid"
    fi
  else
    warn "target start was never armed; skipping database restore"
  fi

  compose up -d --remove-orphans
  local compose_status=$?
  local health_status=1
  local caddy_status=1
  if [[ "$compose_status" -eq 0 ]]; then
    wait_control_plane_health
    health_status=$?
    if [[ "$health_status" -eq 0 ]]; then
      # restore_installed_files may also replace the bind-mounted file inode.
      compose up -d --no-deps --force-recreate caddy
      caddy_status=$?
      if [[ "$caddy_status" -eq 0 ]]; then
        compose exec -T caddy caddy validate --config /etc/caddy/Caddyfile >/dev/null
        caddy_status=$?
      fi
    fi
  fi

  systemctl enable --now argus-helper.service
  local helper_status=$?
  systemctl enable --now argus-agent.service
  local agent_status=$?

  local smoke_status=1
  if [[ "$files_status" -eq 0 && "$db_status" -eq 0 && "$compose_status" -eq 0 && "$health_status" -eq 0 && "$caddy_status" -eq 0 && "$helper_status" -eq 0 && "$agent_status" -eq 0 ]]; then
    /usr/local/bin/argusctl smoke
    smoke_status=$?
  fi

  if [[ "$files_status" -eq 0 && "$db_status" -eq 0 && "$compose_status" -eq 0 && "$health_status" -eq 0 && "$caddy_status" -eq 0 && "$helper_status" -eq 0 && "$agent_status" -eq 0 && "$smoke_status" -eq 0 ]]; then
    if write_transaction_result ROLLED_BACK; then
      warn "rollback completed successfully; restored revision $CURRENT_REVISION"
      return 0
    fi
    warn "rollback restored the previous revision but could not persist its terminal result"
  fi

  write_transaction_result ROLLBACK_FAILED || true
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
  validate_acceptance_failure_hook
  validate_delegated_runner
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
  command -v sha256sum >/dev/null || die "sha256sum is required"
  command -v sync >/dev/null || die "sync is required"
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

  log "verifying current installation before update"
  /usr/local/bin/argusctl smoke

  prune_completed_transactions
  pull_and_verify_target
  if [[ "$TARGET_REVISION" == "$CURRENT_REVISION" ]]; then
    normalize_installed_version
    log "already running requested revision $CURRENT_REVISION"
    return
  fi

  [[ "$TARGET_RUNNER_PROTOCOL" == "$UPDATE_RUNNER_PROTOCOL_VERSION" ]] \
    || die "target host tools do not support update runner protocol $UPDATE_RUNNER_PROTOCOL_VERSION"

  prepare_target_bundle
  if [[ -n "$DELEGATED_REVISION" ]]; then
    [[ "$REQUESTED_VERSION" == "$DELEGATED_REVISION" ]] \
      || die "target update runner was invoked for an unexpected version"
    [[ "$TARGET_REVISION" == "$DELEGATED_REVISION" ]] \
      || die "target update runner revision does not match the verified image set"
    log "target update runner accepted revision $TARGET_REVISION"
    normalize_installed_version
  else
    delegate_to_target_runner
    return
  fi
  preflight_snapshot_space

  TRANSACTION_DIR="$BACKUP_ROOT/$(date -u +%Y%m%dT%H%M%SZ)-${CURRENT_REVISION:0:12}-to-${TARGET_REVISION:0:12}"
  mkdir -p "$TRANSACTION_DIR"
  chmod 0700 "$TRANSACTION_DIR"
  cat >"$TRANSACTION_DIR/metadata.env" <<EOF
TRANSACTION_FORMAT=${TRANSACTION_FORMAT_VERSION}
FROM_REVISION=${CURRENT_REVISION}
TO_REVISION=${TARGET_REVISION}
REQUESTED_VERSION=${REQUESTED_VERSION}
STARTED_AT=$(date -u +%Y-%m-%dT%H:%M:%SZ)
EOF
  chmod 0600 "$TRANSACTION_DIR/metadata.env"
  sync -f "$TRANSACTION_DIR/metadata.env"
  sync -f "$TRANSACTION_DIR"

  backup_installed_files
  ROLLBACK_READY=1
  quiesce_argus
  create_database_backup

  install_target_files
  arm_target_start
  if [[ "${ARGUS_UPDATE_ACCEPTANCE_FAILURE:-}" == "after-target-start-armed" ]]; then
    warn "injecting confirmed acceptance failure after target start was durably armed"
    false
  fi
  start_target

  ROLLBACK_READY=0
  write_transaction_result SUCCEEDED
  log "update succeeded: $CURRENT_REVISION -> $TARGET_REVISION"
  log "rollback snapshot retained at $TRANSACTION_DIR"
  prune_completed_transactions
}

if [[ "${1:-}" != "--internal-test-library" ]]; then
  main "$@"
fi
