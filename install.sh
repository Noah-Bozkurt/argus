#!/usr/bin/env bash
set -Eeuo pipefail
umask 077

INSTALL_DIR="${ARGUS_INSTALL_DIR:-/opt/argus}"
CONFIG_DIR="${ARGUS_CONFIG_DIR:-/etc/argus}"
STATE_DIR="${ARGUS_STATE_DIR:-/var/lib/argus}"
ENV_FILE="$INSTALL_DIR/.env"
COMPOSE_FILE="$INSTALL_DIR/compose.yaml"
CADDY_FILE="$INSTALL_DIR/Caddyfile"
HOST_TOOLS_CONTAINER=""
DOCKER_CONFIG_DIR=""
GENERATED_BASIC_AUTH_PASSWORD=""
EXISTING_INSTALL=0
INSTALL_MODE="${ARGUS_INSTALL_MODE:-}"
DISTRIBUTION_URL="${ARGUS_DISTRIBUTION_URL:-https://install.argus.example}"
RELEASE_CHANNEL="${ARGUS_RELEASE_CHANNEL:-stable}"
DEVICE_SESSION=""
RELEASE_TMP=""
LOG_DIR="${ARGUS_LOG_DIR:-/var/log/argus}"
LOG_FILE="$LOG_DIR/install-$(date -u +%Y%m%dT%H%M%SZ).log"
STAGE=0
TOTAL_STAGES=7

log() { printf '[argus] %s\n' "$*"; }
warn() { printf '[argus] warning: %s\n' "$*" >&2; }
die() { printf '[argus] error: %s\n' "$*" >&2; exit 1; }
stage() { STAGE=$((STAGE + 1)); printf '\n[%d/%d] %s\n' "$STAGE" "$TOTAL_STAGES" "$1"; }

banner() {
  printf '\n========================================\n'
  printf '           ARGUS INSTALLER\n'
  printf '========================================\n\n'
}

select_mode() {
  banner
  if [[ -z "$INSTALL_MODE" ]]; then
    [[ -t 0 ]] || die "ARGUS_INSTALL_MODE must be control-plane or agent in non-interactive mode"
    printf '  1. Install an Argus control plane here.\n'
    printf '  2. Connect this server to an existing Argus instance.\n\n'
    read -r -p 'Choose [1-2]: ' choice
    case "$choice" in 1) INSTALL_MODE=control-plane ;; 2) INSTALL_MODE=agent ;; *) die "invalid installation mode" ;; esac
  fi
  [[ "$INSTALL_MODE" == "control-plane" || "$INSTALL_MODE" == "agent" ]] || die "ARGUS_INSTALL_MODE must be control-plane or agent"
}

cleanup() {
  if [[ -n "$HOST_TOOLS_CONTAINER" ]]; then
    docker rm -f "$HOST_TOOLS_CONTAINER" >/dev/null 2>&1 || true
  fi
  if [[ -n "$DOCKER_CONFIG_DIR" ]]; then
    rm -rf "$DOCKER_CONFIG_DIR"
  fi
  if [[ -n "$RELEASE_TMP" ]]; then
    rm -rf "$RELEASE_TMP"
  fi
}
trap cleanup EXIT

require_root() {
  [[ "${EUID}" -eq 0 ]] || die "run the installer as root (sudo ./install.sh)"
}

new_secret() { openssl rand -hex 32; }
new_uuid() { cat /proc/sys/kernel/random/uuid; }
new_password() { openssl rand -base64 24 | tr -d '\n'; }

validate_domain() {
  local value="$1"
  [[ "$value" =~ ^[A-Za-z0-9]([A-Za-z0-9.-]*[A-Za-z0-9])?$ ]] || die "invalid domain: $value"
  [[ "$value" == *.* ]] || die "domain must be a fully-qualified DNS name: $value"
}

validate_basic_user() {
  [[ "$1" =~ ^[A-Za-z0-9._-]+$ ]] || die "ARGUS_BASIC_AUTH_USER may only contain letters, digits, dot, underscore and hyphen"
}

is_revision() {
  [[ "$1" =~ ^[0-9a-f]{40}$ ]]
}

prompt_required() {
  local variable="$1"
  local prompt="$2"
  local current="${!variable:-}"
  if [[ -n "$current" ]]; then
    return
  fi
  if [[ -t 0 ]]; then
    read -r -p "$prompt: " current
    [[ -n "$current" ]] || die "$variable is required"
    printf -v "$variable" '%s' "$current"
    export "$variable"
  else
    die "$variable is required in non-interactive mode"
  fi
}

prompt_password() {
  if [[ -n "${ARGUS_BASIC_AUTH_PASSWORD:-}" ]]; then return; fi
  if [[ ! -t 0 ]]; then
    ARGUS_BASIC_AUTH_PASSWORD="$(new_password)"
    GENERATED_BASIC_AUTH_PASSWORD="$ARGUS_BASIC_AUTH_PASSWORD"
    return
  fi
  local first second
  read -r -s -p 'Browser password (Enter to generate): ' first; printf '\n'
  if [[ -z "$first" ]]; then
    ARGUS_BASIC_AUTH_PASSWORD="$(new_password)"
    GENERATED_BASIC_AUTH_PASSWORD="$ARGUS_BASIC_AUTH_PASSWORD"
    return
  fi
  read -r -s -p 'Confirm browser password: ' second; printf '\n'
  [[ "$first" == "$second" ]] || die "passwords do not match"
  ARGUS_BASIC_AUTH_PASSWORD="$first"
}

