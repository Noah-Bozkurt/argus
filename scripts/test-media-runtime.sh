#!/usr/bin/env bash
set -Eeuo pipefail

CONTENT_URL="${ARGUS_TEST_CONTENT_URL:-http://127.0.0.1:3000}"
TOKEN="${ARGUS_CONTENT_SYNC_TOKEN:?ARGUS_CONTENT_SYNC_TOKEN is required}"
ORG_ID="00000000-0000-4000-8000-000000000001"
OTHER_ORG_ID="00000000-0000-4000-8000-000000000099"
USER_ID="00000000-0000-4000-8000-000000000002"
PROJECT_ID="00000000-0000-4000-8000-000000000004"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

auth=(-H "Authorization: Bearer $TOKEN" -H "X-Argus-Org-Id: $ORG_ID" -H "X-Argus-User-Id: $USER_ID")

curl -fsS -X POST "$CONTENT_URL/internal/argus/project-sync" \
  -H "Authorization: Bearer $TOKEN" -H 'Content-Type: application/json' \
  --data "{\"organization_id\":\"$ORG_ID\",\"project_id\":\"$PROJECT_ID\",\"name\":\"Personal media acceptance\",\"client_id\":null,\"status\":\"active\"}" >/dev/null

# Keep the acceptance path repeatable against a retained local test database.
while IFS= read -r existing_id; do
  curl -fsS -X DELETE "${auth[@]}" "$CONTENT_URL/internal/argus/media/projects/$PROJECT_ID?media_id=$existing_id" >/dev/null
done < <(curl -fsS "${auth[@]}" "$CONTENT_URL/internal/argus/media/projects/$PROJECT_ID" | jq -r '.media[].id')

node -e "require('./apps/content/node_modules/sharp')({create:{width:1200,height:800,channels:3,background:'#336699'}}).png().toFile(process.argv[1])" "$tmp/hero.png"
printf '<svg xmlns="http://www.w3.org/2000/svg"/>' >"$tmp/unsafe.svg"
truncate -s 10485761 "$tmp/too-large.png"

unauthorized="$(curl -sS -o /dev/null -w '%{http_code}' "$CONTENT_URL/internal/argus/media/projects/$PROJECT_ID")"
[[ "$unauthorized" == 401 ]] || { echo "unauthenticated media request returned $unauthorized" >&2; exit 1; }
unsupported="$(curl -sS -o /dev/null -w '%{http_code}' -X POST "$CONTENT_URL/internal/argus/media/projects/$PROJECT_ID" \
  "${auth[@]}" -F "file=@$tmp/unsafe.svg;type=image/svg+xml" -F 'alt=Unsafe')"
[[ "$unsupported" == 400 ]] || { echo "unsupported media returned $unsupported" >&2; exit 1; }
oversized="$(curl -sS -o /dev/null -w '%{http_code}' -X POST "$CONTENT_URL/internal/argus/media/projects/$PROJECT_ID" \
  "${auth[@]}" -F "file=@$tmp/too-large.png;type=image/png" -F 'alt=Too large')"
[[ "$oversized" == 400 || "$oversized" == 413 ]] || { echo "oversized media returned $oversized" >&2; exit 1; }

private_response="$(curl -fsS -X POST "$CONTENT_URL/internal/argus/media/projects/$PROJECT_ID" \
  "${auth[@]}" -F "file=@$tmp/hero.png;type=image/png" -F 'alt=Private hero' -F 'caption=Not public')"
private_id="$(jq -er '.media.id' <<<"$private_response")"
private_url="$(jq -er '.media.url' <<<"$private_response")"
jq -e '.media.public_read == false and .media.width == 1200 and .media.height == 800 and .media.sizes.thumbnail.filename != null and .media.sizes.medium.filename != null' <<<"$private_response" >/dev/null
private_status="$(curl -sS -o /dev/null -w '%{http_code}' "$CONTENT_URL$private_url")"
[[ "$private_status" == 403 ]] || { echo "private media delivery returned $private_status" >&2; exit 1; }
curl -fsS -X PATCH "$CONTENT_URL/internal/argus/media/projects/$PROJECT_ID" \
  "${auth[@]}" -H 'Content-Type: application/json' \
  --data "{\"media_id\":\"$private_id\",\"alt\":\"Updated private hero\",\"caption\":\"Temporarily public\",\"public_read\":true}" \
  | jq -e '.media.alt == "Updated private hero" and .media.public_read == true' >/dev/null
