#!/usr/bin/env bash
set -Eeuo pipefail
umask 077

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "$SCRIPT_DIR/.." && pwd)"
INSTALL_DIR="${ARGUS_INSTALL_DIR:-/opt/argus}"
STATE_DIR="${ARGUS_STATE_DIR:-/var/lib/argus}"
ENV_FILE="$INSTALL_DIR/.env"
ACCEPTANCE_DIR="${ARGUS_ACCEPTANCE_DIR:-$STATE_DIR/acceptance/first-server}"
ACCEPTANCE_ARCHIVE_DIR="${ARGUS_ACCEPTANCE_ARCHIVE_DIR:-/var/lib/argus-acceptance/first-server}"
REPORT_FILE="$ACCEPTANCE_DIR/report.txt"
UPDATE_VERSION="${ARGUS_ACCEPTANCE_UPDATE_VERSION:-main}"
FAILURE_UPDATE_VERSION="${ARGUS_ACCEPTANCE_FAILURE_UPDATE_VERSION:-$UPDATE_VERSION}"

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
  update-rollback  Deliberately fail after a newer target is durably armed, require
                   automatic rollback to the current revision, then pass smoke.
  product          Exercise a new personal Project and supported product APIs on the
                   installed server, including Agent and Docker protection checks.
  content          Exercise CMS model, draft, publish and public-read workflows for
                   the personal Project created by the product stage.
  restore          On a disposable host, require explicit confirmation and exercise
                   maintenance-gated transactional restore plus post-restore smoke.
  reset-reinstall  Terminal disposable-host stage: archive the PASS report outside
                   Argus state, reset it, perform a second clean install, and smoke.
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

