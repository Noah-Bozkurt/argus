#!/usr/bin/env bash
set -Eeuo pipefail
umask 077

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

export ARGUS_INSTALL_DIR="$tmp/install"
export ARGUS_STATE_DIR="$tmp/state"
export ARGUS_ACCEPTANCE_DIR="$tmp/acceptance"
mkdir -p "$ARGUS_INSTALL_DIR" "$ARGUS_STATE_DIR/update-backups"

# shellcheck disable=SC1091
source "$(dirname "$0")/first-server-acceptance.sh" --internal-test-library

FROM_REVISION=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
TO_REVISION=bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb

is_revision "$FROM_REVISION"
if is_revision main; then
  echo "mutable version unexpectedly accepted as immutable revision" >&2
  exit 1
fi
if is_revision AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA; then
  echo "uppercase revision unexpectedly accepted" >&2
  exit 1
fi

write_env() {
  local revision="$1" postgres_password="$2"
  cat >"$ENV_FILE" <<EOF
ARGUS_VERSION=$revision
ARGUS_POSTGRES_PASSWORD=$postgres_password
ARGUS_WEB_API_TOKEN=web-token
ARGUS_WORKER_TOKEN=worker-token
ARGUS_CONTENT_SYNC_TOKEN=content-token
PAYLOAD_SECRET=payload-secret
ARGUS_ORG_ID=00000000-0000-4000-8000-000000000001
ARGUS_USER_ID=00000000-0000-4000-8000-000000000002
ARGUS_BOOTSTRAP_PROJECT_ID=00000000-0000-4000-8000-000000000003
ARGUS_BOOTSTRAP_ENVIRONMENT_ID=00000000-0000-4000-8000-000000000004
ARGUS_SERVER_ID=00000000-0000-4000-8000-000000000005
EOF
  chmod 0600 "$ENV_FILE"
}

write_env "$FROM_REVISION" postgres-one
[[ "$(installed_revision)" == "$FROM_REVISION" ]]
fingerprint_one="$(configuration_fingerprint)"
[[ "$fingerprint_one" =~ ^[0-9a-f]{64}$ ]]

# Installed revision is lifecycle state, not generated identity/secret material.
write_env "$TO_REVISION" postgres-one
fingerprint_revision_changed="$(configuration_fingerprint)"
[[ "$fingerprint_revision_changed" == "$fingerprint_one" ]]

# A generated secret change must change the fingerprint.
write_env "$TO_REVISION" postgres-two
fingerprint_secret_changed="$(configuration_fingerprint)"
[[ "$fingerprint_secret_changed" != "$fingerprint_one" ]]

write_checkpoint baseline \
  REVISION "$FROM_REVISION" \
  BOOT_ID 00000000-0000-4000-8000-000000000001 \
  CONFIG_FINGERPRINT "$fingerprint_one"
[[ "$(checkpoint_value baseline REVISION)" == "$FROM_REVISION" ]]
[[ "$(stat -c '%a' "$(checkpoint_path baseline)")" == "600" ]]
[[ "$(stat -c '%a' "$ACCEPTANCE_DIR")" == "700" ]]

successful="$ARGUS_STATE_DIR/update-backups/20260819T140000Z-aaaaaaaaaaaa-to-bbbbbbbbbbbb"
failed="$ARGUS_STATE_DIR/update-backups/20260819T140100Z-aaaaaaaaaaaa-to-bbbbbbbbbbbb"
unrelated="$ARGUS_STATE_DIR/update-backups/20260819T140200Z-bbbbbbbbbbbb-to-cccccccccccc"
mkdir -p "$successful" "$failed" "$unrelated"
cat >"$successful/metadata.env" <<EOF
FROM_REVISION=$FROM_REVISION
TO_REVISION=$TO_REVISION
EOF
printf 'SUCCEEDED\n' >"$successful/result"
cat >"$failed/metadata.env" <<EOF
FROM_REVISION=$FROM_REVISION
TO_REVISION=$TO_REVISION
EOF
printf 'ROLLBACK_FAILED\n' >"$failed/result"
cat >"$unrelated/metadata.env" <<EOF
FROM_REVISION=$TO_REVISION
TO_REVISION=cccccccccccccccccccccccccccccccccccccccc
EOF
printf 'SUCCEEDED\n' >"$unrelated/result"

selected="$(find_successful_update_transaction "$FROM_REVISION" "$TO_REVISION")"
[[ "$selected" == "$successful" ]]

rm -rf "$successful"
if find_successful_update_transaction "$FROM_REVISION" "$TO_REVISION" >/dev/null; then
  echo "failed/unrelated update transaction unexpectedly accepted" >&2
  exit 1
fi

# Product-acceptance helpers must produce typed command requests and root-only evidence.
# shellcheck disable=SC1091
source "$(dirname "$0")/first-server-product-acceptance.sh" --internal-test-library
ARGUS_SERVER_ID=00000000-0000-4000-8000-000000000005
api() {
  [[ "$1" == POST && "$2" == /commands ]]
  printf '%s\n' "$3"
}
queued="$(queue_command '{"kind":"service.status","service":"argus-agent.service"}' LOW)"
jq -e --arg server "$ARGUS_SERVER_ID" '
  .server_id == $server and .command_type.kind == "service.status" and
  .command_type.service == "argus-agent.service" and .risk_level == "LOW" and
  .ttl_seconds == 300 and (.idempotency_key | startswith("acceptance-"))
' <<<"$queued" >/dev/null

write_checkpoint project environment service site safe-command protected-command
[[ "$(stat -c '%a' "$CHECKPOINT_FILE")" == 600 ]]
# shellcheck disable=SC1090
. "$CHECKPOINT_FILE"
[[ "$PROJECT_ID" == project && "$PROTECTED_COMMAND_ID" == protected-command ]]

printf 'first-server acceptance helper tests passed\n'
