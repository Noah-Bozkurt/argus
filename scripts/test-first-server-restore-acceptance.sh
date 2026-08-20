#!/usr/bin/env bash
set -Eeuo pipefail

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
export ARGUS_INSTALL_DIR="$tmp/install"
export ARGUS_STATE_DIR="$tmp/state"
export ARGUS_ACCEPTANCE_DIR="$tmp/acceptance"
mkdir -p "$ARGUS_INSTALL_DIR" "$ARGUS_STATE_DIR" "$ARGUS_ACCEPTANCE_DIR"

# shellcheck disable=SC1091
source "$(dirname "$0")/first-server-restore-acceptance.sh" --internal-test-library
ARGUS_SERVER_ID=00000000-0000-4000-8000-000000000005
request="$(command_request 00000000-0000-4000-8000-000000000010.tar.gz acceptance-key)"
jq -e --arg server "$ARGUS_SERVER_ID" '
  .server_id == $server and .command_type.kind == "backup.restore.apply" and
  .command_type.backup == "00000000-0000-4000-8000-000000000010.tar.gz" and
  .ttl_seconds == 600 and .idempotency_key == "acceptance-key" and .risk_level == "CRITICAL"
' <<<"$request" >/dev/null

systemctl() { return 3; }
wait_for_commit 00000000-0000-4000-8000-000000000011 1
write_checkpoint 00000000-0000-4000-8000-000000000010.tar.gz \
  00000000-0000-4000-8000-000000000011 00000000-0000-4000-8000-000000000012
[[ "$(stat -c '%a' "$CHECKPOINT_FILE")" == 600 ]]
# shellcheck disable=SC1090
. "$CHECKPOINT_FILE"
[[ "$MAINTENANCE_GATE_VERIFIED" == yes && "$TIMED_ROLLBACK_DISARMED" == yes && "$POST_RESTORE_SMOKE" == yes ]]

printf 'first-server restore acceptance helper tests passed\n'
