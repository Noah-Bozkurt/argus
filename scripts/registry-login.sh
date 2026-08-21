#!/usr/bin/env bash
set -Eeuo pipefail

args=(registry-login)
if [[ -n "${ARGUS_REGISTRY_USERNAME_OVERRIDE:-}" ]]; then
  args+=(--username "$ARGUS_REGISTRY_USERNAME_OVERRIDE")
fi

exec /usr/local/bin/argus-installer "${args[@]}"