device_authorize() {
  if [[ -n "${ARGUS_REGISTRY_TOKEN:-}" ]]; then
    warn "using emergency registry compatibility path"
    return
  fi
  local response code url interval expires started status
  response="$(curl -fsS -X POST "$DISTRIBUTION_URL/api/device/start")" || die "could not start GitHub device authorization"
  DEVICE_SESSION="$(jq -er '.id' <<<"$response")"
  code="$(jq -er '.user_code' <<<"$response")"
  url="$(jq -er '.verification_uri' <<<"$response")"
  interval="$(jq -er '.interval // 5' <<<"$response")"
  expires="$(jq -er '.expires_in' <<<"$response")"
  printf '\nAuthorize this server with GitHub:\n\n  Code: %s\n  Open: %s\n\n' "$code" "$url"
  started=$SECONDS
  while (( SECONDS - started < expires )); do
    sleep "$interval"
    response="$(curl -sS -w '\n%{http_code}' "$DISTRIBUTION_URL/api/device/sessions/$DEVICE_SESSION")"
    status="$(tail -n1 <<<"$response")"; response="$(sed '$d' <<<"$response")"
    if [[ "$status" == "200" && "$(jq -r '.status' <<<"$response")" == "authorized" ]]; then
      printf 'GitHub authorization confirmed.\n'
      return
    fi
    [[ "$status" == "202" ]] || die "GitHub authorization was denied or expired"
    interval="$(jq -r '.retry_after // 5' <<<"$response")"
    printf '.'
  done
  die "GitHub device code expired; rerun the installer"
}

release_public_key() {
  cat <<'EOF'
-----BEGIN PUBLIC KEY-----
MCowBQYDK2VwAyEAGD+O6E423q4GIkGMWc7kYqtsOH0VHJPTBoHPy90B1NQ=
-----END PUBLIC KEY-----
EOF
}

download_release_bundle() {
  local bundle="$1" response object expected size grant actual
  RELEASE_TMP="$(mktemp -d)"; chmod 0700 "$RELEASE_TMP"
  response="$(curl -fsS -H "x-argus-device-session: $DEVICE_SESSION" "$DISTRIBUTION_URL/api/releases/$RELEASE_CHANNEL/$bundle/manifest")" || die "could not retrieve release manifest"
  jq -er '.manifest' <<<"$response" >"$RELEASE_TMP/manifest.json"
  jq -er '.signature' <<<"$response" | base64 -d >"$RELEASE_TMP/manifest.sig" || die "release manifest signature is invalid"
  release_public_key >"$RELEASE_TMP/release-public.pem"
  openssl pkeyutl -verify -pubin -inkey "$RELEASE_TMP/release-public.pem" -rawin -in "$RELEASE_TMP/manifest.json" -sigfile "$RELEASE_TMP/manifest.sig" >/dev/null \
    || die "release manifest signature verification failed"
  [[ "$(jq -r '.architecture' "$RELEASE_TMP/manifest.json")" == "amd64" ]] || die "release architecture does not match this host"
  [[ "$(jq -r '.bundle' "$RELEASE_TMP/manifest.json")" == "$bundle" ]] || die "release manifest contains the wrong bundle"
  ARGUS_VERSION="$(jq -er '.commit_sha' "$RELEASE_TMP/manifest.json")"; is_revision "$ARGUS_VERSION" || die "release manifest has an invalid revision"
  object="$(jq -er '.artifact.object' "$RELEASE_TMP/manifest.json")"
  expected="$(jq -er '.artifact.sha256' "$RELEASE_TMP/manifest.json")"
  size="$(jq -er '.artifact.size' "$RELEASE_TMP/manifest.json")"
  grant="$(curl -fsS -H "x-argus-device-session: $DEVICE_SESSION" -H 'content-type: application/json' -d "$(jq -nc --arg object "$object" '{object:$object}')" "$DISTRIBUTION_URL/api/artifact-grants" | jq -er '.url')"
  curl -fL --retry 3 --continue-at - "$grant" -o "$RELEASE_TMP/bundle.tar.zst" || die "release download failed"
  actual="$(stat -c %s "$RELEASE_TMP/bundle.tar.zst")"; [[ "$actual" == "$size" ]] || die "release bundle size verification failed"
  printf '%s  %s\n' "$expected" "$RELEASE_TMP/bundle.tar.zst" | sha256sum -c - >/dev/null || die "release bundle checksum verification failed"
  mkdir "$RELEASE_TMP/unpacked"
  tar --zstd -xf "$RELEASE_TMP/bundle.tar.zst" -C "$RELEASE_TMP/unpacked" || die "could not extract release bundle"
}

install_prerequisites() {
  export DEBIAN_FRONTEND=noninteractive
  apt-get update
  apt-get install -y --no-install-recommends \
    ca-certificates curl jq openssl zstd iproute2 ufw unattended-upgrades
}

