#!/usr/bin/env bash
set -Eeuo pipefail

INSTALL_DIR="${ARGUS_INSTALL_DIR:-/opt/argus}"
CONFIG_DIR="${ARGUS_CONFIG_DIR:-/etc/argus}"
STATE_DIR="${ARGUS_STATE_DIR:-/var/lib/argus}"

[[ "${EUID}" -eq 0 ]] || { echo "run as root" >&2; exit 1; }
[[ "${ARGUS_CONFIRM_RESET:-}" == "DELETE-ARGUS-FIRST-TEST-DATA" ]] || {
  cat >&2 <<'EOF'
This command removes the disposable first-test Argus installation, including its
PostgreSQL volume, Agent identity and backups.

Rerun with:
  ARGUS_CONFIRM_RESET=DELETE-ARGUS-FIRST-TEST-DATA sudo -E ./scripts/reset-first-test.sh
EOF
  exit 2
}

if [[ -f "$INSTALL_DIR/compose.yaml" && -f "$INSTALL_DIR/.env" ]]; then
  docker compose \
    --project-directory "$INSTALL_DIR" \
    --env-file "$INSTALL_DIR/.env" \
    -f "$INSTALL_DIR/compose.yaml" \
    down --volumes --remove-orphans || true
fi

systemctl disable --now argus-agent.service argus-helper.service >/dev/null 2>&1 || true
rm -f /etc/systemd/system/argus-agent.service /etc/systemd/system/argus-helper.service
systemctl daemon-reload

rm -f /usr/local/bin/argus-agent /usr/local/bin/argus-helper /usr/local/bin/argusctl
rm -rf "$CONFIG_DIR" "$INSTALL_DIR" "$STATE_DIR"

if id argus >/dev/null 2>&1; then
  userdel argus >/dev/null 2>&1 || true
fi
if getent group argus >/dev/null 2>&1; then
  groupdel argus >/dev/null 2>&1 || true
fi

echo "Argus first-test installation and data removed. Docker itself was left installed."