find_rolled_back_update_transaction() {
  local from_revision="$1" not_before="${2:-}" root="$STATE_DIR/update-backups"
  [[ -d "$root" ]] || return 1
  local entry dir result from started_at
  while IFS= read -r entry; do
    dir="${entry#* }"
    [[ -f "$dir/metadata.env" && -f "$dir/result" ]] || continue
    result="$(tr -d '[:space:]' <"$dir/result")"
    [[ "$result" == "ROLLED_BACK" ]] || continue
    from="$(awk -F= '$1 == "FROM_REVISION" { print $2; exit }' "$dir/metadata.env")"
    started_at="$(awk -F= '$1 == "STARTED_AT" { print $2; exit }' "$dir/metadata.env")"
    if [[ -n "$not_before" && ( -z "$started_at" || "$started_at" < "$not_before" ) ]]; then
      continue
    fi
    if [[ "$from" == "$from_revision" ]]; then
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
  require_checkpoint rollback-test

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

stage_update_rollback() {
  require_root
  require_checkpoint installer-rerun
  local before after transaction target started_at
  before="$(installed_revision)"
  started_at="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  log "deliberately failing the update target '$FAILURE_UPDATE_VERSION' after target-start arming"
  if ARGUS_UPDATE_ACCEPTANCE_FAILURE=after-target-start-armed \
    ARGUS_UPDATE_ACCEPTANCE_CONFIRM_FAILURE=ROLLBACK-TEST-ONLY \
    ARGUS_REGISTRY_USERNAME="${ARGUS_REGISTRY_USERNAME:-}" \
    ARGUS_REGISTRY_TOKEN="${ARGUS_REGISTRY_TOKEN:-}" \
      /usr/local/bin/argusctl update --version "$FAILURE_UPDATE_VERSION"; then
    die "failure-injected update unexpectedly succeeded; ensure the discovery target is a newer immutable revision"
  fi
  after="$(installed_revision)"
  [[ "$after" == "$before" ]] || die "automatic rollback did not restore revision $before"
  transaction="$(find_rolled_back_update_transaction "$before" "$started_at")" \
    || die "could not find a durable ROLLED_BACK transaction from $before"
  target="$(awk -F= '$1 == "TO_REVISION" { print $2; exit }' "$transaction/metadata.env")"
  is_revision "$target" && [[ "$target" != "$before" ]] || die "rollback transaction target is invalid"
  run_smoke
  write_checkpoint rollback-test COMPLETED_AT "$(date -u +%Y-%m-%dT%H:%M:%SZ)" FROM_REVISION "$before" \
    FAILED_TARGET_REVISION "$target" TRANSACTION "$transaction" RESULT ROLLED_BACK SMOKE_PASSED yes
  log "confirmed automatic rollback from failed target $target to $before"
}

stage_product() {
  require_root
  require_checkpoint post-reboot
  "$REPO_ROOT/scripts/first-server-product-acceptance.sh"
}

stage_content() {
  require_root
  [[ -f "$ACCEPTANCE_DIR/product.env" ]] || die "run acceptance stage 'product' first"
  local project_id organization_id user_id result model_id record_id model_slug data_model_id data_record_id data_relation_target_id
  project_id="$(checkpoint_value product PROJECT_ID)"
  organization_id="$(read_installed_value ARGUS_ORG_ID)"
  user_id="$(read_installed_value ARGUS_USER_ID)"
  log "exercising draft and publication through the installed Content API"
  result="$(docker compose --project-directory "$INSTALL_DIR" --env-file "$ENV_FILE" -f "$INSTALL_DIR/compose.yaml" exec -T \
    -e ARGUS_TEST_PROJECT_ID="$project_id" -e ARGUS_TEST_ORG_ID="$organization_id" -e ARGUS_TEST_USER_ID="$user_id" \
    content node --input-type=module - <"$REPO_ROOT/scripts/first-server-content-acceptance.mjs")"
  model_id="$(jq -er .model_id <<<"$result")"
  record_id="$(jq -er .record_id <<<"$result")"
  model_slug="$(jq -er .model_slug <<<"$result")"
  data_model_id="$(jq -er .data_model_id <<<"$result")"
  data_record_id="$(jq -er .data_record_id <<<"$result")"
  data_relation_target_id="$(jq -er .data_relation_target_id <<<"$result")"
  [[ "$model_id" =~ ^[0-9a-f-]{36}$ && "$record_id" =~ ^[0-9a-f-]{36}$ && "$model_slug" =~ ^acceptance_[a-z0-9_]+$ ]] \
    || die "Content acceptance returned invalid evidence"
  [[ "$data_model_id" =~ ^[0-9a-f-]{36}$ && "$data_record_id" =~ ^[0-9a-f-]{36}$ && "$data_relation_target_id" =~ ^[0-9a-f-]{36}$ ]] \
    || die "App Data acceptance returned invalid evidence"
  write_checkpoint content COMPLETED_AT "$(date -u +%Y-%m-%dT%H:%M:%SZ)" PROJECT_ID "$project_id" \
    MODEL_ID "$model_id" RECORD_ID "$record_id" MODEL_SLUG "$model_slug" DATA_MODEL_ID "$data_model_id" \
    DATA_RECORD_ID "$data_record_id" DATA_RELATION_TARGET_ID "$data_relation_target_id"
  log "Content acceptance passed for model $model_id and record $record_id"
}

stage_restore() {
  require_root
  [[ -f "$ACCEPTANCE_DIR/product.env" ]] || die "run acceptance stage 'product' first"
  require_checkpoint content
  "$REPO_ROOT/scripts/first-server-restore-acceptance.sh"
}

stage_reset_reinstall() {
  require_root
  [[ -f "$REPORT_FILE" ]] || die "run acceptance stage 'report' first"
  ARGUS_ACCEPTANCE_ARCHIVE_DIR="$ACCEPTANCE_ARCHIVE_DIR" \
    "$REPO_ROOT/scripts/first-server-reset-reinstall-acceptance.sh"
}

stage_status() {
  local terminal="$ACCEPTANCE_ARCHIVE_DIR/reset-reinstall.env" phase=""
  if [[ -f "$terminal" ]]; then
    phase="$(
      set +u
      # Root-owned terminal checkpoint written with shell escaping.
      # shellcheck disable=SC1090
      . "$terminal"
      printf '%s\n' "${PHASE:-}"
    )"
  fi
  if [[ "$phase" == COMPLETE ]]; then
    local archived_name
    for archived_name in baseline post-reboot installer-rerun rollback-test post-update product content restore; do
      printf '%-18s %s\n' "$archived_name" 'complete (archived)'
    done
    printf '%-18s %s\n' reset-reinstall complete
    printf 'final report: %s/final-report.txt\n' "$ACCEPTANCE_ARCHIVE_DIR"
    return 0
  fi
  ensure_acceptance_dir
  local name
  for name in baseline post-reboot installer-rerun rollback-test post-update product content restore; do
    if [[ "$name" == product && -f "$ACCEPTANCE_DIR/product.env" ]] ||
       [[ "$name" != product && -f "$(checkpoint_path "$name")" ]]; then
      printf '%-18s %s\n' "$name" complete
    else
      printf '%-18s %s\n' "$name" pending
    fi
  done
  printf '%-18s %s\n' reset-reinstall "${phase:-pending}"
}