install_docker() {
  if command -v docker >/dev/null 2>&1; then
    docker compose version >/dev/null 2>&1 \
      || die "Docker is installed but the Compose plugin is missing; install docker-compose-plugin before rerunning"
    systemctl enable --now docker >/dev/null
    return
  fi

  . /etc/os-release
  case "${ID:-}" in
    ubuntu|debian) ;;
    *) die "first-test installer supports Ubuntu or Debian only" ;;
  esac

  for conflict in docker.io docker-compose docker-compose-v2 podman-docker containerd runc; do
    if dpkg-query -W -f='${Status}' "$conflict" 2>/dev/null | grep -q 'install ok installed'; then
      die "conflicting package '$conflict' is installed; the first-test installer will not remove an existing container stack automatically"
    fi
  done

  install -m 0755 -d /etc/apt/keyrings
  curl -fsSL "https://download.docker.com/linux/${ID}/gpg" -o /etc/apt/keyrings/docker.asc
  chmod a+r /etc/apt/keyrings/docker.asc
  local codename="${UBUNTU_CODENAME:-${VERSION_CODENAME:-}}"
  [[ -n "$codename" ]] || die "could not determine distribution codename"
  cat >/etc/apt/sources.list.d/docker.sources <<EOF
Types: deb
URIs: https://download.docker.com/linux/${ID}
Suites: ${codename}
Components: stable
Architectures: $(dpkg --print-architecture)
Signed-By: /etc/apt/keyrings/docker.asc
EOF
  apt-get update
  apt-get install -y docker-ce docker-ce-cli containerd.io docker-buildx-plugin docker-compose-plugin
  systemctl enable --now docker >/dev/null
  docker compose version >/dev/null
}

preflight() {
  require_root
  [[ -r /etc/os-release ]] || die "/etc/os-release is required"
  . /etc/os-release
  case "${ID:-}" in
    ubuntu|debian) ;;
    *) die "first-test installer supports Ubuntu or Debian only" ;;
  esac
  [[ "$(dpkg --print-architecture)" == "amd64" ]] \
    || die "first-test installer currently supports amd64 only"

  install_prerequisites
  if [[ "$INSTALL_MODE" == "control-plane" ]]; then install_docker; fi

  if [[ "$INSTALL_MODE" == "control-plane" && ! -f "$COMPOSE_FILE" ]]; then
    for port in 80 443; do
      if ss -ltnH | awk '{print $4}' | grep -Eq ":${port}$"; then
        die "TCP port ${port} is already in use; use a clean first-test host or free the port"
      fi
    done
  fi
}

load_or_create_configuration() {
  local requested_registry="${ARGUS_REGISTRY:-}"
  local requested_version="${ARGUS_VERSION:-}"
  local requested_domain="${ARGUS_DOMAIN:-}"
  local requested_content_domain="${ARGUS_CONTENT_DOMAIN:-}"
  local requested_basic_user="${ARGUS_BASIC_AUTH_USER:-}"
  local requested_basic_password="${ARGUS_BASIC_AUTH_PASSWORD:-}"
  local installed_version=""

  if [[ -f "$ENV_FILE" ]]; then
    EXISTING_INSTALL=1
    log "existing Argus environment found; preserving generated IDs, secrets and installed revision"
    set -a
    # shellcheck disable=SC1090
    . "$ENV_FILE"
    set +a
    installed_version="${ARGUS_VERSION:-}"
  fi

  ARGUS_REGISTRY="${requested_registry:-${ARGUS_REGISTRY:-ghcr.io/noah-bozkurt}}"
  if [[ "$EXISTING_INSTALL" == "1" && -n "$installed_version" ]]; then
    if [[ -n "$requested_version" && "$requested_version" != "$installed_version" ]]; then
      warn "ignoring requested ARGUS_VERSION=$requested_version on an existing install; use 'argusctl update --version $requested_version' for version changes"
    fi
    ARGUS_VERSION="$installed_version"
  else
    ARGUS_VERSION="${requested_version:-${ARGUS_VERSION:-main}}"
  fi

  ARGUS_DOMAIN="${requested_domain:-${ARGUS_DOMAIN:-}}"
  prompt_required ARGUS_DOMAIN "Primary Argus domain (for example argus.example.com)"
  ARGUS_DOMAIN="$(printf '%s' "$ARGUS_DOMAIN" | tr '[:upper:]' '[:lower:]')"

  ARGUS_CONTENT_DOMAIN="${requested_content_domain:-${ARGUS_CONTENT_DOMAIN:-content.${ARGUS_DOMAIN}}}"
  ARGUS_CONTENT_DOMAIN="$(printf '%s' "$ARGUS_CONTENT_DOMAIN" | tr '[:upper:]' '[:lower:]')"
  validate_domain "$ARGUS_DOMAIN"
  validate_domain "$ARGUS_CONTENT_DOMAIN"
  [[ "$ARGUS_DOMAIN" != "$ARGUS_CONTENT_DOMAIN" ]] || die "Web and content domains must differ"

  ARGUS_BASIC_AUTH_USER="${requested_basic_user:-${ARGUS_BASIC_AUTH_USER:-argus}}"
  validate_basic_user "$ARGUS_BASIC_AUTH_USER"
  if [[ -n "$requested_basic_password" ]]; then
    ARGUS_BASIC_AUTH_PASSWORD="$requested_basic_password"
  elif [[ -z "${ARGUS_BASIC_AUTH_PASSWORD:-}" ]]; then
    prompt_password
  fi

  ARGUS_OPERATOR_EMAIL="${ARGUS_OPERATOR_EMAIL:-operator@argus.local}"
  ARGUS_ORG_NAME="${ARGUS_ORG_NAME:-Argus}"

  ARGUS_POSTGRES_PASSWORD="${ARGUS_POSTGRES_PASSWORD:-$(new_secret)}"
  ARGUS_WEB_API_TOKEN="${ARGUS_WEB_API_TOKEN:-$(new_secret)}"
  ARGUS_WORKER_TOKEN="${ARGUS_WORKER_TOKEN:-$(new_secret)}"
  ARGUS_CONTENT_SYNC_TOKEN="${ARGUS_CONTENT_SYNC_TOKEN:-$(new_secret)}"
  PAYLOAD_SECRET="${PAYLOAD_SECRET:-$(new_secret)}"
  ARGUS_ORG_ID="${ARGUS_ORG_ID:-$(new_uuid)}"
  ARGUS_USER_ID="${ARGUS_USER_ID:-$(new_uuid)}"
  ARGUS_BOOTSTRAP_PROJECT_ID="${ARGUS_BOOTSTRAP_PROJECT_ID:-$(new_uuid)}"
  ARGUS_BOOTSTRAP_ENVIRONMENT_ID="${ARGUS_BOOTSTRAP_ENVIRONMENT_ID:-$(new_uuid)}"
  ARGUS_SERVER_ID="${ARGUS_SERVER_ID:-$(new_uuid)}"
  ARGUS_RUST_LOG="${ARGUS_RUST_LOG:-info}"
  ARGUS_GITHUB_TOKEN="${ARGUS_GITHUB_TOKEN:-}"

  export ARGUS_REGISTRY ARGUS_VERSION ARGUS_DOMAIN ARGUS_CONTENT_DOMAIN
  export ARGUS_BASIC_AUTH_USER ARGUS_BASIC_AUTH_PASSWORD
  export ARGUS_POSTGRES_PASSWORD ARGUS_WEB_API_TOKEN ARGUS_WORKER_TOKEN
  export ARGUS_CONTENT_SYNC_TOKEN PAYLOAD_SECRET ARGUS_ORG_ID ARGUS_USER_ID
  export ARGUS_BOOTSTRAP_PROJECT_ID ARGUS_BOOTSTRAP_ENVIRONMENT_ID ARGUS_SERVER_ID
  export ARGUS_RUST_LOG ARGUS_GITHUB_TOKEN
}

