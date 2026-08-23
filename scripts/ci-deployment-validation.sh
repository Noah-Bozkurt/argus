#!/usr/bin/env bash
set -euo pipefail

(
  bash -n install.sh
  bash -n scripts/first-server-smoke.sh
  bash -n scripts/test-native-cms-runtime.sh
  bash -n scripts/test-media-runtime.sh
  bash -n scripts/test-forms-runtime.sh
  bash -n scripts/update-first-test.sh
  bash -n scripts/recover-interrupted-update.sh
  bash -n scripts/uninstall.sh
  grep -Fq 'argus-installer' install.sh
  grep -Fq 'argus-installer' scripts/uninstall.sh
  grep -Fq 'pg_dump' scripts/update-first-test.sh
  grep -Fq 'pg_restore' scripts/update-first-test.sh
  grep -Fq 'ROLLBACK_READY=1' scripts/update-first-test.sh
  grep -Fq 'TRANSACTION_FORMAT_VERSION=2' scripts/update-first-test.sh
  grep -Fq 'file-snapshot.sha256' scripts/update-first-test.sh
  grep -Fq 'database-snapshot.sha256' scripts/update-first-test.sh
  grep -Fq 'target-start-armed' scripts/update-first-test.sh
  grep -Fq 'pg_database_size(current_database())' scripts/update-first-test.sh
  grep -Fq 'df -PB1' scripts/update-first-test.sh
  grep -Fq 'ExecStartPre=/usr/local/bin/argusctl recover-update' deploy/systemd/argus-helper.service
  grep -Fq 'ABORTED_PRE_MUTATION' scripts/recover-interrupted-update.sh
  grep -Fq 'ARGUS_UPDATE_RECOVERY_RETRY_FAILED' scripts/recover-interrupted-update.sh
  grep -Fq 'recover-update --retry-failed' scripts/recover-interrupted-update.sh
  grep -Fq 'pg_restore --list' scripts/recover-interrupted-update.sh
  grep -Fq 'up -d --no-deps --force-recreate caddy' scripts/update-first-test.sh
  grep -Fq 'up -d --no-deps --force-recreate caddy' scripts/recover-interrupted-update.sh
  grep -Fq 'Caddyfile.rendered' scripts/update-first-test.sh
  grep -Fq 'ARGUS_GLOBAL_OPTIONS' scripts/update-first-test.sh
  grep -Fq 'ARGUS_TLS_MODE' scripts/update-first-test.sh
  grep -Fq 'delegating update transaction to target runner' scripts/update-first-test.sh
  grep -Fq 'target update runner revision does not match the verified image set' scripts/update-first-test.sh
  grep -Fq '/proc/$BASHPID/fd/9' scripts/update-first-test.sh
  grep -Fq 'ARGUS_UPDATE_DELEGATED_REVISION' crates/cli/src/main.rs
  grep -Fq 'org.argus.update-runner-protocol' docker-bake.hcl
  grep -Fq 'UPDATE_RUNNER_PROTOCOL_VERSION=1' scripts/update-first-test.sh
  grep -Fq 'using verified prepared target revision' scripts/update-first-test.sh
  grep -Fq 'still working' scripts/update-first-test.sh
  grep -Fq '"$tls_dir:/etc/caddy/argus-tls:ro"' scripts/update-first-test.sh
  grep -Fq "grep -Eq '__[A-Z0-9_]+__'" scripts/update-first-test.sh
  update_lock="$(sed -n 's/^LOCK_FILE="\(.*\)"$/\1/p' scripts/update-first-test.sh | head -n1)"
  recovery_lock="$(sed -n 's/^LOCK_FILE="\(.*\)"$/\1/p' scripts/recover-interrupted-update.sh | head -n1)"
  [[ -n "$update_lock" && "$update_lock" == "$recovery_lock" ]]
)

