# Operations

This document describes the current operational model in Argus. Installation and host lifecycle commands are covered in [Installation](installation.md); security-sensitive mutation and rollback details are covered in [Security & Recovery](security-and-recovery.md).

## Supported control-plane deployment

The current supported host target is deliberately narrow:

- Ubuntu or Debian;
- amd64;
- one control-plane host;
- Docker Compose for the control plane;
- native systemd Agent and Helper services;
- Caddy on ports 80/443;
- separate main and content DNS names.

Install through:

```bash
curl -fsSL https://install.noahbozkurt.nl/install | sudo bash
```

Do not use the underlying Cloudflare Pages hostname as the documented product URL. `install.noahbozkurt.nl` is the stable installer entry point.

## Runtime layout

The control plane contains:

- PostgreSQL 16;
- Argus Control API;
- Argus Worker;
- Argus Web;
- Payload Content;
- Caddy.

These run with Docker Compose. The managed-node Agent and privileged Helper run directly under systemd.

Default host paths:

```text
/opt/argus/       Compose/runtime files
/etc/argus/       host configuration and registry credentials
/var/lib/argus/   persistent state and backups
/var/log/argus/   installer logs
/usr/local/bin/   argus-agent, argus-helper, argusctl
```

Only Caddy is publicly exposed. The Control API has a host-loopback path for the local Agent. PostgreSQL and the application containers are not published directly.

## Host lifecycle

### Local status

```bash
argusctl status
```

Shows native Agent/Helper service state and the enrolled Agent/Server/control-plane identity.

### Local health

```bash
argusctl health
```

Checks Agent, Helper, Helper socket reachability and local system collection.

### Control-plane connectivity

```bash
argusctl connection
```

Performs an authenticated Agent identity request against the configured control plane.

### Full installed-system smoke check

```bash
sudo argusctl smoke
```

Use this after installation, reboot and update. It performs broader installed-system validation than the local health command.

### System snapshot

```bash
argusctl system info
```

Returns a local JSON snapshot including hostname, OS/kernel, architecture, CPU/RAM/disk usage, load and uptime.

### Version

```bash
argusctl version
```

## Installation behavior

The installer is rerunnable and preserves generated IDs, secrets, data and the installed immutable revision when an existing installation is detected.

A rerun is not an update request. Use:

```bash
sudo argusctl update --version main
```

for a normal update.

The installer stores the validated private-registry login in `/etc/argus/registry.env` with mode `0600`. The token is not written to `/opt/argus/.env`.

To rotate the credential:

```bash
sudo argusctl registry-login
```

## Coordinated image releases

Argus publishes five coordinated custom images:

- `argus-web`;
- `argus-control-api`;
- `argus-worker`;
- `argus-content`;
- `argus-host-tools`.

Normal CI must succeed on `main` before release publication starts. The release workflow builds/publishes the full SHA-tagged set, verifies that all required immutable artifacts are remotely available and only then promotes the `main` pointers.

`main` is therefore a discovery/update alias. Install/update resolves it to a full Git SHA and persists that SHA as the installed `ARGUS_VERSION`.

## Transactional self-update

The control plane is updated locally with:

```bash
sudo argusctl update --version main
```

or a specific published revision:

```bash
sudo argusctl update --version <40-character-git-sha>
```

The updater is intentionally not implemented as a normal remote Agent command because it must stop/replace parts of the same control plane that would otherwise coordinate the operation.

At a high level the update flow:

1. validates the currently installed coordinated revision;
2. resolves and pre-pulls the complete target revision;
3. prepares the target host-tools/deployment bundle before downtime;
4. checks snapshot space;
5. seals the current deployment/native file set for rollback;
6. stops writers at the transaction boundary;
7. captures and validates a PostgreSQL backup when required by the update phase;
8. installs the target files and records the durable target-start boundary;
9. starts the target services;
10. requires service health and `argusctl smoke` before success.

If a mutation-stage failure occurs, the updater uses the stored transaction state to restore the previous coordinated revision. Recovery behavior distinguishes failures before target startup from failures after target startup so the database is not unnecessarily replaced.

Interrupted-update recovery is integrated into the native Helper startup path. See [Security & Recovery](security-and-recovery.md) for the exact transaction boundaries and rollback guarantees.

## Uninstall

Interactive:

```bash
sudo argusctl uninstall
```

Automation:

```bash
sudo argusctl uninstall --yes
```

Persistent state is retained conservatively unless purge is explicitly requested:

```bash
sudo argusctl uninstall --purge-data
```

`--purge-data` is destructive and should only be used when the Argus data/backups on that host are intentionally disposable.

## Managed server model

A server becomes controllable through an authenticated Argus Agent, not merely by creating a hostname record in the UI.

Enrollment flow:

1. project Infrastructure -> Add server;
2. generate the 15-minute single-use setup code;
3. run the public installer on the new server;
4. choose managed-server mode;
5. paste the setup code;
6. let the installer enroll/start Agent and Helper.

The Agent reports heartbeats and snapshots, polls typed commands, calls the local privileged Helper and returns command results.

## Agent/Helper trust boundary

The Agent is the network-facing managed-node component. It is not root.

The Helper is root and listens only on a local Unix socket. It supports fixed, validated operation classes. Argus does not construct arbitrary shell commands from operator input for normal infrastructure management.

Argus-owned control-plane containers are marked with `com.argus.protected=true`. Managed Docker/Compose operations reject stop/restart actions against protected containers so the normal infrastructure surface cannot turn off its own control plane.

## Systemd services

Managed systemd services support typed start, stop and restart operations. Service names must pass validation and Helper allowlisting.

The Agent can also collect relevant service/journald diagnostics. Host-side troubleshooting for Argus itself starts with:

```bash
systemctl status argus-agent.service
systemctl status argus-helper.service
journalctl -u argus-agent.service
journalctl -u argus-helper.service
```

## Package maintenance

APT-based managed hosts expose update/reboot information and support typed maintenance operations such as metadata refresh and supported upgrade flows.

Disruptive operations are maintenance-gated where required by Control API policy. UI button state is not the enforcement boundary.

## Docker and Compose

When Docker is available, Agents report container inventory and expose typed start/stop/restart actions through the Helper.

Compose projects are represented as stack resources. The Helper resolves/validates known Compose project configuration before executing stack actions; the API does not simply accept arbitrary Docker/Compose CLI argument strings.

## Monitoring

Site monitoring can record operational checks including DNS/HTTP reachability, response latency, TLS state and selected website metadata.

Monitoring requests include SSRF protections instead of blindly requesting arbitrary internal addresses.

Checks are persisted and feed site state, incident automation and related operational views.

## Schedules and jobs

Recurring operational work is persisted in PostgreSQL. Workers claim due jobs and record attempts/results.

The same job foundation is used by capabilities such as:

- site monitoring;
- incident evaluation;
- notification materialization;
- desired-state reconciliation;
- domain lifecycle evaluation;
- Payload project synchronization.

The global Jobs view exposes this background activity instead of hiding it inside long-running web processes.

## Incidents

Incidents are project-scoped and carry severity, status, affected resources, timeline and resolution context.

Monitoring automation can evaluate/create incidents after configured failure conditions rather than turning every failed request into a new incident.

Dependency and change-correlation information can be used as operational context. Correlation is evidence for investigation, not automatic root-cause proof.

## Notifications

Project/domain events can materialize operator notifications through background jobs. The global Notifications view is the current operator-facing notification surface.

External notification providers such as email/chat/webhooks should only be documented as available once a concrete provider implementation exists.

## Status pages

Projects can expose public status information based on selected operational components/incidents. Status pages are a presentation layer over normal Argus operational state and intentionally render outside the authenticated control-panel shell.

## Desired state and drift

Argus stores desired security state separately from authenticated Agent observations.

Current automatic enforcement remains deliberately narrow. Firewall `must be active` is the supported reconciliation primitive because it has explicit preflight and rollback behavior. Other observed fields should remain monitor-only until equivalent safety semantics exist.

## Operational non-goals / current limits

Argus currently does not claim to provide:

- arbitrary remote shell as its normal management model;
- full metrics/time-series observability;
- generalized configuration management for every Linux setting;
- automatic cloud/provider provisioning;
- automatic Cloudflare/DNS mutation;
- HA/multi-node control-plane operation;
- arm64 installation support;
- arbitrary point-in-time database rollback after a successful update;
- completely autonomous remediation for arbitrary incidents.

See [Roadmap](roadmap.md) for current priorities rather than historical first-test phases.