registry_login_if_configured() {
  if [[ -z "${ARGUS_REGISTRY_TOKEN:-}" ]]; then
    return
  fi
  [[ -n "${ARGUS_REGISTRY_USERNAME:-}" ]] \
    || die "ARGUS_REGISTRY_USERNAME is required when ARGUS_REGISTRY_TOKEN is set"

  DOCKER_CONFIG_DIR="$(mktemp -d)"
  chmod 0700 "$DOCKER_CONFIG_DIR"
  export DOCKER_CONFIG="$DOCKER_CONFIG_DIR"
  local registry_host="${ARGUS_REGISTRY%%/*}"
  printf '%s' "$ARGUS_REGISTRY_TOKEN" \
    | docker login "$registry_host" -u "$ARGUS_REGISTRY_USERNAME" --password-stdin >/dev/null
}

resolve_existing_mutable_revision() {
  if [[ "$EXISTING_INSTALL" != "1" ]] || is_revision "$ARGUS_VERSION"; then
    return
  fi
  [[ -f "$COMPOSE_FILE" ]] || die "existing install uses mutable version '$ARGUS_VERSION' but compose.yaml is missing"

  local cid image_id revision
  cid="$(docker compose --project-directory "$INSTALL_DIR" --env-file "$ENV_FILE" -f "$COMPOSE_FILE" ps -q control-api)"
  [[ -n "$cid" ]] || die "existing install uses mutable version '$ARGUS_VERSION' and the Control API container is not running; use argusctl diagnostics before rerunning the installer"
  image_id="$(docker inspect -f '{{.Image}}' "$cid")"
  revision="$(docker image inspect "$image_id" --format '{{ index .Config.Labels "org.opencontainers.image.revision" }}')"
  is_revision "$revision" || die "could not recover immutable revision from the running Control API image"

  log "normalizing legacy mutable installed version '$ARGUS_VERSION' to running revision $revision"
  ARGUS_VERSION="$revision"
  export ARGUS_VERSION
}

