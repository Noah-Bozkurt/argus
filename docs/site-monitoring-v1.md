# Site Monitoring V1

Site Monitoring V1 provides safe, manual project-level health checks before Argus introduces a background scheduler.

## Execution model

V1 deliberately has no `enabled` toggle or fake interval setting. An operator saves a monitor configuration and presses `Run check now`.

Automatic scheduling belongs in the later jobs/worker layer. Until that exists, the UI does not imply that Argus is continuously monitoring anything.

## Monitor configuration

Each Site can have one monitor configuration:

- target URL;
- robots.txt check on/off;
- sitemap.xml check on/off;
- timeout between 2 and 30 seconds.

The target hostname must belong to the Site through either its canonical URL or a Domain linked to that Site.

## SSRF boundary

Monitoring is an outbound network feature and is treated as a security boundary.

Before any HTTP request:

1. only `http` and `https` URLs are accepted;
2. credentials in URLs are rejected;
3. IP-literal targets are rejected;
4. only ports 80 and 443 are accepted;
5. DNS is resolved explicitly;
6. every resolved address must be public;
7. private, loopback, link-local, CGNAT, benchmark and documentation ranges are blocked;
8. the HTTP client is pinned to one validated resolved IP while retaining the original hostname for Host/TLS verification;
9. redirects are disabled.

IPv6 is accepted only from the global-unicast `2000::/3` range, excluding `2001:db8::/32` documentation addresses.

If a hostname resolves to both public and non-public addresses, the check is blocked. Argus does not pick the convenient public answer and ignore the unsafe one.

## Check result

Checks are immutable history records containing only technical metadata:

- overall status: HEALTHY / DEGRADED / DOWN / ERROR;
- target URL;
- resolved IP addresses;
- DNS result;
- HTTP status and latency;
- whether HTTPS/TLS successfully completed;
- optional robots.txt status;
- optional sitemap.xml status;
- bounded error code/message;
- operator and timestamp.

Response bodies are never stored.

## Health semantics

- main HTTP 2xx/3xx: healthy;
- main HTTP 4xx/5xx: down;
- DNS or main request failure: down;
- blocked/unsafe target: error;
- optional robots/sitemap failure or non-2xx/3xx: degraded when the main site is healthy.

The latest manual check updates the Site health status. A successful HTTPS request may positively mark the matching linked Domain TLS status as valid. V1 does not infer certificate expiry dates.

## Audit and events

- `site.monitor.updated`
- `site.check.completed`

Checks themselves are retained as immutable records; no edit/delete API is exposed.

## Non-goals

V1 does not implement:

- background scheduling;
- alerting;
- external uptime redundancy;
- browser/synthetic journeys;
- response body storage;
- redirect following;
- TLS-expiry parsing;
- DNS record mutation;
- public status publication.

## Next phase

The dependency graph should follow: Services, Sites, Domains and Servers now have stable identities, so impact analysis can be modeled before Incidents and Status Pages.

## Validation gate

Merge only after Rust workspace tests, rustfmt and the web TypeScript check pass, with no temporary workflow files left on the branch.
