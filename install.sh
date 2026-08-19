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

log() { printf '[argus] %s\n' "$*"; }
warn() { printf '[argus] warning: %s\n' "$*" >&2; }
die() { printf '[argus] error: %s\n' "$*" >&2; exit 1; }

cleanup() {
  if [[ -n "$HOST_TOOLS_CONTAINER" ]]; then
    docker rm -f "$HOST_TOOLS_CONTAINER" >/dev/null 2>&1 || true
  fi
  if [[ -n "$DOCKER_CONFIG_DIR" ]]; then
    rm -rf "$DOCKER_CONFIG_DIR"
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

install_prerequisites() {
  export DEBIAN_FRONTEND=noninteractive
  apt-get update
  apt-get install -y --no-install-recommends \
    ca-certificates curl jq openssl iproute2 ufw unattended-upgrades
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
  install_docker

  if [[ ! -f "$COMPOSE_FILE" ]]; then
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
    ARGUS_BASIC_AUTH_PASSWORD="$(new_password)"
    GENERATED_BASIC_AUTH_PASSWORD="$ARGUS_BASIC_AUTH_PASSWORD"
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
  if [[ "$EXISTING_INSTALL" != "1" || is_revision "$ARGUS_VERSION" ]]; then
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

  if [[ "$EXISTING_INSTALL" == "1" && is_revision "$requested" && "$resolved_revision" != "$requested" ]]; then
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
  compose pull
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
  printf 'Config:   %s\n' "$INSTALL_DIR"
  printf 'Agent:    %s\n' "$ARGUS_SERVER_ID"
  printf '\nUseful diagnostics:\n'
  printf '  cd %s && docker compose --env-file .env ps\n' "$INSTALL_DIR"
  printf '  journalctl -u argus-agent -u argus-helper --no-pager -n 100\n'
  printf '  argusctl status\n'
  printf '  sudo argusctl smoke\n'
  printf '\nTransactional update (requires read-only registry credentials):\n'
  printf '  sudo -E argusctl update --version main\n'
}

main() {
  preflight
  load_or_create_configuration
  log "installing Argus ${ARGUS_VERSION} for ${ARGUS_DOMAIN}"
  pull_host_bundle
  ensure_argus_user
  write_runtime_env
  generate_caddy_config
  start_control_plane
  bootstrap_control_plane
  enroll_local_agent
  verify_installation
  print_summary
}

main "$@"
