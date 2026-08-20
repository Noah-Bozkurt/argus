#!/usr/bin/env bash
set -Eeuo pipefail
umask 077

CONFIG_DIR="${ARGUS_CONFIG_DIR:-/etc/argus}"
CREDENTIAL_FILE="$CONFIG_DIR/registry.env"
REGISTRY="${ARGUS_REGISTRY:-ghcr.io/noah-bozkurt}"
USERNAME="${ARGUS_REGISTRY_USERNAME_OVERRIDE:-${ARGUS_REGISTRY_USERNAME:-}}"
TOKEN="${ARGUS_REGISTRY_TOKEN:-}"
DOCKER_CONFIG_DIR=""

die() { printf '[argus-registry] error: %s\n' "$*" >&2; exit 1; }
cleanup() { [[ -z "$DOCKER_CONFIG_DIR" ]] || rm -rf -- "$DOCKER_CONFIG_DIR"; }
trap cleanup EXIT

main() {
  [[ "${EUID}" -eq 0 ]] || die "run as root (sudo argusctl registry-login)"
  command -v docker >/dev/null || die "docker is required"
  if [[ -z "$USERNAME" ]]; then
    [[ -t 0 ]] || die "--username is required in non-interactive mode"
    read -r -p 'GitHub username: ' USERNAME
  fi
  [[ "$USERNAME" =~ ^[A-Za-z0-9][A-Za-z0-9-]{0,38}$ && "$USERNAME" =~ [A-Za-z0-9]$ && "$USERNAME" != *--* ]] \
    || die "invalid GitHub username"
  if [[ -z "$TOKEN" ]]; then
    [[ -t 0 ]] || die "ARGUS_REGISTRY_TOKEN is required in non-interactive mode"
    read -r -s -p 'GitHub token (classic PAT with read:packages): ' TOKEN
    printf '\n'
  fi
  [[ -n "$TOKEN" ]] || die "GitHub token is required"

  DOCKER_CONFIG_DIR="$(mktemp -d)"; chmod 0700 "$DOCKER_CONFIG_DIR"
  printf '%s' "$TOKEN" | DOCKER_CONFIG="$DOCKER_CONFIG_DIR" docker login "${REGISTRY%%/*}" -u "$USERNAME" --password-stdin >/dev/null \
    || die "GHCR login failed; verify the classic PAT has read:packages"

  install -m 0700 -d "$CONFIG_DIR"
  local tmp
  tmp="$(mktemp "$CONFIG_DIR/registry.env.XXXXXX")"
  {
    printf 'ARGUS_REGISTRY=%q\n' "$REGISTRY"
    printf 'ARGUS_REGISTRY_USERNAME=%q\n' "$USERNAME"
    printf 'ARGUS_REGISTRY_TOKEN=%q\n' "$TOKEN"
  } >"$tmp"
  chmod 0600 "$tmp"
  mv "$tmp" "$CREDENTIAL_FILE"
  printf 'Stored validated GHCR credentials in %s (mode 0600).\n' "$CREDENTIAL_FILE"
}

if [[ "${1:-}" != "--internal-test-library" ]]; then main "$@"; fi
