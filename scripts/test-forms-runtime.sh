#!/usr/bin/env bash
set -Eeuo pipefail

CONTENT_URL="${ARGUS_TEST_CONTENT_URL:-http://127.0.0.1:3000}"
TOKEN="${ARGUS_CONTENT_SYNC_TOKEN:?ARGUS_CONTENT_SYNC_TOKEN is required}"
ORG_ID="00000000-0000-4000-8000-000000000001"
OTHER_ORG_ID="00000000-0000-4000-8000-000000000099"
USER_ID="00000000-0000-4000-8000-000000000002"
PROJECT_ID="00000000-0000-4000-8000-000000000005"
INTERNAL="$CONTENT_URL/internal/argus/forms/projects/$PROJECT_ID"
PUBLIC="$CONTENT_URL/public/projects/$PROJECT_ID/forms/contact"
auth=(-H "Authorization: Bearer $TOKEN" -H "X-Argus-Org-Id: $ORG_ID" -H "X-Argus-User-Id: $USER_ID")

curl -fsS -X POST "$CONTENT_URL/internal/argus/project-sync" \
  -H "Authorization: Bearer $TOKEN" -H 'Content-Type: application/json' \
  --data "{\"organization_id\":\"$ORG_ID\",\"project_id\":\"$PROJECT_ID\",\"name\":\"Personal forms acceptance\",\"client_id\":null,\"status\":\"active\"}" >/dev/null

unauthorized="$(curl -sS -o /dev/null -w '%{http_code}' "$INTERNAL")"
[[ "$unauthorized" == 401 ]] || { echo "unauthenticated forms request returned $unauthorized" >&2; exit 1; }

existing="$(curl -fsS "${auth[@]}" "$INTERNAL")"
if jq -e '.forms | length > 0' <<<"$existing" >/dev/null; then
  echo 'forms runtime acceptance requires a clean project scope' >&2
  exit 1
fi

form_response="$(curl -fsS -X POST "$INTERNAL" "${auth[@]}" -H 'Content-Type: application/json' --data '{
  "operation":"create_form","form":{"name":"Contact","slug":"contact","description":"Talk to us","success_message":"Thanks for reaching out.","published":false,
  "fields":[{"key":"email","label":"Email","type":"email","required":true,"options":[]},{"key":"topic","label":"Topic","type":"select","required":true,"options":["Support","Sales"]},{"key":"message","label":"Message","type":"textarea","required":true,"options":[]}]}}
')"
form_id="$(jq -er '.form.id' <<<"$form_response")"
jq -e '.form.status == "draft" and .form.fields[1].options == ["Support","Sales"]' <<<"$form_response" >/dev/null
[[ "$(curl -sS -o /dev/null -w '%{http_code}' "$PUBLIC")" == 404 ]]

curl -fsS -X POST "$INTERNAL" "${auth[@]}" -H 'Content-Type: application/json' \
  --data "{\"operation\":\"update_form_status\",\"form_id\":\"$form_id\",\"status\":\"published\"}" >/dev/null
public_form="$(curl -fsS "$PUBLIC")"
jq -e '.form.name == "Contact" and .form.fields[0].key == "email" and .form.fields[1].options == ["Support","Sales"] and (.form | has("id") | not)' <<<"$public_form" >/dev/null

invalid="$(curl -sS -o /dev/null -w '%{http_code}' -X POST "$PUBLIC" -H 'Content-Type: application/json' -H 'X-Forwarded-For: 198.51.100.10' \
  --data '{"values":{"email":"not-an-email","topic":"Support","message":"Hello"}}')"
[[ "$invalid" == 400 ]] || { echo "invalid form submission returned $invalid" >&2; exit 1; }

honeypot="$(curl -fsS -X POST "$PUBLIC" -H 'Content-Type: application/json' -H 'X-Forwarded-For: 198.51.100.10' \
  --data '{"_company":"bot","values":{"email":"bot@example.com","topic":"Support","message":"Spam"}}')"
jq -e '.accepted == true and (has("submission_id") | not)' <<<"$honeypot" >/dev/null

first="$(curl -fsS -X POST "$PUBLIC" -H 'Content-Type: application/json' -H 'X-Forwarded-For: 198.51.100.10' \
  --data '{"values":{"email":"person@example.com","topic":"Support","message":"=2+2"}}')"
