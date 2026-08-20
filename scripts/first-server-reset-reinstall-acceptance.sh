#!/usr/bin/env bash
set -Eeuo pipefail
umask 077

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "$SCRIPT_DIR/.." && pwd)"
INSTALL_DIR="${ARGUS_INSTALL_DIR:-/opt/argus}"
CONFIG_DIR="${ARGUS_CONFIG_DIR:-/etc/argus}"
STATE_DIR="${ARGUS_STATE_DIR:-/var/lib/argus}"
ENV_FILE="$INSTALL_DIR/.env"
ACCEPTANCE_DIR="${ARGUS_ACCEPTANCE_DIR:-$STATE_DIR/acceptance/first-server}"
SOURCE_REPORT="$ACCEPTANCE_DIR/report.txt"
ARCHIVE_DIR="${ARGUS_ACCEPTANCE_ARCHIVE_DIR:-/var/lib/argus-acceptance/first-server}"
ARCHIVED_REPORT="$ARCHIVE_DIR/lifecycle-report.txt"
CHECKPOINT_FILE="$ARCHIVE_DIR/reset-reinstall.env"
FINAL_REPORT="$ARCHIVE_DIR/final-report.txt"

log() { printf '[argus-reset-acceptance] %s\n' "$*"; }
die() { printf '[argus-reset-acceptance] error: %s\n' "$*" >&2; exit 1; }

require_root() { [[ "${EUID}" -eq 0 ]] || die "run as root (sudo -E ...)"; }
is_revision() { [[ "$1" =~ ^[0-9a-f]{40}$ ]]; }

path_is_inside() {
  local child="${1%/}/" parent="${2%/}/"
  [[ "$child" == "$parent"* ]]
}