curl -fsS "$CONTENT_URL$private_url" >/dev/null
curl -fsS -X PATCH "$CONTENT_URL/internal/argus/media/projects/$PROJECT_ID" \
  "${auth[@]}" -H 'Content-Type: application/json' \
  --data "{\"media_id\":\"$private_id\",\"alt\":\"Updated private hero\",\"caption\":\"Private again\",\"public_read\":false}" >/dev/null
revoked_status="$(curl -sS -o /dev/null -w '%{http_code}' "$CONTENT_URL$private_url")"
[[ "$revoked_status" == 403 ]] || { echo "revoked media delivery returned $revoked_status" >&2; exit 1; }

public_response="$(curl -fsS -X POST "$CONTENT_URL/internal/argus/media/projects/$PROJECT_ID" \
  "${auth[@]}" -F "file=@$tmp/hero.png;type=image/png" -F 'alt=Public hero' -F 'public_read=true')"
public_id="$(jq -er '.media.id' <<<"$public_response")"
public_url="$(jq -er '.media.url' <<<"$public_response")"
curl -fsS "$CONTENT_URL$public_url" >/dev/null
curl -fsS -X POST "$CONTENT_URL/internal/argus/project-sync" \
  -H "Authorization: Bearer $TOKEN" -H 'Content-Type: application/json' \
  --data "{\"organization_id\":\"$ORG_ID\",\"project_id\":\"$PROJECT_ID\",\"name\":\"Personal media acceptance\",\"client_id\":null,\"status\":\"archived\"}" >/dev/null
archived_status="$(curl -sS -o /dev/null -w '%{http_code}' "$CONTENT_URL$public_url")"
[[ "$archived_status" == 403 ]] || { echo "archived-project media delivery returned $archived_status" >&2; exit 1; }
curl -fsS -X POST "$CONTENT_URL/internal/argus/project-sync" \
  -H "Authorization: Bearer $TOKEN" -H 'Content-Type: application/json' \
  --data "{\"organization_id\":\"$ORG_ID\",\"project_id\":\"$PROJECT_ID\",\"name\":\"Personal media acceptance\",\"client_id\":null,\"status\":\"active\"}" >/dev/null
curl -fsS "$CONTENT_URL$public_url" >/dev/null

workspace="$(curl -fsS "${auth[@]}" "$CONTENT_URL/internal/argus/media/projects/$PROJECT_ID")"
jq -e --arg private "$private_id" --arg public "$public_id" '(.media | length) == 2 and any(.media[]; .id == $private and .alt == "Updated private hero" and .public_read == false) and any(.media[]; .id == $public and .public_read == true)' <<<"$workspace" >/dev/null

cross_delete="$(curl -sS -o /dev/null -w '%{http_code}' -X DELETE \
  -H "Authorization: Bearer $TOKEN" -H "X-Argus-Org-Id: $OTHER_ORG_ID" -H "X-Argus-User-Id: $USER_ID" \
  "$CONTENT_URL/internal/argus/media/projects/$PROJECT_ID?media_id=$public_id")"
[[ "$cross_delete" == 404 ]] || { echo "cross-organization media delete returned $cross_delete" >&2; exit 1; }

curl -fsS -X DELETE "${auth[@]}" "$CONTENT_URL/internal/argus/media/projects/$PROJECT_ID?media_id=$private_id" >/dev/null
curl -fsS -X DELETE "${auth[@]}" "$CONTENT_URL/internal/argus/media/projects/$PROJECT_ID?media_id=$public_id" >/dev/null
deleted_status="$(curl -sS -o /dev/null -w '%{http_code}' "$CONTENT_URL$public_url")"
[[ "$deleted_status" == 403 || "$deleted_status" == 404 ]] || { echo "deleted public media returned $deleted_status" >&2; exit 1; }
if [[ -n "${ARGUS_MEDIA_DIR:-}" ]]; then
  [[ ! -e "$ARGUS_MEDIA_DIR/${public_url##*/}" ]] || { echo 'deleted media file remains on disk' >&2; exit 1; }
fi
jq -e '.media | length == 0' <<<"$(curl -fsS "${auth[@]}" "$CONTENT_URL/internal/argus/media/projects/$PROJECT_ID")" >/dev/null

printf 'media library runtime acceptance passed\n'
