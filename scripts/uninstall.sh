#!/usr/bin/env bash
set -Eeuo pipefail

args=(uninstall)
[[ "${ARGUS_UNINSTALL_CONFIRM:-0}" == "1" ]] && args+=(--yes)
[[ "${ARGUS_UNINSTALL_PURGE_DATA:-0}" == "1" ]] && args+=(--purge-data)

exec /usr/local/bin/argus-installer "${args[@]}"
