#!/usr/bin/env bash
set -Eeuo pipefail

CONTENT_URL="${ARGUS_TEST_CONTENT_URL:-http://127.0.0.1:3000}"
TOKEN="${ARGUS_CONTENT_SYNC_TOKEN:?ARGUS_CONTENT_SYNC_TOKEN is required}"
ORG_ID="00000000-0000-4000-8000-000000000001"
OTHER_ORG_ID="00000000-0000-4000-8000-000000000099"
USER_ID="00000000-0000-4000-8000-000000000002"
PROJECT_ID="00000000-0000-4000-8000-000000000003"

auth=(-H "Authorization: Bearer $TOKEN" -H "X-Argus-Org-Id: $ORG_ID" -H "X-Argus-User-Id: $USER_ID")

curl -fsS -X POST "$CONTENT_URL/internal/argus/project-sync" \
  -H "Authorization: Bearer $TOKEN" -H 'Content-Type: application/json' \
  --data "{\"organization_id\":\"$ORG_ID\",\"project_id\":\"$PROJECT_ID\",\"name\":\"Personal CMS acceptance\",\"client_id\":null,\"status\":\"active\"}" >/dev/null

unauthorized="$(curl -sS -o /dev/null -w '%{http_code}' "$CONTENT_URL/internal/argus/cms/projects/$PROJECT_ID")"
[[ "$unauthorized" == 401 ]] || { echo "unauthenticated CMS request returned $unauthorized" >&2; exit 1; }

cross_scope="$(curl -sS -o /dev/null -w '%{http_code}' \
  -H "Authorization: Bearer $TOKEN" -H "X-Argus-Org-Id: $OTHER_ORG_ID" -H "X-Argus-User-Id: $USER_ID" \
  "$CONTENT_URL/internal/argus/cms/projects/$PROJECT_ID")"
[[ "$cross_scope" == 404 ]] || { echo "cross-organization CMS request returned $cross_scope" >&2; exit 1; }

model_response="$(curl -fsS -X POST "$CONTENT_URL/internal/argus/cms/projects/$PROJECT_ID" \
  "${auth[@]}" -H 'Content-Type: application/json' \
  --data '{"operation":"create_model","model":{"name":"Articles","slug":"articles","description":"Runtime acceptance","public_read":true,"fields":[{"key":"title","label":"Title","type":"text","required":true},{"key":"body","label":"Body","type":"textarea","required":true}]}}')"
model_id="$(jq -er '.model.id' <<<"$model_response")"

draft_response="$(curl -fsS -X POST "$CONTENT_URL/internal/argus/cms/projects/$PROJECT_ID" \
  "${auth[@]}" -H 'Content-Type: application/json' \
  --data "{\"operation\":\"save_record\",\"model_id\":\"$model_id\",\"values\":{\"title\":\"Draft title\",\"body\":\"Draft body\"},\"publish\":false}")"
record_id="$(jq -er '.record.id' <<<"$draft_response")"
jq -e '.record.editorial_status == "draft"' <<<"$draft_response" >/dev/null

public_draft="$(curl -fsS "$CONTENT_URL/public/projects/$PROJECT_ID/content/articles")"
jq -e '.records | length == 0' <<<"$public_draft" >/dev/null

published_response="$(curl -fsS -X POST "$CONTENT_URL/internal/argus/cms/projects/$PROJECT_ID" \
  "${auth[@]}" -H 'Content-Type: application/json' \
  --data "{\"operation\":\"save_record\",\"model_id\":\"$model_id\",\"record_id\":\"$record_id\",\"values\":{\"title\":\"Published title\",\"body\":\"Published body\"},\"publish\":true}")"
jq -e '.record.editorial_status == "published"' <<<"$published_response" >/dev/null

public_published="$(curl -fsS "$CONTENT_URL/public/projects/$PROJECT_ID/content/articles")"
jq -e '(.records | length) == 1 and .records[0].values.title == "Published title" and .records[0].values.body == "Published body"' <<<"$public_published" >/dev/null