(
  tmp="$(mktemp -d)"
  export ARGUS_STATE_DIR="$tmp/state"
  source scripts/update-first-test.sh --internal-test-library
  [[ "$(required_snapshot_space_bytes 0)" == "1073741824" ]]
  [[ "$(required_snapshot_space_bytes 1073741824)" == "3221225472" ]]
  mkdir -p "$BACKUP_ROOT"
  make_transaction() {
    local name="$1" result="${2:-}"
    mkdir -p "$BACKUP_ROOT/$name/files"
    printf 'FROM_REVISION=x\n' >"$BACKUP_ROOT/$name/metadata.env"
    [[ -z "$result" ]] || printf '%s\n' "$result" >"$BACKUP_ROOT/$name/result"
  }
  make_transaction 20260819T120005Z-aaaaaaaaaaaa-to-bbbbbbbbbbbb SUCCEEDED
  make_transaction 20260819T120004Z-aaaaaaaaaaaa-to-bbbbbbbbbbbb ROLLED_BACK
  make_transaction 20260819T120003Z-aaaaaaaaaaaa-to-bbbbbbbbbbbb SUCCEEDED
  make_transaction 20260819T120002Z-aaaaaaaaaaaa-to-bbbbbbbbbbbb ROLLED_BACK
  make_transaction 20260819T120001Z-aaaaaaaaaaaa-to-bbbbbbbbbbbb SUCCEEDED
  make_transaction 20260819T115959Z-aaaaaaaaaaaa-to-bbbbbbbbbbbb ROLLBACK_FAILED
  make_transaction 20260819T115958Z-aaaaaaaaaaaa-to-bbbbbbbbbbbb
  make_transaction manual-snapshot SUCCEEDED
  prune_completed_transactions
  [[ -d "$BACKUP_ROOT/20260819T120005Z-aaaaaaaaaaaa-to-bbbbbbbbbbbb" ]]
  [[ -d "$BACKUP_ROOT/20260819T120004Z-aaaaaaaaaaaa-to-bbbbbbbbbbbb" ]]
  [[ -d "$BACKUP_ROOT/20260819T120003Z-aaaaaaaaaaaa-to-bbbbbbbbbbbb" ]]
  [[ ! -e "$BACKUP_ROOT/20260819T120002Z-aaaaaaaaaaaa-to-bbbbbbbbbbbb" ]]
  [[ ! -e "$BACKUP_ROOT/20260819T120001Z-aaaaaaaaaaaa-to-bbbbbbbbbbbb" ]]
  [[ -d "$BACKUP_ROOT/20260819T115959Z-aaaaaaaaaaaa-to-bbbbbbbbbbbb" ]]
  [[ -d "$BACKUP_ROOT/20260819T115958Z-aaaaaaaaaaaa-to-bbbbbbbbbbbb" ]]
  [[ -d "$BACKUP_ROOT/manual-snapshot" ]]
)

(
  tmp="$(mktemp -d)"
  export ARGUS_STATE_DIR="$tmp/state"
  source scripts/update-first-test.sh --internal-test-library
  CURRENT_REVISION=bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
  REQUESTED_VERSION=main
  ARGUS_REGISTRY=registry.example/argus
  pulls=()
  pull_image() { pulls+=("$1"); }
  image_revision() { printf '%s\n' "$CURRENT_REVISION"; }
  validate_version_tag() { :; }
  validate_revision() { :; }
  log() { :; }
  verify_target_images() { return 1; }
  pull_and_verify_target
  [[ "${pulls[*]}" == "registry.example/argus/argus-host-tools:main" ]]
  [[ "$TARGET_REVISION" == "$CURRENT_REVISION" ]]
)

(
  tmp="$(mktemp -d)"
  export ARGUS_STATE_DIR="$tmp/state"
  source scripts/update-first-test.sh --internal-test-library
  ARGUS_REGISTRY=registry.example/argus
  REQUESTED_VERSION=main
  TARGET_REVISION=bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
  pulls=()
  tags=()
  try_pull_image() {
    pulls+=("$1")
    if [[ "$1" == *":$TARGET_REVISION" ]]; then
      PULL_ERROR="manifest unknown"
      return 1
    fi
  }
  image_revision() { printf '%s\n' "$TARGET_REVISION"; }
  docker() { [[ "$1" == tag ]]; tags+=("$2 $3"); }
  pull_revision_image argus-web
  [[ "${pulls[*]}" == "registry.example/argus/argus-web:$TARGET_REVISION registry.example/argus/argus-web:main" ]]
  [[ "${tags[*]}" == "registry.example/argus/argus-web:main registry.example/argus/argus-web:$TARGET_REVISION" ]]

  image_revision() { printf '%s\n' aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa; }
  if (pull_revision_image argus-worker); then
    echo "mismatched promoted image was accepted" >&2
    exit 1
  fi
)