first_id="$(jq -er '.submission_id' <<<"$first")"
jq -e '.accepted == true and .success_message == "Thanks for reaching out."' <<<"$first" >/dev/null
for index in $(seq 2 10); do
  curl -fsS -X POST "$PUBLIC" -H 'Content-Type: application/json' -H 'X-Forwarded-For: 198.51.100.10' \
    --data "{\"values\":{\"email\":\"person$index@example.com\",\"topic\":\"Sales\",\"message\":\"Message $index\"}}" >/dev/null
done
limited="$(curl -sS -o /dev/null -w '%{http_code}' -X POST "$PUBLIC" -H 'Content-Type: application/json' -H 'X-Forwarded-For: 198.51.100.10' \
  --data '{"values":{"email":"eleven@example.com","topic":"Support","message":"Too many"}}')"
[[ "$limited" == 429 ]] || { echo "eleventh form submission returned $limited" >&2; exit 1; }

workspace="$(curl -fsS "${auth[@]}" "$INTERNAL")"
jq -e --arg form "$form_id" --arg first "$first_id" '
  (.forms | any(.id == $form and .status == "published")) and
  (.submissions | length) == 10 and
  .submission_pagination.total_docs == 10 and .submission_pagination.page == 1 and
  (.submissions | any(.id == $first and .status == "new")) and
  (.submissions | all((has("source_hash") | not) and (has("rate_key") | not)))
' <<<"$workspace" >/dev/null

export_file="$(mktemp)"
export_headers="$(mktemp)"
curl -fsS -D "$export_headers" -o "$export_file" "${auth[@]}" "$INTERNAL/exports/$form_id"
grep -Fiq 'content-type: text/csv' "$export_headers"
grep -Fiq 'content-disposition: attachment; filename="contact-submissions.csv"' "$export_headers"
grep -Fq '"submission_id","status","submitted_at","email","topic","message"' "$export_file"
grep -Fq '"'"'"'=2+2"' "$export_file"
[[ "$(wc -l <"$export_file")" -eq 11 ]]
cross_export="$(curl -sS -o /dev/null -w '%{http_code}' -H "Authorization: Bearer $TOKEN" -H "X-Argus-Org-Id: $OTHER_ORG_ID" -H "X-Argus-User-Id: $USER_ID" "$INTERNAL/exports/$form_id")"
[[ "$cross_export" == 404 ]] || { echo "cross-organization form export returned $cross_export" >&2; exit 1; }

curl -fsS -X POST "$INTERNAL" "${auth[@]}" -H 'Content-Type: application/json' \
  --data "{\"operation\":\"update_submission_status\",\"submission_id\":\"$first_id\",\"status\":\"reviewed\"}" \
  | jq -e '.submission.status == "reviewed"' >/dev/null
curl -fsS -X POST "$INTERNAL" "${auth[@]}" -H 'Content-Type: application/json' \
  --data "{\"operation\":\"delete_submission\",\"submission_id\":\"$first_id\"}" >/dev/null
jq -e --arg first "$first_id" '.submission_pagination.total_docs == 9 and (.submissions | all(.id != $first))' <<<"$(curl -fsS "${auth[@]}" "$INTERNAL")" >/dev/null

cross_scope="$(curl -sS -o /dev/null -w '%{http_code}' \
  -H "Authorization: Bearer $TOKEN" -H "X-Argus-Org-Id: $OTHER_ORG_ID" -H "X-Argus-User-Id: $USER_ID" "$INTERNAL")"
[[ "$cross_scope" == 404 ]] || { echo "cross-organization forms request returned $cross_scope" >&2; exit 1; }

curl -fsS -X POST "$CONTENT_URL/internal/argus/project-sync" \
  -H "Authorization: Bearer $TOKEN" -H 'Content-Type: application/json' \
  --data "{\"organization_id\":\"$ORG_ID\",\"project_id\":\"$PROJECT_ID\",\"name\":\"Personal forms acceptance\",\"client_id\":null,\"status\":\"archived\"}" >/dev/null
[[ "$(curl -sS -o /dev/null -w '%{http_code}' "$PUBLIC")" == 404 ]]
[[ "$(curl -sS -o /dev/null -w '%{http_code}' -X POST "$PUBLIC" -H 'Content-Type: application/json' --data '{"values":{}}')" == 404 ]]

printf 'forms and submissions runtime acceptance passed\n'
