#!/usr/bin/env bash
set -Eeuo pipefail
umask 077

INSTALL_DIR="${ARGUS_INSTALL_DIR:-/opt/argus}"
CONFIG_DIR="${ARGUS_CONFIG_DIR:-/etc/argus}"
STATE_DIR="${ARGUS_STATE_DIR:-/var/lib/argus}"
LOG_DIR="${ARGUS_LOG_DIR:-/var/log/argus}"
ENV_FILE="$INSTALL_DIR/.env"
COMPOSE_FILE="$INSTALL_DIR/compose.yaml"
PURGE_DATA="${ARGUS_UNINSTALL_PURGE_DATA:-0}"

log() { printf '[argus-uninstall] %s\n' "$*"; }
die() { printf '[argus-uninstall] error: %s\n' "$*" >&2; exit 1; }

confirm_uninstall() {
  [[ "${EUID}" -eq 0 ]] || die "run as root (sudo argusctl uninstall)"
  if [[ "${ARGUS_UNINSTALL_CONFIRM:-0}" == "1" ]]; then return; fi
  [[ -t 0 ]] || die "confirmation required; rerun with --yes"
  printf 'This will stop Argus and remove its binaries and configuration.\n'
  if [[ "$PURGE_DATA" == "1" ]]; then printf 'Docker volumes, state, backups, and logs will also be permanently deleted.\n'; fi
  local answer
  read -r -p 'Type UNINSTALL ARGUS to continue: ' answer
  [[ "$answer" == "UNINSTALL ARGUS" ]] || die "uninstall cancelled"
}

compose_down() {
  [[ -f "$ENV_FILE" && -f "$COMPOSE_FILE" ]] || return 0
  local args=(down --remove-orphans)
  [[ "$PURGE_DATA" == "1" ]] && args+=(--volumes)
  docker compose --project-directory "$INSTALL_DIR" --env-file "$ENV_FILE" -f "$COMPOSE_FILE" "${args[@]}" || \
    die "could not stop the control-plane stack; no files were removed"
}

main() {
  confirm_uninstall
  log "stopping Argus services"
  systemctl disable --now argus-agent.service argus-helper.service >/dev/null 2>&1 || true
  compose_down

  log "removing Argus binaries and configuration"
  rm -f -- /usr/local/bin/argus-agent /usr/local/bin/argus-helper /usr/local/bin/argusctl
  rm -f -- /etc/systemd/system/argus-agent.service /etc/systemd/system/argus-helper.service
  systemctl daemon-reload
  rm -rf -- "$INSTALL_DIR" "$CONFIG_DIR"

  if [[ "$PURGE_DATA" == "1" ]]; then
    log "purging Argus state, backups, and logs"
    rm -rf -- "$STATE_DIR" "$LOG_DIR"
    userdel argus >/dev/null 2>&1 || true
    groupdel argus >/dev/null 2>&1 || true
    printf 'Argus and its data were removed. This cannot be recovered without an external backup.\n'
  else
    printf 'Argus was removed. State and Docker volumes were preserved for recovery.\n'
    printf 'Preserved state: %s\n' "$STATE_DIR"
  fi
}

if [[ "${1:-}" != "--internal-test-library" ]]; then main "$@"; fi