(
  tmp="$(mktemp -d)"
  export ARGUS_STATE_DIR="$tmp/state"
  source scripts/update-first-test.sh --internal-test-library
  mkdir -p "$ARGUS_STATE_DIR"
  exec 9>"$LOCK_FILE"
  flock -n 9
  DELEGATED_REVISION=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
  acquire_update_lock
  if (exec 9>&-; acquire_update_lock); then exit 1; fi
  DELEGATED_REVISION=invalid
  if (validate_delegated_runner); then exit 1; fi
  DELEGATED_REVISION=""
  TARGET_REVISION=bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
  TARGET_TMP="$tmp/target"
  mkdir -p "$TARGET_TMP"
  printf '#!/usr/bin/env bash\nprintf "%%s\\n" "$ARGUS_UPDATE_DELEGATED_REVISION" "$ARGUS_UPDATE_DELEGATED_RUNNER_SHA256" "$ARGUS_UPDATE_PREPARED_REVISION" "$*" >"%s/handoff"\n' "$tmp" >"$TARGET_TMP/argusctl"
  chmod 0700 "$TARGET_TMP/argusctl"
  delegate_to_target_runner
  mapfile -t handoff <"$tmp/handoff"
  [[ "${handoff[0]}" == "$TARGET_REVISION" ]]
  [[ "${handoff[1]}" =~ ^[0-9a-f]{64}$ ]]
  [[ "${handoff[2]}" == "$TARGET_REVISION" ]]
  [[ "${handoff[3]}" == "update --version $TARGET_REVISION --yes --verbose" ]]

  DELEGATED_REVISION="$TARGET_REVISION"
  PREPARED_REVISION="$TARGET_REVISION"
  verify_target_images() { TARGET_RUNNER_PROTOCOL=1; TARGET_BRANCH_PROTOCOL=1; }
  pull_and_verify_target() { return 99; }
  prepare_update_target
  [[ "$TARGET_REVISION" == "$PREPARED_REVISION" ]]
  PREPARED_REVISION=""
  REQUESTED_VERSION="$DELEGATED_REVISION"
  prepare_update_target
  [[ "$PREPARED_REVISION" == "$DELEGATED_REVISION" ]]
)

(
  tmp="$(mktemp -d)"
  export ARGUS_STATE_DIR="$tmp/update-state"
  source scripts/update-first-test.sh --internal-test-library
  ARGUS_UPDATE_ACCEPTANCE_FAILURE=after-target-start-armed
  ARGUS_UPDATE_ACCEPTANCE_CONFIRM_FAILURE=wrong
  if (validate_acceptance_failure_hook); then exit 1; fi
  ARGUS_UPDATE_ACCEPTANCE_CONFIRM_FAILURE=ROLLBACK-TEST-ONLY
  validate_acceptance_failure_hook
  unset ARGUS_UPDATE_ACCEPTANCE_FAILURE ARGUS_UPDATE_ACCEPTANCE_CONFIRM_FAILURE
  TRANSACTION_DIR="$BACKUP_ROOT/20260819T130000Z-aaaaaaaaaaaa-to-bbbbbbbbbbbb"
  TARGET_REVISION=bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
  mkdir -p "$TRANSACTION_DIR"
  while IFS= read -r path; do
    mkdir -p "$(dirname "$TRANSACTION_DIR/$path")"
    printf 'snapshot:%s\n' "$path" >"$TRANSACTION_DIR/$path"
  done < <(file_snapshot_paths)
  seal_file_snapshot
  verify_file_snapshot "$TRANSACTION_DIR"
  printf 'corruption\n' >>"$TRANSACTION_DIR/files/bin/argusctl"
  if verify_file_snapshot "$TRANSACTION_DIR"; then exit 1; fi
  printf 'snapshot:files/bin/argusctl\n' >"$TRANSACTION_DIR/files/bin/argusctl"
  seal_file_snapshot
  printf 'fake-dump\n' >"$TRANSACTION_DIR/argus.dump"
  (cd "$TRANSACTION_DIR" && sha256sum argus.dump >database-snapshot.sha256)
  verify_database_snapshot_file "$TRANSACTION_DIR"
  DATABASE_BACKUP_READY=1
  arm_target_start
  [[ "$(cat "$TRANSACTION_DIR/target-start-armed")" == "$TARGET_REVISION" ]]
)

(
  tmp="$(mktemp -d)"
  trap 'rm -rf "$tmp"' EXIT
  source scripts/update-first-test.sh --internal-test-library
  install -m 0755 /bin/sleep "$tmp/running"
  "$tmp/running" 30 &
  running_pid=$!
  atomic_install_file /bin/true "$tmp/running" 0755
  "$tmp/running"
  kill "$running_pid"
  wait "$running_pid" 2>/dev/null || true
)