draft_target="$(curl -fsS -X POST "$CONTENT_URL/internal/argus/cms/projects/$PROJECT_ID" \
  "${auth[@]}" -H 'Content-Type: application/json' \
  --data "{\"operation\":\"save_record\",\"model_id\":\"$model_id\",\"values\":{\"title\":\"Private draft\",\"body\":\"Must not expand\"},\"publish\":false}")"
draft_target_id="$(jq -er '.record.id' <<<"$draft_target")"

feature_model="$(curl -fsS -X POST "$CONTENT_URL/internal/argus/cms/projects/$PROJECT_ID" \
  "${auth[@]}" -H 'Content-Type: application/json' \
  --data "{\"operation\":\"create_model\",\"model\":{\"name\":\"Features\",\"slug\":\"features\",\"public_read\":true,\"fields\":[{\"key\":\"title\",\"label\":\"Title\",\"type\":\"text\",\"required\":true},{\"key\":\"article\",\"label\":\"Article\",\"type\":\"relationship\",\"required\":true,\"target_model_id\":\"$model_id\",\"has_many\":false}]}}")"
feature_model_id="$(jq -er '.model.id' <<<"$feature_model")"
jq -e --arg target "$model_id" '.model.fields[1].target_model_id == $target and .model.fields[1].has_many == false' <<<"$feature_model" >/dev/null

feature="$(curl -fsS -X POST "$CONTENT_URL/internal/argus/cms/projects/$PROJECT_ID" \
  "${auth[@]}" -H 'Content-Type: application/json' \
  --data "{\"operation\":\"save_record\",\"model_id\":\"$feature_model_id\",\"values\":{\"title\":\"Featured\"},\"relationships\":{\"article\":[\"$record_id\"]},\"publish\":true}")"
feature_id="$(jq -er '.record.id' <<<"$feature")"
jq -e '(.records[0] | has("relationships")) | not' <<<"$(curl -fsS "$CONTENT_URL/public/projects/$PROJECT_ID/content/features")" >/dev/null
jq -e --arg article "$record_id" '.records[0].relationships.article[0].id == $article and .records[0].relationships.article[0].model == "articles" and .records[0].relationships.article[0].values.title == "Published title"' \
  <<<"$(curl -fsS "$CONTENT_URL/public/projects/$PROJECT_ID/content/features?expand=relationships")" >/dev/null

curl -fsS -X POST "$CONTENT_URL/internal/argus/cms/projects/$PROJECT_ID" "${auth[@]}" -H 'Content-Type: application/json' \
  --data "{\"operation\":\"save_record\",\"model_id\":\"$feature_model_id\",\"values\":{\"title\":\"Draft target feature\"},\"relationships\":{\"article\":[\"$draft_target_id\"]},\"publish\":true}" >/dev/null
jq -e '.records | map(select(.values.title == "Draft target feature"))[0].relationships == {}' \
  <<<"$(curl -fsS "$CONTENT_URL/public/projects/$PROJECT_ID/content/features?expand=relationships")" >/dev/null

missing_relation_status="$(curl -sS -o /dev/null -w '%{http_code}' -X POST "$CONTENT_URL/internal/argus/cms/projects/$PROJECT_ID" \
  "${auth[@]}" -H 'Content-Type: application/json' \
  --data "{\"operation\":\"save_record\",\"model_id\":\"$feature_model_id\",\"values\":{\"title\":\"Missing\"},\"relationships\":{},\"publish\":false}")"
[[ "$missing_relation_status" == 400 ]] || { echo "missing required relationship returned $missing_relation_status" >&2; exit 1; }

component_response="$(curl -fsS -X POST "$CONTENT_URL/internal/argus/cms/projects/$PROJECT_ID" \
  "${auth[@]}" -H 'Content-Type: application/json' \
  --data '{"operation":"create_model","model":{"name":"Hero","slug":"hero","content_role":"component","public_read":true,"fields":[{"key":"heading","label":"Heading","type":"text","required":true},{"key":"body","label":"Body","type":"textarea","required":false}]}}')"
