# Sites & Domains V1

Sites and domains are first-class project inventory. Both work for personal and client projects; neither requires a Client.

## Sites

A Site may link to:

- one Service Catalog service;
- one repository;
- one environment;
- zero or more domains.

A Site records its name, description, framework, canonical URL, lifecycle and health status. Health starts as `UNKNOWN`; it is not manually editable because the monitoring phase should derive it.

If a linked Service already has a repository or environment, the Site inherits those values when they are omitted. Supplying a conflicting repository or environment is rejected.

## Domains

A Domain belongs to a Project and may optionally link to a Site. This means Argus can inventory a domain before the site exists, or keep domains used for APIs, mail or future launches without inventing a Site.

V1 records:

- hostname;
- optional Site;
- registrar;
- DNS provider;
- routing mode;
- primary-domain flag;
- expiration date;
- TLS status.

Hostnames are normalized to lowercase without a trailing dot and validated as DNS-style ASCII hostnames. Internationalized domains should be stored in their ASCII/Punycode form.

## Routing modes

V1 models three routing modes:

- `DIRECT`
- `CLOUDFLARE_PROXY`
- `CLOUDFLARE_TUNNEL`

These are inventory only. This phase does **not** create DNS records, toggle the Cloudflare proxy, create tunnels or provision certificates.

That distinction is important because health checks differ by routing mode. A proxied domain should not be compared directly with an origin IP, and a tunnel does not imply that ports 80/443 are publicly reachable on the origin.

## Primary domains

A primary domain must link to a Site. A Site can have at most one primary domain. Other linked domains remain aliases/secondary domains.

## Deletion safety

A Site cannot be deleted while Domains still point to it. The operator must explicitly unlink or remove those domains first. Deleting a Domain does not delete its Site.

## Activity and audit

Mutations emit project-scoped domain events and audit events:

- `site.created`
- `site.updated`
- `site.deleted`
- `domain.created`
- `domain.updated`
- `domain.deleted`

## Non-goals

V1 does not implement:

- DNS record management;
- registrar APIs;
- Cloudflare provisioning;
- automatic TLS issuance;
- site deployment execution;
- uptime history;
- synthetic browser checks;
- SEO crawling.

## Next phase

Site Monitoring V1 should build on this inventory with safe outbound HTTP/DNS/TLS checks, immutable check history and manual `Run check now` execution before adding a background scheduler.

## Validation gate

Merge only after Rust workspace tests, `cargo fmt --all -- --check`, and the web TypeScript check pass. The final branch must not contain temporary branch-specific workflow files.
