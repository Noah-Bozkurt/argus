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
  if [[ "$1" == POST && "$2" == /commands ]]; then
    printf '%s\n' "$3"
  elif [[ "$1" == GET && "$2" == /background-jobs ]]; then
    printf '%s\n' '{"jobs":[{"id":"00000000-0000-4000-8000-000000000010","job_kind":"site_monitor.check","resource_key":"site","status":"SUCCEEDED","completed_at":"2026-08-20T00:01:00Z"}]}'
  elif [[ "$1" == GET && "$2" == /servers ]]; then
    printf '%s\n' '[{"server_id":"00000000-0000-4000-8000-000000000005","snapshot":{"backups":{"artifacts":[{"name":"backup.tar.gz","profile":"system-config","size_bytes":10,"sha256":"abc","verified":true}]}}}]'
  else
    return 1
  fi
}
queued="$(queue_command '{"kind":"service.status","service":"argus-agent.service"}' LOW)"
jq -e --arg server "$ARGUS_SERVER_ID" '
  .server_id == $server and .command_type.kind == "service.status" and
  .command_type.service == "argus-agent.service" and .risk_level == "LOW" and
  .ttl_seconds == 300 and (.idempotency_key | startswith("acceptance-"))
' <<<"$queued" >/dev/null
backup_queued="$(queue_command '{"kind":"backup.create","profile":"system-config"}' MEDIUM)"
jq -e '.command_type.kind == "backup.create" and .command_type.profile == "system-config" and .risk_level == "MEDIUM"' <<<"$backup_queued" >/dev/null
[[ "$(wait_for_job site_monitor.check site '' 1 | jq -r .id)" == 00000000-0000-4000-8000-000000000010 ]]
wait_for_backup backup.tar.gz true 1

write_checkpoint project environment service site safe-command protected-command monitor-job backup-command.tar.gz backup-command verify-command preflight-command
[[ "$(stat -c '%a' "$CHECKPOINT_FILE")" == 600 ]]
# shellcheck disable=SC1090
. "$CHECKPOINT_FILE"
[[ "$PROJECT_ID" == project && "$PROTECTED_COMMAND_ID" == protected-command ]]
[[ "$MONITOR_JOB_ID" == monitor-job && "$BACKUP_NAME" == backup-command.tar.gz ]]
[[ "$BACKUP_COMMAND_ID" == backup-command && "$VERIFY_COMMAND_ID" == verify-command && "$PREFLIGHT_COMMAND_ID" == preflight-command ]]

# The lifecycle report must consume and expose the expanded product evidence.
product_project=00000000-0000-4000-8000-000000000020
monitor_job=00000000-0000-4000-8000-000000000021
backup_command=00000000-0000-4000-8000-000000000022
verify_command=00000000-0000-4000-8000-000000000023
preflight_command=00000000-0000-4000-8000-000000000024
write_checkpoint "$product_project" environment service site safe-command protected-command "$monitor_job" "$backup_command.tar.gz" "$backup_command" "$verify_command" "$preflight_command"

# Restore lifecycle helpers after sourcing the product helper library above.
# shellcheck disable=SC1091
source "$(dirname "$0")/first-server-acceptance.sh" --internal-test-library
write_checkpoint baseline REVISION "$FROM_REVISION" BOOT_ID 00000000-0000-4000-8000-000000000001 CONFIG_FINGERPRINT "$fingerprint_one"
write_checkpoint post-reboot COMPLETED_AT 2026-08-20T00:00:00Z REVISION "$FROM_REVISION" BOOT_ID 00000000-0000-4000-8000-000000000002
write_checkpoint installer-rerun REVISION "$FROM_REVISION"
write_checkpoint post-update FROM_REVISION "$FROM_REVISION" TO_REVISION "$TO_REVISION" TRANSACTION "$successful"
write_checkpoint content PROJECT_ID "$product_project" MODEL_ID 00000000-0000-4000-8000-000000000025 RECORD_ID 00000000-0000-4000-8000-000000000026 MODEL_SLUG acceptance_test
mkdir -p "$successful"
printf 'FROM_REVISION=%s\nTO_REVISION=%s\n' "$FROM_REVISION" "$TO_REVISION" >"$successful/metadata.env"
printf 'SUCCEEDED\n' >"$successful/result"
stage_report >/dev/null
grep -Fq "scheduled_site_monitor_job: $monitor_job" "$REPORT_FILE"
grep -Fq "verified_system_config_backup: $backup_command.tar.gz" "$REPORT_FILE"
grep -Fq "restore_preflight_command: $preflight_command" "$REPORT_FILE"
grep -Fq "cms_model: 00000000-0000-4000-8000-000000000025" "$REPORT_FILE"
grep -Fq "cms_draft_publication_public_read_verified: yes" "$REPORT_FILE"

printf 'first-server acceptance helper tests passed\n'