pull_host_bundle() {
  if [[ -z "${ARGUS_REGISTRY_TOKEN:-}" ]]; then
    download_release_bundle control-plane
    local root="$RELEASE_TMP/unpacked"
    for required in images.tar out/argus-agent out/argus-helper out/argusctl deploy/compose.yaml deploy/Caddyfile.template deploy/systemd/argus-agent.service deploy/systemd/argus-helper.service; do
      [[ -s "$root/$required" ]] || die "verified control-plane bundle is incomplete: $required"
    done
    docker load -i "$root/images.tar" >/dev/null
    mkdir -p "$INSTALL_DIR" "$CONFIG_DIR" "$STATE_DIR" "$STATE_DIR/backups"
    install -m 0644 "$root/deploy/compose.yaml" "$COMPOSE_FILE"
    install -m 0644 "$root/deploy/Caddyfile.template" "$INSTALL_DIR/Caddyfile.template"
    install -m 0755 "$root/out/argus-agent" /usr/local/bin/argus-agent
    install -m 0755 "$root/out/argus-helper" /usr/local/bin/argus-helper
    install -m 0755 "$root/out/argusctl" /usr/local/bin/argusctl
    install -m 0644 "$root/deploy/systemd/argus-agent.service" /etc/systemd/system/argus-agent.service
    install -m 0644 "$root/deploy/systemd/argus-helper.service" /etc/systemd/system/argus-helper.service
    systemctl daemon-reload
    export ARGUS_VERSION
    return
  fi
  registry_login_if_configured
  resolve_existing_mutable_revision

  local requested="$ARGUS_VERSION"
  local image="${ARGUS_REGISTRY}/argus-host-tools:${requested}"
  if ! docker pull "$image"; then
    if [[ -z "${ARGUS_REGISTRY_TOKEN:-}" ]]; then
      die "could not pull private Argus images. Set ARGUS_REGISTRY_USERNAME and a read-only ARGUS_REGISTRY_TOKEN, then rerun"
    fi
    die "could not pull $image"
  fi

  local resolved_revision
  resolved_revision="$(docker image inspect "$image" --format '{{ index .Config.Labels "org.opencontainers.image.revision" }}')"
  is_revision "$resolved_revision" \
    || die "$image is not a verified Argus artifact with an immutable revision label"

  if [[ "$EXISTING_INSTALL" == "1" ]] && is_revision "$requested" && [[ "$resolved_revision" != "$requested" ]]; then
    die "installed revision $requested resolved to unexpected artifact revision $resolved_revision"
  fi

  ARGUS_VERSION="$resolved_revision"
  export ARGUS_VERSION
  image="${ARGUS_REGISTRY}/argus-host-tools:${ARGUS_VERSION}"
  if [[ "$requested" != "$ARGUS_VERSION" ]]; then
    log "pinning requested version '$requested' to immutable revision $ARGUS_VERSION"
    docker pull "$image" >/dev/null
  fi

  mkdir -p "$INSTALL_DIR" "$CONFIG_DIR" "$STATE_DIR" "$STATE_DIR/backups"
  chmod 0750 "$CONFIG_DIR" "$STATE_DIR"

  HOST_TOOLS_CONTAINER="$(docker create "$image")"
  local tmp
  tmp="$(mktemp -d)"
  docker cp "$HOST_TOOLS_CONTAINER:/out/." "$tmp/"
  docker cp "$HOST_TOOLS_CONTAINER:/deploy/compose.yaml" "$INSTALL_DIR/compose.yaml"
  docker cp "$HOST_TOOLS_CONTAINER:/deploy/Caddyfile.template" "$INSTALL_DIR/Caddyfile.template"
  docker cp "$HOST_TOOLS_CONTAINER:/deploy/systemd/argus-agent.service" "$tmp/argus-agent.service"
  docker cp "$HOST_TOOLS_CONTAINER:/deploy/systemd/argus-helper.service" "$tmp/argus-helper.service"

  install -m 0755 "$tmp/argus-agent" /usr/local/bin/argus-agent
  install -m 0755 "$tmp/argus-helper" /usr/local/bin/argus-helper
  install -m 0755 "$tmp/argusctl" /usr/local/bin/argusctl
  install -m 0644 "$tmp/argus-agent.service" /etc/systemd/system/argus-agent.service
  install -m 0644 "$tmp/argus-helper.service" /etc/systemd/system/argus-helper.service
  rm -rf "$tmp"
  docker rm "$HOST_TOOLS_CONTAINER" >/dev/null
  HOST_TOOLS_CONTAINER=""
  systemctl daemon-reload
}

ensure_argus_user() {
  if ! getent group argus >/dev/null; then
    groupadd --system argus
  fi
  if ! id argus >/dev/null 2>&1; then
    useradd --system --gid argus --home-dir "$STATE_DIR" --shell /usr/sbin/nologin argus
  fi
  mkdir -p "$CONFIG_DIR" "$STATE_DIR" "$STATE_DIR/backups"
  chown root:argus "$CONFIG_DIR"
  chmod 0750 "$CONFIG_DIR"
  chown argus:argus "$STATE_DIR"
  chmod 0750 "$STATE_DIR"
}

write_runtime_env() {
  cat >"$ENV_FILE" <<EOF
ARGUS_REGISTRY=${ARGUS_REGISTRY}
ARGUS_VERSION=${ARGUS_VERSION}
ARGUS_DOMAIN=${ARGUS_DOMAIN}
ARGUS_CONTENT_DOMAIN=${ARGUS_CONTENT_DOMAIN}
ARGUS_BASIC_AUTH_USER=${ARGUS_BASIC_AUTH_USER}
ARGUS_BASIC_AUTH_PASSWORD=${ARGUS_BASIC_AUTH_PASSWORD}
ARGUS_POSTGRES_PASSWORD=${ARGUS_POSTGRES_PASSWORD}
ARGUS_WEB_API_TOKEN=${ARGUS_WEB_API_TOKEN}
ARGUS_WORKER_TOKEN=${ARGUS_WORKER_TOKEN}
ARGUS_CONTENT_SYNC_TOKEN=${ARGUS_CONTENT_SYNC_TOKEN}
PAYLOAD_SECRET=${PAYLOAD_SECRET}
ARGUS_ORG_ID=${ARGUS_ORG_ID}
ARGUS_USER_ID=${ARGUS_USER_ID}
ARGUS_BOOTSTRAP_PROJECT_ID=${ARGUS_BOOTSTRAP_PROJECT_ID}
ARGUS_BOOTSTRAP_ENVIRONMENT_ID=${ARGUS_BOOTSTRAP_ENVIRONMENT_ID}
ARGUS_SERVER_ID=${ARGUS_SERVER_ID}
ARGUS_GITHUB_TOKEN=${ARGUS_GITHUB_TOKEN}
ARGUS_RUST_LOG=${ARGUS_RUST_LOG}
EOF
  chmod 0600 "$ENV_FILE"
}

