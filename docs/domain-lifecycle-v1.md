# Domain Lifecycle V1

Argus now tracks domain expiration and TLS lifecycle state as derived operational data instead of leaving those fields as static inventory metadata.

## Evaluation

Each organization receives a `domains.lifecycle_evaluate` background schedule that runs every six hours through the existing PostgreSQL Jobs / Worker layer. Existing organizations are backfilled by migration `0020_domain_lifecycle.sql`.

A project can also trigger an immediate lifecycle evaluation from the Sites & Domains workspace.

## Expiration states

Expiration is derived from the manually stored domain `expires_at` value:

- `UNKNOWN`: no expiration date is known;
- `OK`: more than 30 days remain;
- `WARNING`: 30 days or fewer remain;
- `CRITICAL`: 7 days or fewer remain;
- `EXPIRED`: the expiration timestamp has passed.

The lifecycle record also stores `days_until_expiry`.

V1 does not query registrar APIs and does not renew domains. The date remains operator/provider inventory until provider integrations are added later.

## TLS states

TLS lifecycle is derived from recent Site Monitoring observations for the exact domain hostname.

Argus examines up to the latest 20 checks for a linked Site and uses the newest HTTPS check whose URL host exactly matches the domain. This avoids incorrectly applying a Site's primary-domain TLS result to another domain attached to the same Site.

States:

- `VALID`: a matching HTTPS observation from the past 24 hours reported valid TLS;
- `FAILED`: a fresh matching HTTPS observation did not report valid TLS;
- `STALE`: the latest matching HTTPS observation is older than 24 hours;
- `UNKNOWN`: no matching HTTPS observation exists.

The derived TLS state is also copied into the existing Domain inventory `tls_status` field so Sites & Domains displays current derived information rather than an indefinitely cached historical `VALID` value.

## Overall state

- `CRITICAL`: domain is expired/within seven days, or fresh TLS validation failed;
- `ATTENTION`: expiration is warning/unknown, or TLS is stale/unknown;
- `OK`: known-safe expiration and fresh valid TLS;
- `UNKNOWN`: both expiration and TLS are unknown.

## Change events

Lifecycle state is persisted in `domain_lifecycle_states`. Every material state change emits `domain.lifecycle.changed` as a normal project-scoped domain event containing the previous and current derived values.

That means existing Notification rules can match `domain.lifecycle.changed` without a separate alerting implementation.

## Safety and scope

Domain Lifecycle V1 is observation-only. It does not:

- renew registrations;
- change registrar settings;
- create or edit DNS records;
- provision or rotate certificates;
- call Cloudflare APIs;
- infer expiration dates from untrusted public WHOIS data.

Provider provisioning and Cloudflare automation remain separate future capabilities.
