# Status Pages V1

Status Pages publish a deliberately limited view of project health. The internal operational model and the public response are separate by design.

## Default visibility

Every new Status Page starts as `INTERNAL`.

Switching a page to `PUBLIC` only makes the page shell and configured public components available. It does not publish internal Incidents automatically.

## Components

V1 supports public components backed by:

- Site;
- Service Catalog service.

The operator supplies a public display name. Internal resource IDs and internal names are not present in the unauthenticated DTO.

Component status is derived, not manually set:

- healthy/running/active -> `OPERATIONAL`;
- degraded -> `DEGRADED`;
- down/error/failed/stopped/unhealthy -> `OUTAGE`;
- anything else -> `UNKNOWN`.

## Incident publication

An internal Incident is public only after an operator creates a Status Page publication record with:

- explicit public title;
- explicit public message;
- published on/off flag.

The public title/message are separate from the internal Incident title, summary, timeline and notes.

Publication can be prepared while disabled and enabled later. Disabling publication preserves the public-safe text internally without returning it from the public endpoint.

## Public endpoint

`GET /public/status/:slug` is unauthenticated only for Status Pages whose visibility is `PUBLIC`.

Its response contains only:

- page name;
- coarse overall status;
- public component display names + coarse status;
- explicitly published Incident title/message + lifecycle timestamps/status;
- page updated timestamp.

It does **not** expose:

- organization/project IDs;
- resource IDs;
- server names;
- dependency graph paths;
- blast-radius snapshots;
- Incident notes/timeline data;
- Change Correlation data;
- user IDs;
- audit records;
- command output;
- internal Incident summary/title.

The Next web route `/status/[slug]` renders only this public DTO.

## Overall status

Overall status is derived from published active Incidents first, then components:

- published active CRITICAL Incident -> `MAJOR_OUTAGE`;
- published active MAJOR Incident -> `PARTIAL_OUTAGE`;
- published active MINOR Incident -> `DEGRADED`;
- otherwise any component OUTAGE -> `PARTIAL_OUTAGE`;
- otherwise any component DEGRADED -> `DEGRADED`;
- no usable signals -> `UNKNOWN`;
- otherwise -> `OPERATIONAL`.

Unpublished Incidents never affect public overall status.

## Slugs

Status Page slugs are globally unique, lowercase URL-safe values using letters, numbers and hyphens. This supports stable public paths such as `/status/argus-status`.

## Audit and activity

Mutations emit:

- `status_page.created`
- `status_page.updated`
- `status_page.deleted`
- `status_page.component_added`
- `status_page.component_removed`
- `status_page.incident_publication_updated`

## Non-goals

V1 does not implement:

- custom domains such as `status.example.com`;
- subscriber notifications;
- customer-only authenticated pages;
- SLA/uptime calculations;
- maintenance-window publication;
- automatic Incident publication;
- status-history charts;
- arbitrary HTML/Markdown on the public page.

## Next phase

Maintenance Windows and Runbooks can now integrate with Incidents and Status Pages without weakening the public/private boundary. External uptime redundancy should remain separate because Argus cannot reliably alert that Argus itself is fully down.

## Validation gate

Merge only after Rust workspace tests, `cargo fmt --all -- --check`, and the web TypeScript check pass, with no temporary workflow files left on the branch.
