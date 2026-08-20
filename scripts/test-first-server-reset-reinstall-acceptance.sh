#!/usr/bin/env bash
set -Eeuo pipefail

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
export ARGUS_INSTALL_DIR="$tmp/install"
export ARGUS_CONFIG_DIR="$tmp/config"
export ARGUS_STATE_DIR="$tmp/state"
export ARGUS_ACCEPTANCE_DIR="$tmp/state/acceptance/first-server"
export ARGUS_ACCEPTANCE_ARCHIVE_DIR="$tmp/archive"
mkdir -p "$ARGUS_INSTALL_DIR" "$ARGUS_CONFIG_DIR" "$ARGUS_ACCEPTANCE_DIR"

# shellcheck disable=SC1091
source "$(dirname "$0")/first-server-reset-reinstall-acceptance.sh" --internal-test-library
validate_archive_location

write_env() {
  local suffix="$1"
  cat >"$ENV_FILE" <<EOF
ARGUS_VERSION=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
ARGUS_POSTGRES_PASSWORD=postgres-$suffix
ARGUS_WEB_API_TOKEN=web-$suffix
ARGUS_WORKER_TOKEN=worker-$suffix
ARGUS_CONTENT_SYNC_TOKEN=content-$suffix
PAYLOAD_SECRET=payload-$suffix
ARGUS_ORG_ID=00000000-0000-4000-8000-00000000000$suffix
ARGUS_USER_ID=00000000-0000-4000-8000-00000000001$suffix
ARGUS_BOOTSTRAP_PROJECT_ID=00000000-0000-4000-8000-00000000002$suffix
ARGUS_BOOTSTRAP_ENVIRONMENT_ID=00000000-0000-4000-8000-00000000003$suffix
ARGUS_SERVER_ID=00000000-0000-4000-8000-00000000004$suffix
EOF
}

write_env 1
first="$(environment_fingerprint "$ENV_FILE")"
write_env 2
second="$(environment_fingerprint "$ENV_FILE")"
[[ "$first" =~ ^[0-9a-f]{64}$ && "$second" =~ ^[0-9a-f]{64}$ && "$first" != "$second" ]]

mkdir -p "$ARCHIVE_DIR"
printf 'result: PASS\n' >"$ARCHIVED_REPORT"
report_sha="$(sha256sum "$ARCHIVED_REPORT" | awk '{ print $1 }')"
write_final_report "$first" aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa "$report_sha" "$second" bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
final_report_sha="$(sha256sum "$FINAL_REPORT" | awk '{ print $1 }')"
write_checkpoint COMPLETE "$first" aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa "$report_sha" "$second" bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb "$final_report_sha"
[[ "$(stat -c '%a' "$CHECKPOINT_FILE")" == 600 && "$(stat -c '%a' "$FINAL_REPORT")" == 600 ]]
grep -Fq 'reset_installation_absence_verified: yes' "$FINAL_REPORT"
grep -Fq 'second_clean_install_new_identity: yes' "$FINAL_REPORT"
[[ "$(checkpoint_value LIFECYCLE_REPORT_SHA256)" == "$report_sha" ]]
[[ "$(checkpoint_value FINAL_REPORT_SHA256)" == "$final_report_sha" ]]
print_completed_report >/dev/null

ARCHIVE_DIR="$ARGUS_STATE_DIR/archive"
if (validate_archive_location); then
  echo "archive inside deleted state unexpectedly accepted" >&2
  exit 1
fi

printf 'first-server reset/reinstall acceptance helper tests passed\n'