generate_caddy_config() {
  if [[ -f "$CADDY_FILE" && "${ARGUS_RECONFIGURE_CADDY:-0}" != "1" ]]; then
    log "preserving existing Caddyfile"
    return
  fi

  local hash
  hash="$(docker run --rm caddy:2-alpine caddy hash-password --plaintext "$ARGUS_BASIC_AUTH_PASSWORD")"
  cp "$INSTALL_DIR/Caddyfile.template" "$CADDY_FILE"
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

configure_firewall_if_active() {
  if command -v ufw >/dev/null 2>&1 && ufw status | grep -q '^Status: active'; then
    log "UFW is active; allowing Argus HTTP/HTTPS ingress"
    ufw allow 80/tcp >/dev/null
    ufw allow 443/tcp >/dev/null
    ufw allow 443/udp >/dev/null
  fi
}

compose() {
  docker compose --project-directory "$INSTALL_DIR" --env-file "$ENV_FILE" -f "$COMPOSE_FILE" "$@"
}

start_control_plane() {
  compose config >/dev/null
  if [[ -n "${ARGUS_REGISTRY_TOKEN:-}" ]]; then compose pull; fi
  configure_firewall_if_active
  compose up -d

  log "waiting for Control API migrations and health"
  for _ in $(seq 1 90); do
    if curl -fsS http://127.0.0.1:8080/health >/dev/null 2>&1; then
      return
    fi
    sleep 2
  done
  compose ps || true
  compose logs --tail=120 control-api postgres || true
  die "Control API did not become healthy"
}

parse_setup_code() {
  local code="$1" decoded
  decoded="$(printf '%s' "$code" | base64 -d 2>/dev/null)" || die "invalid setup code"
  ARGUS_CONTROL_PLANE_URL="$(jq -er '.control_plane_url' <<<"$decoded")"
  ARGUS_SERVER_ID="$(jq -er '.server_id' <<<"$decoded")"
  ARGUS_ENROLLMENT_TOKEN="$(jq -er '.enrollment_token' <<<"$decoded")"
  if [[ "$ARGUS_CONTROL_PLANE_URL" != https://* && "${ARGUS_ALLOW_INSECURE_CONTROL_PLANE:-0}" != "1" ]]; then
    die "remote control plane must use HTTPS"
  fi
}

install_managed_node() {
  local setup_code="${ARGUS_SETUP_CODE:-}"
  if [[ -z "$setup_code" ]]; then
    [[ -t 0 ]] || die "ARGUS_SETUP_CODE is required in non-interactive agent mode"
    read -r -s -p 'Argus setup code: ' setup_code; printf '\n'
  fi
  parse_setup_code "$setup_code"; unset setup_code ARGUS_SETUP_CODE
  download_release_bundle managed-node
  local root="$RELEASE_TMP/unpacked"
  for required in out/argus-agent out/argus-helper out/argusctl deploy/systemd/argus-agent.service deploy/systemd/argus-helper.service; do
    [[ -s "$root/$required" ]] || die "verified managed-node bundle is incomplete: $required"
  done
  ensure_argus_user
  install -m 0755 "$root/out/argus-agent" /usr/local/bin/argus-agent
  install -m 0755 "$root/out/argus-helper" /usr/local/bin/argus-helper
  install -m 0755 "$root/out/argusctl" /usr/local/bin/argusctl
  install -m 0644 "$root/deploy/systemd/argus-agent.service" /etc/systemd/system/argus-agent.service
  install -m 0644 "$root/deploy/systemd/argus-helper.service" /etc/systemd/system/argus-helper.service
  systemctl daemon-reload
  write_helper_env
  cat >"$CONFIG_DIR/agent.env" <<EOF
ARGUS_CONTROL_PLANE_URL=${ARGUS_CONTROL_PLANE_URL}
ARGUS_SERVER_ID=${ARGUS_SERVER_ID}
ARGUS_AGENT_CONFIG=${STATE_DIR}/agent.json
ARGUS_HELPER_SOCKET=/run/argus/helper.sock
ARGUS_ENROLLMENT_TOKEN=${ARGUS_ENROLLMENT_TOKEN}
RUST_LOG=${ARGUS_RUST_LOG:-info}
EOF
  chown root:argus "$CONFIG_DIR/agent.env"; chmod 0640 "$CONFIG_DIR/agent.env"
  systemctl enable --now argus-helper.service argus-agent.service
  for _ in $(seq 1 60); do [[ -s "$STATE_DIR/agent.json" ]] && break; sleep 2; done
  [[ -s "$STATE_DIR/agent.json" ]] || die "Argus Agent did not enroll successfully"
  sed -i '/^ARGUS_ENROLLMENT_TOKEN=/d' "$CONFIG_DIR/agent.env"
  unset ARGUS_ENROLLMENT_TOKEN
  systemctl restart argus-agent.service
  systemctl is-active --quiet argus-helper.service || die "argus-helper.service is not active"
  systemctl is-active --quiet argus-agent.service || die "argus-agent.service is not active"
  printf '\nArgus managed node is connected.\nControl plane: %s\nServer ID:     %s\nRevision:      %s\n' "$ARGUS_CONTROL_PLANE_URL" "$ARGUS_SERVER_ID" "$ARGUS_VERSION"
}

bootstrap_control_plane() {
  local hostname_value
  hostname_value="$(hostname -f 2>/dev/null || hostname)"

  compose exec -T postgres psql -v ON_ERROR_STOP=1 -U argus -d argus \
    -v org_id="$ARGUS_ORG_ID" \
    -v user_id="$ARGUS_USER_ID" \
    -v project_id="$ARGUS_BOOTSTRAP_PROJECT_ID" \
    -v environment_id="$ARGUS_BOOTSTRAP_ENVIRONMENT_ID" \
    -v server_id="$ARGUS_SERVER_ID" \
    -v org_name="$ARGUS_ORG_NAME" \
    -v operator_email="$ARGUS_OPERATOR_EMAIL" \
    -v host_name="$hostname_value" <<'SQL'
INSERT INTO organizations(id,name)
VALUES (:'org_id'::uuid, :'org_name')
ON CONFLICT(id) DO NOTHING;

INSERT INTO users(id,organization_id,email)
VALUES (:'user_id'::uuid, :'org_id'::uuid, :'operator_email')
ON CONFLICT(id) DO NOTHING;

INSERT INTO projects(id,organization_id,name,client_id,description,preset,status,tags)
VALUES (:'project_id'::uuid, :'org_id'::uuid, 'Argus Control Plane', NULL,
        'Bootstrap project for the server running Argus itself.', 'infrastructure', 'ACTIVE', '[]'::jsonb)
ON CONFLICT(id) DO NOTHING;

INSERT INTO environments(
  id,organization_id,project_id,name,type,description,is_protected,sort_order
)
VALUES (
  :'environment_id'::uuid, :'org_id'::uuid, :'project_id'::uuid,
  'Control Plane', 'production', 'Environment containing the Argus host.', TRUE, 0
)
ON CONFLICT(id) DO NOTHING;

INSERT INTO servers(id,organization_id,project_id,environment_id,hostname)
VALUES (
  :'server_id'::uuid, :'org_id'::uuid, :'project_id'::uuid,
  :'environment_id'::uuid, :'host_name'
)
ON CONFLICT(id) DO NOTHING;
SQL
}

write_helper_env() {
  cat >"$CONFIG_DIR/helper.env" <<EOF
ARGUS_HELPER_SOCKET=/run/argus/helper.sock
ARGUS_ALLOWED_SERVICES=${ARGUS_ALLOWED_SERVICES:-}
ARGUS_BACKUP_DIR=${STATE_DIR}/backups
EOF
  chown root:argus "$CONFIG_DIR/helper.env"
  chmod 0640 "$CONFIG_DIR/helper.env"
}

write_agent_env() {
  local enrollment_token="${1:-}"
  cat >"$CONFIG_DIR/agent.env" <<EOF
ARGUS_CONTROL_PLANE_URL=http://127.0.0.1:8080
ARGUS_SERVER_ID=${ARGUS_SERVER_ID}
ARGUS_AGENT_CONFIG=${STATE_DIR}/agent.json
ARGUS_HELPER_SOCKET=/run/argus/helper.sock
ARGUS_MANAGED_SERVICES=${ARGUS_MANAGED_SERVICES:-}
RUST_LOG=${ARGUS_RUST_LOG}
EOF
  if [[ -n "$enrollment_token" ]]; then
    printf 'ARGUS_ENROLLMENT_TOKEN=%s\n' "$enrollment_token" >>"$CONFIG_DIR/agent.env"
  fi
  chown root:argus "$CONFIG_DIR/agent.env"
  chmod 0640 "$CONFIG_DIR/agent.env"
}

enroll_local_agent() {
  write_helper_env
  systemctl enable --now argus-helper.service

  if [[ -s "$STATE_DIR/agent.json" ]]; then
    log "existing local Agent identity found; skipping enrollment"
    write_agent_env
    systemctl enable --now argus-agent.service
    return
  fi

  local payload response enrollment_token
  payload="$(jq -nc --arg server "$ARGUS_SERVER_ID" '{server_id:$server,ttl_seconds:1800}')"
  response="$(curl -fsS \
    -H "Authorization: Bearer ${ARGUS_WEB_API_TOKEN}" \
    -H "x-argus-org-id: ${ARGUS_ORG_ID}" \
    -H "x-argus-user-id: ${ARGUS_USER_ID}" \
    -H 'content-type: application/json' \
    -d "$payload" \
    http://127.0.0.1:8080/enrollment/tokens)"
  enrollment_token="$(jq -er '.token' <<<"$response")"

  write_agent_env "$enrollment_token"
  systemctl enable --now argus-agent.service

  for _ in $(seq 1 60); do
    if [[ -s "$STATE_DIR/agent.json" ]]; then
      local enrolled
      enrolled="$(compose exec -T postgres psql -U argus -d argus -Atc \
        "SELECT EXISTS(SELECT 1 FROM agents WHERE server_id='${ARGUS_SERVER_ID}'::uuid)")"
      if [[ "$enrolled" == "t" ]]; then
        write_agent_env
        return
      fi
    fi
    sleep 2
  done

  journalctl -u argus-helper.service -n 80 --no-pager || true
  journalctl -u argus-agent.service -n 80 --no-pager || true
  die "local Argus Agent did not enroll successfully"
}

verify_compose_service() {
  local service="$1"
  local require_health="${2:-1}"
  local cid running health
  cid="$(compose ps -q "$service")"
  [[ -n "$cid" ]] || die "Compose service '$service' has no container"
  running="$(docker inspect -f '{{.State.Running}}' "$cid")"
  [[ "$running" == "true" ]] || die "Compose service '$service' is not running"
  if [[ "$require_health" == "1" ]]; then
    health="$(docker inspect -f '{{if .State.Health}}{{.State.Health.Status}}{{else}}missing{{end}}' "$cid")"
    [[ "$health" == "healthy" ]] || die "Compose service '$service' is not healthy (status: $health)"
  fi
}

wait_for_https() {
  local url="$1"
  for _ in $(seq 1 45); do
    local code
    code="$(curl -sS --connect-timeout 5 -o /dev/null -w '%{http_code}' "$url" 2>/dev/null || true)"
    if [[ "$code" == "401" || "$code" == "200" || "$code" == "302" || "$code" == "307" || "$code" == "308" ]]; then
      return 0
    fi
    sleep 2
  done
  return 1
}

verify_installation() {
  log "running first-test health checks"
  compose ps

  verify_compose_service postgres
  verify_compose_service control-api
  verify_compose_service worker
  verify_compose_service web
  verify_compose_service content
  verify_compose_service caddy 0

  systemctl is-active --quiet argus-helper.service || die "argus-helper.service is not active"
  systemctl is-active --quiet argus-agent.service || die "argus-agent.service is not active"
  curl -fsS http://127.0.0.1:8080/health >/dev/null || die "Control API loopback health failed"

  if ! wait_for_https "https://${ARGUS_DOMAIN}/"; then
    compose logs --tail=120 caddy || true
    die "Argus HTTPS did not become reachable. Verify A/AAAA DNS for ${ARGUS_DOMAIN} and external firewall access to ports 80/443"
  fi
  if ! wait_for_https "https://${ARGUS_CONTENT_DOMAIN}/"; then
    compose logs --tail=120 caddy || true
    die "Payload HTTPS did not become reachable. Verify A/AAAA DNS for ${ARGUS_CONTENT_DOMAIN} and external firewall access to ports 80/443"
  fi

  curl -fsS -u "${ARGUS_BASIC_AUTH_USER}:${ARGUS_BASIC_AUTH_PASSWORD}" \
    "https://${ARGUS_DOMAIN}/healthz" >/dev/null \
    || die "authenticated Web health check failed"
  curl -fsS -u "${ARGUS_BASIC_AUTH_USER}:${ARGUS_BASIC_AUTH_PASSWORD}" \
    "https://${ARGUS_CONTENT_DOMAIN}/healthz" >/dev/null \
    || die "authenticated Payload health check failed"
}

print_summary() {
  printf '\nArgus first-test installation is ready.\n\n'
  printf 'Web:      https://%s\n' "$ARGUS_DOMAIN"
  printf 'Content:  https://%s\n' "$ARGUS_CONTENT_DOMAIN"
  printf 'User:     %s\n' "$ARGUS_BASIC_AUTH_USER"
  printf 'Password: %s\n' "$ARGUS_BASIC_AUTH_PASSWORD"
  if [[ -n "$GENERATED_BASIC_AUTH_PASSWORD" ]]; then
    printf '\nA new first-test password was generated and stored only in the root-readable %s file.\n' "$ENV_FILE"
  fi
  printf '\nVersion:  %s\n' "$ARGUS_VERSION"
  printf 'Recovery: sudo grep ^ARGUS_BASIC_AUTH_PASSWORD= %s\n' "$ENV_FILE"
  printf 'Config:   %s\n' "$INSTALL_DIR"
  printf 'Agent:    %s\n' "$ARGUS_SERVER_ID"
  printf '\nUseful diagnostics:\n'
  printf '  cd %s && docker compose --env-file .env ps\n' "$INSTALL_DIR"
  printf '  journalctl -u argus-agent -u argus-helper --no-pager -n 100\n'
  printf '  argusctl status\n'
  printf '  sudo argusctl smoke\n'
  printf '\nTransactional update:\n'
  printf '  sudo argusctl update --version stable\n'
}

main() {
  select_mode
  install -m 0700 -d "$LOG_DIR"
  touch "$LOG_FILE"; chmod 0600 "$LOG_FILE"
  stage "Checking host requirements"
  preflight
  stage "Authorizing release access"
  device_authorize
  if [[ "$INSTALL_MODE" == "agent" ]]; then
    stage "Installing managed-node bundle"
    install_managed_node
    return
  fi
  stage "Collecting control-plane configuration"
  load_or_create_configuration
  log "installing Argus for ${ARGUS_DOMAIN}"
  stage "Downloading and verifying release"
  pull_host_bundle
  stage "Configuring services"
  ensure_argus_user
  write_runtime_env
  generate_caddy_config
  stage "Starting the control plane"
  start_control_plane
  bootstrap_control_plane
  enroll_local_agent
  stage "Verifying health"
  verify_installation
  print_summary
}

main "$@"
