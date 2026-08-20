#!/usr/bin/env bash
set -Eeuo pipefail
umask 077

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "$SCRIPT_DIR/.." && pwd)"
INSTALL_DIR="${ARGUS_INSTALL_DIR:-/opt/argus}"
STATE_DIR="${ARGUS_STATE_DIR:-/var/lib/argus}"
ENV_FILE="$INSTALL_DIR/.env"
ACCEPTANCE_DIR="${ARGUS_ACCEPTANCE_DIR:-$STATE_DIR/acceptance/first-server}"
REPORT_FILE="$ACCEPTANCE_DIR/report.txt"
UPDATE_VERSION="${ARGUS_ACCEPTANCE_UPDATE_VERSION:-main}"

log() { printf '[argus-acceptance] %s\n' "$*"; }
die() { printf '[argus-acceptance] error: %s\n' "$*" >&2; exit 1; }

usage() {
  cat <<'EOF'
Usage: sudo -E ./scripts/first-server-acceptance.sh <command>

Commands:
  install          Run the first clean install, smoke test, and record baseline.
  post-reboot      Require a real reboot, smoke test, and verify revision/identity.
  rerun-installer  Rerun install.sh and prove IDs/secrets/revision are preserved.
  update           Update to ARGUS_ACCEPTANCE_UPDATE_VERSION (default: main), smoke,
                   require a different immutable revision, and verify SUCCEEDED state.
  product          Exercise a new personal Project and supported product APIs on the
                   installed server, including Agent and Docker protection checks.
  report           Write/print the sanitized lifecycle acceptance report.
  status           Show which lifecycle checkpoints have completed.

Required environment for install/rerun/update is the same as the normal lifecycle,
including private-registry credentials where applicable. This runner does not store
registry credentials or plaintext Argus secrets in its report.
EOF
}

require_root() {
  [[ "${EUID}" -eq 0 ]] || die "run as root (sudo -E ...)"
}

is_revision() {
  [[ "$1" =~ ^[0-9a-f]{40}$ ]]
}

current_boot_id() {
  local boot_id
  boot_id="$(tr -d '[:space:]' </proc/sys/kernel/random/boot_id)"
  [[ "$boot_id" =~ ^[0-9a-f-]{36}$ ]] || die "could not read a valid Linux boot ID"
  printf '%s\n' "$boot_id"
}

read_installed_value() {
  local key="$1"
  [[ -f "$ENV_FILE" ]] || die "installed environment is missing: $ENV_FILE"
  awk -F= -v key="$key" '$1 == key { sub(/^[^=]*=/, ""); print; exit }' "$ENV_FILE"
}

installed_revision() {
  local revision
  revision="$(read_installed_value ARGUS_VERSION)"
  is_revision "$revision" \
    || die "installed ARGUS_VERSION is not an immutable full commit SHA: '${revision:-missing}'"
  printf '%s\n' "$revision"
}

configuration_fingerprint() {
  [[ -f "$ENV_FILE" ]] || die "installed environment is missing: $ENV_FILE"
  local material
  material="$(awk -F= '
    $1 == "ARGUS_POSTGRES_PASSWORD" ||
    $1 == "ARGUS_WEB_API_TOKEN" ||
    $1 == "ARGUS_WORKER_TOKEN" ||
    $1 == "ARGUS_CONTENT_SYNC_TOKEN" ||
    $1 == "PAYLOAD_SECRET" ||
    $1 == "ARGUS_ORG_ID" ||
    $1 == "ARGUS_USER_ID" ||
    $1 == "ARGUS_BOOTSTRAP_PROJECT_ID" ||
    $1 == "ARGUS_BOOTSTRAP_ENVIRONMENT_ID" ||
    $1 == "ARGUS_SERVER_ID" { print }
  ' "$ENV_FILE" | LC_ALL=C sort)"

  local count
  count="$(printf '%s\n' "$material" | sed '/^$/d' | wc -l | tr -d '[:space:]')"
  [[ "$count" == "10" ]] \
    || die "installed environment does not contain the complete generated identity/secret set"

  printf '%s\n' "$material" | sha256sum | awk '{ print $1 }'
}