component_id="$(jq -er '.model.id' <<<"$component_response")"
jq -e '.model.content_role == "component" and .model.public_read == false' <<<"$component_response" >/dev/null

page_response="$(curl -fsS -X POST "$CONTENT_URL/internal/argus/cms/projects/$PROJECT_ID" \
  "${auth[@]}" -H 'Content-Type: application/json' \
  --data "{\"operation\":\"create_model\",\"model\":{\"name\":\"Home page\",\"slug\":\"home_page\",\"content_role\":\"page\",\"public_read\":true,\"allowed_component_ids\":[\"$component_id\"],\"fields\":[{\"key\":\"title\",\"label\":\"Title\",\"type\":\"text\",\"required\":true}]}}")"
page_id="$(jq -er '.model.id' <<<"$page_response")"

block_id="00000000-0000-4000-8000-000000000020"
page_draft="$(curl -fsS -X POST "$CONTENT_URL/internal/argus/cms/projects/$PROJECT_ID" \
  "${auth[@]}" -H 'Content-Type: application/json' \
  --data "{\"operation\":\"save_record\",\"model_id\":\"$page_id\",\"values\":{\"title\":\"Home\"},\"layout\":[{\"id\":\"$block_id\",\"component\":\"hero\",\"values\":{\"heading\":\"Welcome\",\"body\":\"Built with blocks\"}}],\"publish\":false}")"
page_record_id="$(jq -er '.record.id' <<<"$page_draft")"
jq -e '.record.editorial_status == "draft" and .record.layout[0].component == "hero"' <<<"$page_draft" >/dev/null
jq -e '.records | length == 0' <<<"$(curl -fsS "$CONTENT_URL/public/projects/$PROJECT_ID/content/home_page")" >/dev/null

invalid_block_status="$(curl -sS -o /dev/null -w '%{http_code}' -X POST "$CONTENT_URL/internal/argus/cms/projects/$PROJECT_ID" \
  "${auth[@]}" -H 'Content-Type: application/json' \
  --data "{\"operation\":\"save_record\",\"model_id\":\"$page_id\",\"values\":{\"title\":\"Invalid\"},\"layout\":[{\"id\":\"$block_id\",\"component\":\"not_allowed\",\"values\":{}}],\"publish\":false}")"
[[ "$invalid_block_status" == 409 ]] || { echo "disallowed page component returned $invalid_block_status" >&2; exit 1; }

curl -fsS -X POST "$CONTENT_URL/internal/argus/cms/projects/$PROJECT_ID" \
  "${auth[@]}" -H 'Content-Type: application/json' \
  --data "{\"operation\":\"save_record\",\"model_id\":\"$page_id\",\"record_id\":\"$page_record_id\",\"values\":{\"title\":\"Home\"},\"layout\":[{\"id\":\"$block_id\",\"component\":\"hero\",\"values\":{\"heading\":\"Welcome\",\"body\":\"Built with blocks\"}}],\"publish\":true}" >/dev/null
public_page="$(curl -fsS "$CONTENT_URL/public/projects/$PROJECT_ID/content/home_page")"
jq -e '(.records | length) == 1 and .records[0].values.title == "Home" and .records[0].layout[0].component == "hero" and .records[0].layout[0].values.heading == "Welcome"' <<<"$public_page" >/dev/null

workspace="$(curl -fsS "${auth[@]}" "$CONTENT_URL/internal/argus/cms/projects/$PROJECT_ID")"
jq -e --arg model "$model_id" --arg record "$record_id" '
  (.models | any(.id == $model and .slug == "articles" and .public_read == true)) and
  (.records | any(.id == $record and .model_id == $model and .editorial_status == "published"))
' <<<"$workspace" >/dev/null
jq -e --arg source "$feature_id" --arg target "$record_id" '.relations | any(.source_record_id == $source and .target_record_id == $target and .field_key == "article")' <<<"$workspace" >/dev/null

printf 'native CMS runtime acceptance passed\n'