stage_report() {
  require_root
  local name
  for name in baseline post-reboot installer-rerun rollback-test post-update; do
    require_checkpoint "$name"
  done
  [[ -f "$ACCEPTANCE_DIR/product.env" ]] || die "run acceptance stage 'product' first"
  require_checkpoint content
  require_checkpoint restore

  local initial_revision reboot_revision rerun_revision from_revision to_revision transaction product_project_id
  local monitor_job_id backup_name backup_command_id verify_command_id preflight_command_id
  local content_project_id content_model_id content_record_id content_model_slug data_model_id data_record_id data_relation_target_id
  local rollback_from rollback_target rollback_transaction rollback_result
  local restore_backup restore_command_id restore_maintenance_id restore_gate restore_disarmed restore_smoke
  initial_revision="$(checkpoint_value baseline REVISION)"
  reboot_revision="$(checkpoint_value post-reboot REVISION)"
  rerun_revision="$(checkpoint_value installer-rerun REVISION)"
  from_revision="$(checkpoint_value post-update FROM_REVISION)"
  to_revision="$(checkpoint_value post-update TO_REVISION)"
  transaction="$(checkpoint_value post-update TRANSACTION)"
  rollback_from="$(checkpoint_value rollback-test FROM_REVISION)"
  rollback_target="$(checkpoint_value rollback-test FAILED_TARGET_REVISION)"
  rollback_transaction="$(checkpoint_value rollback-test TRANSACTION)"
  rollback_result="$(checkpoint_value rollback-test RESULT)"
  restore_backup="$(checkpoint_value restore BACKUP_NAME)"
  restore_command_id="$(checkpoint_value restore RESTORE_COMMAND_ID)"
  restore_maintenance_id="$(checkpoint_value restore MAINTENANCE_WINDOW_ID)"
  restore_gate="$(checkpoint_value restore MAINTENANCE_GATE_VERIFIED)"
  restore_disarmed="$(checkpoint_value restore TIMED_ROLLBACK_DISARMED)"
  restore_smoke="$(checkpoint_value restore POST_RESTORE_SMOKE)"
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
  content_project_id="$(checkpoint_value content PROJECT_ID)"
  content_model_id="$(checkpoint_value content MODEL_ID)"
  content_record_id="$(checkpoint_value content RECORD_ID)"
  content_model_slug="$(checkpoint_value content MODEL_SLUG)"
  data_model_id="$(checkpoint_value content DATA_MODEL_ID)"
  data_record_id="$(checkpoint_value content DATA_RECORD_ID)"
  data_relation_target_id="$(checkpoint_value content DATA_RELATION_TARGET_ID)"
  [[ "$product_project_id" =~ ^[0-9a-f-]{36}$ ]] || die "product acceptance Project ID is invalid"
  [[ "$monitor_job_id" =~ ^[0-9a-f-]{36}$ ]] || die "product acceptance monitor job ID is invalid"
  [[ "$backup_command_id" =~ ^[0-9a-f-]{36}$ && "$backup_name" == "$backup_command_id.tar.gz" ]] || die "product acceptance backup evidence is inconsistent"
  [[ "$verify_command_id" =~ ^[0-9a-f-]{36}$ && "$preflight_command_id" =~ ^[0-9a-f-]{36}$ ]] || die "product acceptance backup command evidence is invalid"
  [[ "$content_project_id" == "$product_project_id" ]] || die "Content acceptance used a different Project"
  [[ "$content_model_id" =~ ^[0-9a-f-]{36}$ && "$content_record_id" =~ ^[0-9a-f-]{36}$ && "$content_model_slug" =~ ^acceptance_[a-z0-9_]+$ ]] \
    || die "Content acceptance evidence is invalid"
  [[ "$data_model_id" =~ ^[0-9a-f-]{36}$ && "$data_record_id" =~ ^[0-9a-f-]{36}$ && "$data_relation_target_id" =~ ^[0-9a-f-]{36}$ ]] \
    || die "App Data acceptance evidence is invalid"
  [[ "$restore_backup" == "$backup_name" && "$restore_command_id" =~ ^[0-9a-f-]{36}$ && "$restore_maintenance_id" =~ ^[0-9a-f-]{36}$ ]] \
    || die "transactional restore evidence is inconsistent"
  [[ "$restore_gate" == yes && "$restore_disarmed" == yes && "$restore_smoke" == yes ]] \
    || die "transactional restore safety evidence is incomplete"

  [[ "$initial_revision" == "$reboot_revision" && "$initial_revision" == "$rerun_revision" ]] \
    || die "checkpoint revisions are inconsistent"
  [[ "$from_revision" == "$initial_revision" ]] \
    || die "update did not start from the accepted initial revision"
  [[ "$rollback_from" == "$initial_revision" && "$rollback_target" == "$to_revision" && "$rollback_result" == "ROLLED_BACK" ]] \
    || die "rollback acceptance and successful update revisions are inconsistent"
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
automatic_rollback_transaction: $rollback_transaction
safe_target_start_failure_rolled_back: yes
post_rollback_smoke: yes
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
transactional_restore_command: $restore_command_id
restore_maintenance_window: $restore_maintenance_id
restore_maintenance_gate_verified: yes
restore_timed_rollback_disarmed: yes
post_restore_smoke: yes
cms_model: $content_model_id
cms_published_record: $content_record_id
cms_public_model_slug: $content_model_slug
cms_draft_publication_public_read_verified: yes
app_data_model: $data_model_id
app_data_record: $data_record_id
app_data_relation_target: $data_relation_target_id
app_data_immediate_write_and_relation_verified: yes

This report intentionally contains no registry credential or plaintext Argus secret.
It proves the recorded lifecycle, product, App Data, CMS and restore checkpoints only.
Reset/reinstall remains separate acceptance work.
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
    update-rollback) stage_update_rollback ;;
    product) stage_product ;;
    content) stage_content ;;
    restore) stage_restore ;;
    reset-reinstall) stage_reset_reinstall ;;
    report) stage_report ;;
    status) stage_status ;;
    -h|--help|help|'') usage ;;
    *) usage >&2; die "unknown acceptance command: $command" ;;
  esac
}

if [[ "${1:-}" != "--internal-test-library" ]]; then
  main "$@"
fi
