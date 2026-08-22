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

printf 'Argus install\n\n'
printf '  › Downloading native installer\n'
curl -fsSL "$ORIGIN/manifest.json" -o "$TMP/manifest.json"
REVISION="$(sed -n 's/.*"revision"[[:space:]]*:[[:space:]]*"\([0-9a-f]\{40\}\)".*/\1/p' "$TMP/manifest.json" | head -n1)"
EXPECTED="$(sed -n 's/.*"installer_sha256"[[:space:]]*:[[:space:]]*"\([0-9a-f]\{64\}\)".*/\1/p' "$TMP/manifest.json" | head -n1)"
[[ "$REVISION" =~ ^[0-9a-f]{40}$ ]] || { printf '  ✗ Installer manifest has an invalid revision.\n' >&2; exit 1; }
[[ "$EXPECTED" =~ ^[0-9a-f]{64}$ ]] || { printf '  ✗ Installer manifest has an invalid checksum.\n' >&2; exit 1; }

if [[ -t 2 && "${TERM:-}" != "dumb" ]]; then
  # curl knows the actual transfer size, so use its real byte-based progress bar
  # instead of inventing a percentage in the bootstrap shell.
  curl -fL --progress-bar "$ORIGIN/bin/$ASSET" -o "$TMP/argus-installer"
else
  curl -fsSL "$ORIGIN/bin/$ASSET" -o "$TMP/argus-installer"
fi
ACTUAL="$(sha256sum "$TMP/argus-installer" | awk '{print $1}')"
[[ "$ACTUAL" == "$EXPECTED" ]] || { printf '  ✗ Installer checksum verification failed.\n' >&2; exit 1; }
chmod 0755 "$TMP/argus-installer"
printf '  ✓ Installer verified (%s)\n\n' "${REVISION:0:12}"

export ARGUS_VERSION="${ARGUS_VERSION:-$REVISION}"
exec "$TMP/argus-installer" "$@"
