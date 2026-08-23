#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
spec="$root/apps/web/openapi/control-api.json"
generated="$root/apps/web/lib/generated/control-api.ts"
tmp="$(mktemp)"
trap 'rm -f "$tmp"' EXIT

mkdir -p "$(dirname "$spec")" "$(dirname "$generated")"
(
  cd "$root"
  cargo run --quiet -p control-api -- --print-openapi > "$tmp"
)
mv "$tmp" "$spec"
pnpm --dir "$root/apps/web" run generate:api