ensure_acceptance_dir() {
  install -d -m 0700 "$ACCEPTANCE_DIR"
}

checkpoint_path() {
  printf '%s/%s.env\n' "$ACCEPTANCE_DIR" "$1"
}

write_checkpoint() {
  local name="$1"
  shift
  ensure_acceptance_dir
  local path tmp
  path="$(checkpoint_path "$name")"
  tmp="${path}.tmp.$$"
  : >"$tmp"
  chmod 0600 "$tmp"
  while (( $# >= 2 )); do
    printf '%s=%q\n' "$1" "$2" >>"$tmp"
    shift 2
  done
  sync -f "$tmp"
  mv "$tmp" "$path"
  sync -f "$ACCEPTANCE_DIR"
}

checkpoint_value() {
  local name="$1" key="$2" path
  path="$(checkpoint_path "$name")"
  [[ -f "$path" ]] || die "required acceptance checkpoint is missing: $name"
  (
    set +u
    # Checkpoint files are root-owned data emitted by this script.
    # shellcheck disable=SC1090
    . "$path"
    printf '%s\n' "${!key:-}"
  )
}

require_checkpoint() {
  [[ -f "$(checkpoint_path "$1")" ]] || die "run acceptance stage '$1' first"
}

run_smoke() {
  [[ -x /usr/local/bin/argusctl ]] || die "installed argusctl is missing"
  /usr/local/bin/argusctl smoke
}

run_installer() {
  [[ -x "$REPO_ROOT/install.sh" ]] || die "run from an authenticated Argus source checkout containing install.sh"
  "$REPO_ROOT/install.sh"
}

verify_configuration_fingerprint() {
  local expected="$1" actual
  actual="$(configuration_fingerprint)"
  [[ "$actual" == "$expected" ]] \
    || die "generated Argus IDs/secrets changed unexpectedly"
}

find_successful_update_transaction() {
  local from_revision="$1" to_revision="$2"
  local root="$STATE_DIR/update-backups"
  [[ -d "$root" ]] || return 1

  local entry dir result from to
  while IFS= read -r entry; do
    dir="${entry#* }"
    [[ -f "$dir/metadata.env" && -f "$dir/result" ]] || continue
    result="$(tr -d '[:space:]' <"$dir/result")"
    [[ "$result" == "SUCCEEDED" ]] || continue
    from="$(awk -F= '$1 == "FROM_REVISION" { print $2; exit }' "$dir/metadata.env")"
    to="$(awk -F= '$1 == "TO_REVISION" { print $2; exit }' "$dir/metadata.env")"
    if [[ "$from" == "$from_revision" && "$to" == "$to_revision" ]]; then
      printf '%s\n' "$dir"
      return 0
    fi
  done < <(find "$root" -mindepth 1 -maxdepth 1 -type d -printf '%T@ %p\n' | sort -nr)
  return 1
}

stage_install() {
  require_root
  if [[ -e "$(checkpoint_path baseline)" && "${ARGUS_ACCEPTANCE_RESTART:-0}" != "1" ]]; then
    die "baseline checkpoint already exists; use ARGUS_ACCEPTANCE_RESTART=1 only for a deliberate new acceptance run"
  fi
  if [[ "${ARGUS_ACCEPTANCE_RESTART:-0}" == "1" ]]; then
    rm -rf "$ACCEPTANCE_DIR"
  fi

  [[ ! -f "$ENV_FILE" ]] \
    || die "clean-install acceptance requires a host without an existing $ENV_FILE"

  run_installer
  run_smoke

  local revision boot_id fingerprint
  revision="$(installed_revision)"
  boot_id="$(current_boot_id)"
  fingerprint="$(configuration_fingerprint)"
  write_checkpoint baseline \
    COMPLETED_AT "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
    REVISION "$revision" \
    BOOT_ID "$boot_id" \
    CONFIG_FINGERPRINT "$fingerprint"

  log "baseline accepted at immutable revision $revision"
  log "next: reboot the host, then run '$0 post-reboot'"
}

stage_post_reboot() {
  require_root
  require_checkpoint baseline

  local baseline_boot baseline_revision baseline_fingerprint boot revision
  baseline_boot="$(checkpoint_value baseline BOOT_ID)"
  baseline_revision="$(checkpoint_value baseline REVISION)"
  baseline_fingerprint="$(checkpoint_value baseline CONFIG_FINGERPRINT)"
  boot="$(current_boot_id)"
  [[ "$boot" != "$baseline_boot" ]] \
    || die "Linux boot ID has not changed; perform a real reboot before this checkpoint"

  run_smoke
  revision="$(installed_revision)"
  [[ "$revision" == "$baseline_revision" ]] \
    || die "installed revision changed across reboot: $baseline_revision -> $revision"
  verify_configuration_fingerprint "$baseline_fingerprint"

  write_checkpoint post-reboot \
    COMPLETED_AT "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
    REVISION "$revision" \
    BOOT_ID "$boot" \
    CONFIG_PRESERVED yes
  log "post-reboot smoke and immutable revision verification passed"
}

stage_rerun_installer() {
  require_root
  require_checkpoint baseline
  require_checkpoint post-reboot

  local before_revision baseline_fingerprint after_revision
  before_revision="$(installed_revision)"
  baseline_fingerprint="$(checkpoint_value baseline CONFIG_FINGERPRINT)"

  run_installer
  run_smoke

  after_revision="$(installed_revision)"
  [[ "$after_revision" == "$before_revision" ]] \
    || die "installer rerun changed revision; updates must use argusctl update"
  verify_configuration_fingerprint "$baseline_fingerprint"

  write_checkpoint installer-rerun \
    COMPLETED_AT "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
    REVISION "$after_revision" \
    CONFIG_PRESERVED yes
  log "installer rerun preserved immutable revision, generated IDs, secrets, and smoke health"
}

stage_update() {
  require_root
  require_checkpoint installer-rerun

  local before after transaction
  before="$(installed_revision)"
  log "updating acceptance host from $before through discovery target '$UPDATE_VERSION'"
  ARGUS_REGISTRY_USERNAME="${ARGUS_REGISTRY_USERNAME:-}" \
  ARGUS_REGISTRY_TOKEN="${ARGUS_REGISTRY_TOKEN:-}" \
    /usr/local/bin/argusctl update --version "$UPDATE_VERSION"
  run_smoke
  after="$(installed_revision)"
  [[ "$after" != "$before" ]] \
    || die "acceptance update did not move to a different immutable revision; publish a newer green target first"

  transaction="$(find_successful_update_transaction "$before" "$after")" \
    || die "could not find a durable SUCCEEDED update transaction for $before -> $after"

  write_checkpoint post-update \
    COMPLETED_AT "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
    FROM_REVISION "$before" \
    TO_REVISION "$after" \
    TRANSACTION "$transaction" \
    SMOKE_PASSED yes
  log "transactional update accepted: $before -> $after"
}

stage_product() {
  require_root
  require_checkpoint post-reboot
  "$REPO_ROOT/scripts/first-server-product-acceptance.sh"
}

stage_status() {
  ensure_acceptance_dir
  local name
  for name in baseline post-reboot installer-rerun post-update product; do
    if [[ "$name" == product && -f "$ACCEPTANCE_DIR/product.env" ]] ||
       [[ "$name" != product && -f "$(checkpoint_path "$name")" ]]; then
      printf '%-18s %s\n' "$name" complete
    else
      printf '%-18s %s\n' "$name" pending
    fi
  done
}

stage_report() {
  require_root
  local name
  for name in baseline post-reboot installer-rerun post-update; do
    require_checkpoint "$name"
  done
  [[ -f "$ACCEPTANCE_DIR/product.env" ]] || die "run acceptance stage 'product' first"

  local initial_revision reboot_revision rerun_revision from_revision to_revision transaction product_project_id
  local monitor_job_id backup_name backup_command_id verify_command_id preflight_command_id
  initial_revision="$(checkpoint_value baseline REVISION)"
  reboot_revision="$(checkpoint_value post-reboot REVISION)"
  rerun_revision="$(checkpoint_value installer-rerun REVISION)"
  from_revision="$(checkpoint_value post-update FROM_REVISION)"
  to_revision="$(checkpoint_value post-update TO_REVISION)"
  transaction="$(checkpoint_value post-update TRANSACTION)"
  product_project_id="$(
    set +u
    # Root-owned checkpoint emitted by first-server-product-acceptance.sh.
    # shellcheck disable=SC1090
    . "$ACCEPTANCE_DIR/product.env"
    printf '%s\n' "${PROJECT_ID:-}"
  )"
  monitor_job_id="$(checkpoint_value product MONITOR_JOB_ID)"
  backup_name="$(checkpoint_value product BACKUP_NAME)"
  backup_command_id="$(checkpoint_value product BACKUP_COMMAND_ID)"
  verify_command_id="$(checkpoint_value product VERIFY_COMMAND_ID)"
  preflight_command_id="$(checkpoint_value product PREFLIGHT_COMMAND_ID)"
  [[ "$product_project_id" =~ ^[0-9a-f-]{36}$ ]] || die "product acceptance Project ID is invalid"
  [[ "$monitor_job_id" =~ ^[0-9a-f-]{36}$ ]] || die "product acceptance monitor job ID is invalid"
  [[ "$backup_command_id" =~ ^[0-9a-f-]{36}$ && "$backup_name" == "$backup_command_id.tar.gz" ]] || die "product acceptance backup evidence is inconsistent"
  [[ "$verify_command_id" =~ ^[0-9a-f-]{36}$ && "$preflight_command_id" =~ ^[0-9a-f-]{36}$ ]] || die "product acceptance backup command evidence is invalid"

  [[ "$initial_revision" == "$reboot_revision" && "$initial_revision" == "$rerun_revision" ]] \
    || die "checkpoint revisions are inconsistent"
  [[ "$from_revision" == "$initial_revision" ]] \
    || die "update did not start from the accepted initial revision"
  is_revision "$to_revision" || die "post-update revision is invalid"

  ensure_acceptance_dir
  local tmp="${REPORT_FILE}.tmp.$$"
  cat >"$tmp" <<EOF
Argus first-server lifecycle acceptance
======================================
result: PASS
reported_at: $(date -u +%Y-%m-%dT%H:%M:%SZ)
initial_revision: $initial_revision
post_reboot_revision: $reboot_revision
installer_rerun_revision: $rerun_revision
post_update_revision: $to_revision
real_reboot_verified: yes
configuration_identity_preserved: yes
post_install_smoke: yes
post_reboot_smoke: yes
post_rerun_smoke: yes
post_update_smoke: yes
successful_update_transaction: $transaction
personal_project_without_client: $product_project_id
product_api_structures_verified: yes
typed_agent_action_verified: yes
control_plane_docker_protection_verified: yes
payload_project_sync_verified: yes
post_reboot_scheduler_execution_verified: yes
scheduled_site_monitor_job: $monitor_job_id
verified_system_config_backup: $backup_name
backup_create_command: $backup_command_id
backup_verify_command: $verify_command_id
restore_preflight_command: $preflight_command_id

This report intentionally contains no registry credential or plaintext Argus secret.
It proves the recorded lifecycle and product checkpoints only. Transactional restore apply,
CMS/App Data workflows, reset/reinstall and manual-failure acceptance remain separate work.
EOF
  chmod 0600 "$tmp"
  sync -f "$tmp"
  mv "$tmp" "$REPORT_FILE"
  sync -f "$ACCEPTANCE_DIR"
  cat "$REPORT_FILE"
}

main() {
  local command="${1:-}"
  case "$command" in
    install) stage_install ;;
    post-reboot) stage_post_reboot ;;
    rerun-installer) stage_rerun_installer ;;
    update) stage_update ;;
    product) stage_product ;;
    report) stage_report ;;
    status) stage_status ;;
    -h|--help|help|'') usage ;;
    *) usage >&2; die "unknown acceptance command: $command" ;;
  esac
}

if [[ "${1:-}" != "--internal-test-library" ]]; then
  main "$@"
fi