(
  tmp="$(mktemp -d)"
  trap 'rm -rf "$tmp"' EXIT
  source scripts/recover-interrupted-update.sh --internal-test-library
  install -m 0755 /bin/sleep "$tmp/running"
  "$tmp/running" 30 &
  running_pid=$!
  atomic_install_file /bin/true "$tmp/running" 0755
  "$tmp/running"
  kill "$running_pid"
  wait "$running_pid" 2>/dev/null || true
)

(
  tmp="$(mktemp -d)"
  export ARGUS_STATE_DIR="$tmp/recovery-state"
  export ARGUS_INSTALL_DIR="$tmp/recovery-install"
  mkdir -p "$ARGUS_STATE_DIR/update-backups" "$ARGUS_INSTALL_DIR"
  FROM=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
  TO=bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
  early="$ARGUS_STATE_DIR/update-backups/20260819T130100Z-aaaaaaaaaaaa-to-bbbbbbbbbbbb"
  mkdir -p "$early"
  cat >"$early/metadata.env" <<META
TRANSACTION_FORMAT=2
FROM_REVISION=$FROM
TO_REVISION=$TO
META
  printf 'ARGUS_VERSION=%s\n' "$FROM" >"$ARGUS_INSTALL_DIR/.env"
  source scripts/recover-interrupted-update.sh --internal-test-library
  load_transaction_metadata "$early"
  if prepare_transaction_recovery "$early"; then exit 1; fi
  [[ "$(cat "$early/result")" == 'ABORTED_PRE_MUTATION' ]]
  failed="$ARGUS_STATE_DIR/update-backups/20260819T130200Z-aaaaaaaaaaaa-to-bbbbbbbbbbbb"
  mkdir -p "$failed"
  cat >"$failed/metadata.env" <<META
TRANSACTION_FORMAT=2
FROM_REVISION=$FROM
TO_REVISION=$TO
META
  printf 'ROLLBACK_FAILED\n' >"$failed/result"
  RETRY_FAILED=0
  if find_recovery_transaction >/dev/null 2>&1; then exit 1; else [[ $? -eq 2 ]]; fi
  RETRY_FAILED=1
  [[ "$(find_recovery_transaction)" == "$failed" ]]
)

(
  cp deploy/compose/Caddyfile.template deploy/compose/Caddyfile
  compose_env="$RUNNER_TEMP/argus-test.env"
  cat >"$compose_env" <<'ENV'
ARGUS_REGISTRY=ghcr.io/noah-bozkurt
ARGUS_VERSION=test
ARGUS_DOMAIN=argus.example.test
ARGUS_CONTENT_DOMAIN=content.argus.example.test
ARGUS_POSTGRES_PASSWORD=test-postgres-password
ARGUS_WEB_API_TOKEN=0123456789abcdef0123456789abcdef
ARGUS_WORKER_TOKEN=abcdef0123456789abcdef0123456789
ARGUS_CONTENT_SYNC_TOKEN=11111111111111111111111111111111
PAYLOAD_SECRET=22222222222222222222222222222222
ARGUS_ORG_ID=00000000-0000-4000-8000-000000000001
ARGUS_USER_ID=00000000-0000-4000-8000-000000000002
ARGUS_SERVER_ID=00000000-0000-4000-8000-000000000003
ARGUS_GITHUB_TOKEN=
ARGUS_RUST_LOG=info
ENV
  docker compose --project-directory deploy/compose --env-file "$compose_env" -f deploy/compose/compose.yaml config >/dev/null
  caddyfile="$RUNNER_TEMP/argus-Caddyfile"
  hash="$(docker run --rm caddy:2-alpine caddy hash-password --plaintext test-password)"
  cp deploy/compose/Caddyfile.template "$caddyfile"
  sed -i -e 's|__ARGUS_GLOBAL_OPTIONS__||g' -e 's|__ARGUS_DOMAIN__|argus.example.test|g' -e 's|__ARGUS_CONTENT_DOMAIN__|content.argus.example.test|g' -e 's|__ARGUS_TLS__|tls internal|g' -e 's|__BASIC_AUTH_USER__|argus|g' -e "s|__BASIC_AUTH_HASH__|${hash}|g" "$caddyfile"
  grep -Fq 'path /public/status/*' "$caddyfile"
  grep -Fq 'path /public/* /api/media/file/*' "$caddyfile"

  caddy_container="$(docker create caddy:2-alpine caddy validate --config /etc/caddy/Caddyfile)"
  trap 'docker rm -f "$caddy_container" >/dev/null 2>&1 || true' EXIT
  docker cp "$caddyfile" "$caddy_container:/etc/caddy/Caddyfile"
  docker start -a "$caddy_container"
)