validate_archive_location() {
  local name path normalized
  for name in ARCHIVE_DIR INSTALL_DIR CONFIG_DIR STATE_DIR; do
    path="${!name}"
    [[ "$path" == /* ]] || die "$name must be an absolute path"
    normalized="$(realpath -m -- "$path")"
    [[ "$path" == "$normalized" ]] || die "$name must not contain aliases, traversal, or a trailing slash"
  done
  [[ "$ARCHIVE_DIR" != "/" && "$ARCHIVE_DIR" != "/etc" && "$ARCHIVE_DIR" != "/opt" && \
     "$ARCHIVE_DIR" != "/usr" && "$ARCHIVE_DIR" != "/var" && "$ARCHIVE_DIR" != "/root" && \
     "$ARCHIVE_DIR" != "/home" ]] || die "archive directory is too broad"
  path_is_inside "$ARCHIVE_DIR" "$INSTALL_DIR" && die "archive directory must be outside ARGUS_INSTALL_DIR"
  path_is_inside "$ARCHIVE_DIR" "$CONFIG_DIR" && die "archive directory must be outside ARGUS_CONFIG_DIR"
  path_is_inside "$ARCHIVE_DIR" "$STATE_DIR" && die "archive directory must be outside ARGUS_STATE_DIR"
  return 0
}

environment_fingerprint() {
  local env_file="$1" material count
  [[ -f "$env_file" ]] || die "installed environment is missing: $env_file"
  material="$(awk -F= '
    $1 == "ARGUS_POSTGRES_PASSWORD" || $1 == "ARGUS_WEB_API_TOKEN" ||
    $1 == "ARGUS_WORKER_TOKEN" || $1 == "ARGUS_CONTENT_SYNC_TOKEN" ||
    $1 == "PAYLOAD_SECRET" || $1 == "ARGUS_ORG_ID" || $1 == "ARGUS_USER_ID" ||
    $1 == "ARGUS_BOOTSTRAP_PROJECT_ID" || $1 == "ARGUS_BOOTSTRAP_ENVIRONMENT_ID" ||
    $1 == "ARGUS_SERVER_ID" { print }
  ' "$env_file" | LC_ALL=C sort)"
  count="$(printf '%s\n' "$material" | sed '/^$/d' | wc -l | tr -d '[:space:]')"
  [[ "$count" == 10 ]] || die "installed environment does not contain the complete generated identity/secret set"
  printf '%s\n' "$material" | sha256sum | awk '{ print $1 }'
}

installed_revision() {
  local revision
  revision="$(awk -F= '$1 == "ARGUS_VERSION" { sub(/^[^=]*=/, ""); print; exit }' "$ENV_FILE")"
  is_revision "$revision" || die "installed revision is not an immutable commit SHA"
  printf '%s\n' "$revision"
}

write_checkpoint() {
  local phase="$1" old_fingerprint="$2" old_revision="$3" report_sha="$4"
  local new_fingerprint="${5:-}" new_revision="${6:-}" final_report_sha="${7:-}"
  install -d -m 0700 "$ARCHIVE_DIR"
  local tmp="$CHECKPOINT_FILE.tmp.$$"
  {
    printf 'PHASE=%q\n' "$phase"
    printf 'UPDATED_AT=%q\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
    printf 'ORIGINAL_CONFIGURATION_FINGERPRINT=%q\n' "$old_fingerprint"
    printf 'ORIGINAL_REVISION=%q\n' "$old_revision"
    printf 'LIFECYCLE_REPORT_SHA256=%q\n' "$report_sha"
    printf 'SECOND_CONFIGURATION_FINGERPRINT=%q\n' "$new_fingerprint"
    printf 'SECOND_REVISION=%q\n' "$new_revision"
    printf 'FINAL_REPORT_SHA256=%q\n' "$final_report_sha"
  } >"$tmp"
  chmod 0600 "$tmp"
  sync -f "$tmp"
  mv "$tmp" "$CHECKPOINT_FILE"
  sync -f "$ARCHIVE_DIR"
}

checkpoint_value() {
  local key="$1"
  [[ -f "$CHECKPOINT_FILE" ]] || die "terminal acceptance checkpoint is missing"
  (
    set +u
    # Root-owned checkpoint written by this script with shell escaping.
    # shellcheck disable=SC1090
    . "$CHECKPOINT_FILE"
    printf '%s\n' "${!key:-}"
  )
}

print_completed_report() {
  local final_report_sha
  final_report_sha="$(checkpoint_value FINAL_REPORT_SHA256)"
  [[ "$final_report_sha" =~ ^[0-9a-f]{64}$ && -f "$FINAL_REPORT" && \
     "$(sha256sum "$FINAL_REPORT" | awk '{ print $1 }')" == "$final_report_sha" ]] \
    || die "terminal acceptance final report is missing or its checksum changed"
  cat "$FINAL_REPORT"
}

assert_reset_absent() {
  [[ ! -e "$INSTALL_DIR" ]] || die "reset left installation directory behind"
  [[ ! -e "$CONFIG_DIR" ]] || die "reset left configuration directory behind"
  [[ ! -e "$STATE_DIR" ]] || die "reset left state directory behind"
  [[ ! -e /usr/local/bin/argus-agent && ! -e /usr/local/bin/argus-helper && ! -e /usr/local/bin/argusctl ]] \
    || die "reset left native Argus binaries behind"
  [[ ! -e /etc/systemd/system/argus-agent.service && ! -e /etc/systemd/system/argus-helper.service ]] \
    || die "reset left native systemd units behind"
  ! id argus >/dev/null 2>&1 || die "reset left the argus user behind"
  ! getent group argus >/dev/null 2>&1 || die "reset left the argus group behind"
  [[ -z "$(docker ps -aq --filter label=com.argus.protected=true)" ]] \
    || die "reset left protected Argus control-plane containers behind"
}

run_clean_installer() {
  env -u ARGUS_POSTGRES_PASSWORD -u ARGUS_WEB_API_TOKEN -u ARGUS_WORKER_TOKEN \
    -u ARGUS_CONTENT_SYNC_TOKEN -u PAYLOAD_SECRET -u ARGUS_ORG_ID -u ARGUS_USER_ID \
    -u ARGUS_BOOTSTRAP_PROJECT_ID -u ARGUS_BOOTSTRAP_ENVIRONMENT_ID -u ARGUS_SERVER_ID \
    "$REPO_ROOT/install.sh"
}

write_final_report() {
  local old_fingerprint="$1" old_revision="$2" report_sha="$3" new_fingerprint="$4" new_revision="$5" tmp
  tmp="$FINAL_REPORT.tmp.$$"
  {
    cat "$ARCHIVED_REPORT"
    cat <<EOF

Terminal reset and second-install acceptance
============================================
completed_at: $(date -u +%Y-%m-%dT%H:%M:%SZ)
reset_installation_absence_verified: yes
reset_native_services_absence_verified: yes
reset_control_plane_containers_absence_verified: yes
second_clean_install_smoke: yes
second_clean_install_new_identity: yes
original_configuration_fingerprint: $old_fingerprint
second_configuration_fingerprint: $new_fingerprint
lifecycle_report_sha256: $report_sha
original_revision: $old_revision
second_install_revision: $new_revision

The terminal evidence is stored outside the deleted Argus state directory. It contains
only fingerprints, immutable revisions and sanitized lifecycle evidence; no plaintext
Argus secret or registry credential is included.
EOF
  } >"$tmp"
  chmod 0600 "$tmp"
  sync -f "$tmp"
  mv "$tmp" "$FINAL_REPORT"
  sync -f "$ARCHIVE_DIR"
}

main() {
  require_root
  [[ "${ARGUS_CONFIRM_RESET_REINSTALL:-}" == "RESET-AND-REINSTALL-DISPOSABLE-HOST" ]] \
    || die "set ARGUS_CONFIRM_RESET_REINSTALL=RESET-AND-REINSTALL-DISPOSABLE-HOST on a disposable host"
  validate_archive_location
  [[ -x "$REPO_ROOT/install.sh" && -x "$REPO_ROOT/scripts/reset-first-test.sh" ]] \
    || die "run from an authenticated Argus source checkout"

  local phase old_fingerprint old_revision report_sha new_fingerprint new_revision
  if [[ -f "$CHECKPOINT_FILE" && ! -f "$SOURCE_REPORT" ]]; then
    phase="$(checkpoint_value PHASE)"
    old_fingerprint="$(checkpoint_value ORIGINAL_CONFIGURATION_FINGERPRINT)"
    old_revision="$(checkpoint_value ORIGINAL_REVISION)"
    report_sha="$(checkpoint_value LIFECYCLE_REPORT_SHA256)"
    [[ "$old_fingerprint" =~ ^[0-9a-f]{64}$ && "$report_sha" =~ ^[0-9a-f]{64}$ ]] \
      || die "terminal acceptance checkpoint is invalid"
    is_revision "$old_revision" || die "terminal acceptance original revision is invalid"
    [[ -f "$ARCHIVED_REPORT" && "$(sha256sum "$ARCHIVED_REPORT" | awk '{ print $1 }')" == "$report_sha" ]] \
      || die "archived lifecycle report is missing or its checksum changed"
    if [[ "$phase" == COMPLETE ]]; then
      print_completed_report
      return 0
    fi
    [[ "$phase" == PREPARED || "$phase" == RESET_VERIFIED ]] \
      || die "unsupported terminal acceptance phase: $phase"
    log "resuming terminal acceptance from durable phase $phase"
  else
    [[ -f "$SOURCE_REPORT" ]] || die "run the complete acceptance report stage before terminal reset"
    grep -Fqx 'result: PASS' "$SOURCE_REPORT" || die "lifecycle acceptance report is not PASS"
    old_fingerprint="$(environment_fingerprint "$ENV_FILE")"
    old_revision="$(installed_revision)"
    install -d -m 0700 "$ARCHIVE_DIR"
    install -m 0600 "$SOURCE_REPORT" "$ARCHIVED_REPORT"
    report_sha="$(sha256sum "$ARCHIVED_REPORT" | awk '{ print $1 }')"
    write_checkpoint PREPARED "$old_fingerprint" "$old_revision" "$report_sha"
    phase=PREPARED
  fi

  if [[ "$phase" == PREPARED ]]; then
    if [[ -e "$INSTALL_DIR" || -e "$CONFIG_DIR" || -e "$STATE_DIR" ]]; then
      log "resetting the disposable installation after archiving sanitized lifecycle evidence"
      ARGUS_CONFIRM_RESET=DELETE-ARGUS-FIRST-TEST-DATA "$REPO_ROOT/scripts/reset-first-test.sh"
    fi
    assert_reset_absent
    write_checkpoint RESET_VERIFIED "$old_fingerprint" "$old_revision" "$report_sha"
  fi

  log "performing the second clean install with newly generated identities and secrets"
  run_clean_installer
  /usr/local/bin/argusctl smoke
  new_fingerprint="$(environment_fingerprint "$ENV_FILE")"
  new_revision="$(installed_revision)"
  [[ "$new_fingerprint" != "$old_fingerprint" ]] \
    || die "second clean install reused the original generated identity/secret set"

  write_final_report "$old_fingerprint" "$old_revision" "$report_sha" "$new_fingerprint" "$new_revision"
  local final_report_sha
  final_report_sha="$(sha256sum "$FINAL_REPORT" | awk '{ print $1 }')"
  write_checkpoint COMPLETE "$old_fingerprint" "$old_revision" "$report_sha" "$new_fingerprint" "$new_revision" "$final_report_sha"
  cat "$FINAL_REPORT"
}

if [[ "${1:-}" != "--internal-test-library" ]]; then
  main "$@"
fi
