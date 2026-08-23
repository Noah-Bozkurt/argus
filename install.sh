#!/usr/bin/env bash
set -Eeuo pipefail
umask 077

ORIGIN="${ARGUS_INSTALLER_ORIGIN:-https://install.noahbozkurt.nl}"
ARCH="$(uname -m)"
case "$ARCH" in
  x86_64|amd64) ASSET="argus-installer-x86_64" ;;
  *) printf 'Argus installer does not support architecture %s yet.\n' "$ARCH" >&2; exit 1 ;;
esac

TMP="$(mktemp -d)"
cleanup() { rm -rf -- "$TMP"; }
trap cleanup EXIT

curl -fsSL "$ORIGIN/manifest.json" -o "$TMP/manifest.json"
REVISION="$(sed -n 's/.*"revision"[[:space:]]*:[[:space:]]*"\([0-9a-f]\{40\}\)".*/\1/p' "$TMP/manifest.json" | head -n1)"
EXPECTED="$(sed -n 's/.*"installer_sha256"[[:space:]]*:[[:space:]]*"\([0-9a-f]\{64\}\)".*/\1/p' "$TMP/manifest.json" | head -n1)"
[[ "$REVISION" =~ ^[0-9a-f]{40}$ ]] || { printf 'Argus installer manifest has an invalid revision.\n' >&2; exit 1; }
[[ "$EXPECTED" =~ ^[0-9a-f]{64}$ ]] || { printf 'Argus installer manifest has an invalid checksum.\n' >&2; exit 1; }

curl -fsSL "$ORIGIN/bin/$ASSET" -o "$TMP/argus-installer"
ACTUAL="$(sha256sum "$TMP/argus-installer" | awk '{print $1}')"
[[ "$ACTUAL" == "$EXPECTED" ]] || { printf 'Argus installer checksum verification failed.\n' >&2; exit 1; }
chmod 0755 "$TMP/argus-installer"

export ARGUS_VERSION="${ARGUS_VERSION:-$REVISION}"
exec "$TMP/argus-installer" "$@"