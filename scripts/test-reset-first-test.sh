#!/usr/bin/env bash
set -Eeuo pipefail

# shellcheck disable=SC1091
source "$(dirname "$0")/reset-first-test.sh" --internal-test-library

validate_removal_target ARGUS_STATE_DIR /tmp/argus-reset-test/state
if (validate_removal_target ARGUS_STATE_DIR /); then
  echo "root removal target unexpectedly accepted" >&2
  exit 1
fi
if (validate_removal_target ARGUS_STATE_DIR relative/path); then
  echo "relative removal target unexpectedly accepted" >&2
  exit 1
fi
if (validate_removal_target ARGUS_STATE_DIR /var/lib/argus/../..); then
  echo "traversal removal target unexpectedly accepted" >&2
  exit 1
fi

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
INSTALL_DIR="$tmp/install"
CONFIG_DIR="$tmp/config"
STATE_DIR="$tmp/state"
mkdir -p "$INSTALL_DIR" "$CONFIG_DIR" "$STATE_DIR"
touch "$INSTALL_DIR/compose.yaml" "$INSTALL_DIR/.env"
ARGUS_CONFIRM_RESET=DELETE-ARGUS-FIRST-TEST-DATA
docker() { return 1; }
if (perform_reset); then
  echo "reset unexpectedly continued after Compose teardown failure" >&2
  exit 1
fi
[[ -f "$INSTALL_DIR/compose.yaml" && -d "$CONFIG_DIR" && -d "$STATE_DIR" ]] || {
  echo "reset removed recovery state after Compose teardown failure" >&2
  exit 1
}

printf 'reset-first-test safety helper tests passed\n'
