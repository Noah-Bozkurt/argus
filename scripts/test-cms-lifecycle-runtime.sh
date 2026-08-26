#!/usr/bin/env bash
set -Eeuo pipefail

CONTENT_URL="${ARGUS_TEST_CONTENT_URL:-http://127.0.0.1:3000}"
TOKEN="${ARGUS_CONTENT_SYNC_TOKEN:?ARGUS_CONTENT_SYNC_TOKEN is required}"
ORG_ID="${ARGUS_ORG_ID:-00000000-0000-4000-8000-000000000001}"
USER_ID="${ARGUS_USER_ID:-00000000-0000-4000-8000-000000000002}"
PROJECT_ID="00000000-0000-4000-8000-000000000003"
auth=(-H "Authorization: Bearer $TOKEN" -H "X-Argus-Org-Id: $ORG_ID" -H "X-Argus-User-Id: $USER_ID")

model="$(curl -fsS -X POST "$CONTENT_URL/internal/argus/cms/projects/$PROJECT_ID" \
  "${auth[@]}" -H 'Content-Type: application/json' \
  --data '{"operation":"create_model","model":{"name":"Lifecycle test","slug":"lifecycle_test","description":"Initial","public_read":true,"content_role":"collection","fields":[{"key":"title","label":"Title","type":"text","required":true,"target_model_id":null,"has_many":false}]}}')"
model_id="$(jq -er '.model.id' <<<"$model")"
jq -e '.model.schema_version == 1' <<<"$model" >/dev/null

metadata_update="$(curl -fsS -X POST "$CONTENT_URL/internal/argus/cms/projects/$PROJECT_ID" \
  "${auth[@]}" -H 'Content-Type: application/json' \
  --data "{\"operation\":\"update_model\",\"model_id\":\"$model_id\",\"model\":{\"name\":\"Lifecycle test renamed\",\"slug\":\"lifecycle_test\",\"description\":\"Metadata only\",\"public_read\":true,\"content_role\":\"collection\",\"fields\":[{\"key\":\"title\",\"label\":\"Title\",\"type\":\"text\",\"required\":true,\"target_model_id\":null,\"has_many\":false}]}}")"
jq -e '.model.schema_version == 1 and .model.name == "Lifecycle test renamed"' <<<"$metadata_update" >/dev/null

schema_update="$(curl -fsS -X POST "$CONTENT_URL/internal/argus/cms/projects/$PROJECT_ID" \
  "${auth[@]}" -H 'Content-Type: application/json' \
  --data "{\"operation\":\"update_model\",\"model_id\":\"$model_id\",\"model\":{\"name\":\"Lifecycle test renamed\",\"slug\":\"lifecycle_test\",\"description\":\"Schema update\",\"public_read\":true,\"content_role\":\"collection\",\"fields\":[{\"key\":\"title\",\"label\":\"Title\",\"type\":\"text\",\"required\":true,\"target_model_id\":null,\"has_many\":false},{\"key\":\"summary\",\"label\":\"Summary\",\"type\":\"textarea\",\"required\":false,\"target_model_id\":null,\"has_many\":false}]}}")"
jq -e '.model.schema_version == 2 and (.model.fields | length) == 2' <<<"$schema_update" >/dev/null

record="$(curl -fsS -X POST "$CONTENT_URL/internal/argus/cms/projects/$PROJECT_ID" \
  "${auth[@]}" -H 'Content-Type: application/json' \
  --data "{\"operation\":\"save_record\",\"model_id\":\"$model_id\",\"values\":{\"title\":\"Lifecycle record\",\"summary\":\"Published\"},\"relationships\":{},\"layout\":[],\"publish\":true}")"
record_id="$(jq -er '.record.id' <<<"$record")"
jq -e '.records | length == 1' <<<"$(curl -fsS "$CONTENT_URL/public/projects/$PROJECT_ID/content/lifecycle_test")" >/dev/null

curl -fsS -X POST "$CONTENT_URL/internal/argus/cms/projects/$PROJECT_ID" "${auth[@]}" -H 'Content-Type: application/json' \
  --data "{\"operation\":\"set_record_status\",\"record_id\":\"$record_id\",\"status\":\"archived\"}" >/dev/null
jq -e '.records | length == 0' <<<"$(curl -fsS "$CONTENT_URL/public/projects/$PROJECT_ID/content/lifecycle_test")" >/dev/null

curl -fsS -X POST "$CONTENT_URL/internal/argus/cms/projects/$PROJECT_ID" "${auth[@]}" -H 'Content-Type: application/json' \
  --data "{\"operation\":\"set_record_status\",\"record_id\":\"$record_id\",\"status\":\"active\"}" >/dev/null
jq -e '.records | length == 1' <<<"$(curl -fsS "$CONTENT_URL/public/projects/$PROJECT_ID/content/lifecycle_test")" >/dev/null

not_empty="$(curl -sS -o /dev/null -w '%{http_code}' -X POST "$CONTENT_URL/internal/argus/cms/projects/$PROJECT_ID" "${auth[@]}" -H 'Content-Type: application/json' \
  --data "{\"operation\":\"delete_model\",\"model_id\":\"$model_id\"}")"
[[ "$not_empty" == 409 ]] || { echo "non-empty model deletion returned $not_empty" >&2; exit 1; }

record_delete="$(curl -sS -o /dev/null -w '%{http_code}' -X POST "$CONTENT_URL/internal/argus/cms/projects/$PROJECT_ID" "${auth[@]}" -H 'Content-Type: application/json' \
  --data "{\"operation\":\"delete_record\",\"record_id\":\"$record_id\"}")"
[[ "$record_delete" == 204 ]] || { echo "record deletion returned $record_delete" >&2; exit 1; }

model_delete="$(curl -sS -o /dev/null -w '%{http_code}' -X POST "$CONTENT_URL/internal/argus/cms/projects/$PROJECT_ID" "${auth[@]}" -H 'Content-Type: application/json' \
  --data "{\"operation\":\"delete_model\",\"model_id\":\"$model_id\"}")"
[[ "$model_delete" == 204 ]] || { echo "empty model deletion returned $model_delete" >&2; exit 1; }

printf 'CMS lifecycle runtime acceptance passed\n'
