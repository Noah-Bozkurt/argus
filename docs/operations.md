# Operations

This document describes the operational capabilities that currently exist in Argus. Security-sensitive details and recovery guarantees are expanded in [Security & Recovery](security-and-recovery.md).

## First-test server deployment

The first supported installation target is a clean Ubuntu/Debian-class **amd64** server. It is a test topology, not a claim of production readiness.

The control plane runs through Docker Compose:

- official PostgreSQL 16 image;
- custom Argus Control API image;
- custom Argus Worker image;
- custom standalone Next.js Web image;
- custom standalone Payload Content image;
- official Caddy image.

The managed-node Agent and privileged Helper run natively through systemd. `argusctl` is installed as a native diagnostic/lifecycle binary.

### Network layout

For the direct-DNS first test, create DNS records for two hostnames pointing at the server:

- the main Argus hostname, for example `argus.example.com`;
- the content hostname, for example `content.argus.example.com`.

Inbound TCP 80/443 must reach Caddy. UDP 443 is also exposed for HTTP/3 but is not required for basic HTTPS operation. PostgreSQL, Web and Payload are not published directly. Control API port 8080 is bound only to host loopback so the native local Agent can use it.

The primary hostname routes only `/agent/*`, `/enrollment/complete` and the explicit public status API directly to the Control API. Other operator traffic goes to Web and is protected by temporary first-test Caddy basic authentication. The content hostname exposes only `/public/*` and Payload's access-checked `/api/media/file/*` handler without that outer authentication; Payload admin/private routes remain protected. Each media asset still requires its explicit public-delivery flag.

### Installer inputs

The root `install.sh` is the supported first-test path. The minimum operator-provided values are:

```text
ARGUS_DOMAIN=argus.example.com
ARGUS_CONTENT_DOMAIN=content.argus.example.com   # defaults to content.<main-domain>
ARGUS_REGISTRY_USERNAME=<private registry user>
ARGUS_REGISTRY_TOKEN=<credential able to read the private Argus images>
```

`ARGUS_VERSION` defaults to `main`, but `main` is only a discovery alias. The installer reads the artifact's `org.opencontainers.image.revision` label, verifies it is a full commit SHA and persists that immutable SHA as the installed `ARGUS_VERSION`. Compose therefore runs commit-addressed Argus images even when installation started from `main`.

Run the installer from an authenticated checkout/download of this private repository:

```bash
sudo -E ./install.sh
```

The installer:

1. validates Ubuntu/Debian + amd64 and detects conflicting existing container setups;
2. installs/verifies Docker Engine and Compose;
3. resolves the requested image tag to an immutable tested commit revision;
4. pulls the matching `argus-host-tools` artifact image and installs Agent/Helper/CLI plus version-matched deployment templates;
5. creates persistent high-entropy internal credentials and bootstrap IDs;
6. starts PostgreSQL/Control API/Worker/Web/Payload/Caddy through Compose;
7. bootstraps an initial organization, operator identity and an `Argus Control Plane` infrastructure Project without any Client requirement;
8. starts Helper and enrolls the local Agent through the real one-time enrollment API;
9. removes the enrollment token from persistent Agent configuration after enrollment;
10. verifies Compose health, systemd services, Control API loopback health and both HTTPS hostnames.

Generated configuration lives primarily under `/opt/argus`, `/etc/argus` and `/var/lib/argus`. The Compose `.env` is root-readable only and contains the first-test credentials required to reproduce/restart the deployment.

Rerunning the installer preserves existing generated IDs/secrets and the installed revision. It is not an update mechanism. If a legacy first-test install still stores a mutable version such as `main`, the rerun recovers the revision from the currently running verified image before proceeding. A different requested version on an existing install is rejected/ignored in favor of the dedicated transactional update path.

The disposable test reset path is intentionally explicit:

```bash
ARGUS_CONFIRM_RESET=DELETE-ARGUS-FIRST-TEST-DATA \
  sudo -E ./scripts/reset-first-test.sh
```

That removes Argus data/volumes, Agent identity and test backups. It leaves Docker installed.

### First-server smoke verification

After installation or reboot, run:

```bash
sudo argusctl smoke
```

The smoke test is embedded into the installed `argusctl` binary. It verifies the version-matched deployment without requiring a source checkout. Checks include:

- root-only generated-file permissions;
- health/running state for the Compose control plane and native Agent/Helper;
- the Helper Unix-socket permission boundary;
- Control API loopback-only exposure and absence of a host-exposed PostgreSQL port;
- authenticated Agent identity and a fresh heartbeat;
- project-centric bootstrap records with no required Client;
- default background schedules and successful Payload Project synchronization;
- authenticated Web/Payload HTTPS health;
- unauthenticated public-status routing.

`ARGUS_SMOKE_SKIP_PUBLIC_HTTPS=1` skips only the external HTTPS portion for DNS/network diagnosis; internal control-plane checks still run.

### Image publication gate

Custom images are not published by pull-request CI. The image workflow runs only after the repository's normal `CI` workflow completes successfully on `main`, checks out the exact tested commit and publishes the five Argus images to GHCR using both `main` and full-commit-SHA tags.

After all five builds/pushes succeed, a final registry-verification job authenticates to GHCR and remotely inspects both expected tags for every image. The publication workflow is not green until those remote artifacts are resolvable.

PR CI separately proves the source is server-test-ready by checking locked Rust tests, TypeScript, deployment lifecycle script syntax, Compose/Caddy configuration, a Control API boot against an empty PostgreSQL 16 database, and production Web/Payload builds.

### Transactional self-update V1

For the single-server test deployment, `argusctl` exposes a local root-only transactional update path:

```bash
export ARGUS_REGISTRY_USERNAME=<private registry user>
export ARGUS_REGISTRY_TOKEN=<read-only package credential>
sudo -E argusctl update --version main
```

`main` is again only discovery. The updater resolves it to an immutable full commit SHA and verifies/pulls the same revision for Web, Control API, Worker, Content and host-tools before touching the running installation. An explicit full SHA can be supplied instead when reproducing a known revision.

Before mutation, the updater verifies that all running custom control-plane services have the same revision label and locally pins those current image IDs under that SHA for rollback. It then prepares the target host-tools bundle before entering downtime.

Target images are pre-pulled while the current control plane is still healthy, so a registry or image-space failure happens before downtime. After those pulls and target-bundle extraction, the updater performs a second storage preflight on the filesystem that holds `/var/lib/argus/update-backups/`. It reads the live PostgreSQL database size and requires at least **2× the database size + 1 GiB** of free space before writers are stopped. This intentionally over-reserves relative to the normally compressed custom-format dump so snapshot creation is not allowed to consume the last usable disk space.

New updates use transaction format 2 with explicit durable phase boundaries. The transaction is:

1. copy the current `.env`, Compose/Caddy assets, native binaries and systemd units into the root-only transaction directory;
2. seal that exact fixed file set with `file-snapshot.sha256` and verify it before any live mutation is armed;
3. stop Agent/Helper and the control-plane writers;
4. take a custom-format `pg_dump` of the complete Argus PostgreSQL database, including both control and Payload schemas;
5. require `pg_restore --list` to accept the dump, then persist and verify `database-snapshot.sha256`;
6. install the target version-matched assets/binaries and switch `.env` to the target SHA;
7. durably write `target-start-armed` with that target SHA; only after this marker may target Compose services start and migrations run;
8. wait for service health, reload validated Caddy configuration, restart Helper/Agent and require `argusctl smoke` before declaring success.

If a normal mutation-stage step fails, rollback first verifies the sealed file snapshot before restoring it. PostgreSQL is recreated from the pre-update dump only when `target-start-armed` already existed. Before that marker, the target control plane was never allowed to start, so a file-installation failure or crash can be recovered without unnecessarily replacing an otherwise untouched database.

The same phase markers are used after a hard reboot. A format-2 transaction with metadata but no sealed file snapshot is treated as `ABORTED_PRE_MUTATION` when the installed revision is still the original revision. A sealed snapshot with no target-start marker restores the files only. A transaction with `target-start-armed` requires the expected target SHA plus a checksum-valid, structurally readable database dump before database rollback is attempted. Existing format-1 transaction snapshots remain supported by the legacy conservative recovery path.

Transaction snapshots are stored under `/var/lib/argus/update-backups/` with root-only permissions. To prevent normal successful updates from filling a small VPS indefinitely, Argus automatically keeps the **three newest terminal snapshots** whose result is `SUCCEEDED` or `ROLLED_BACK`. Pruning recognizes only Argus-generated transaction directory names with complete metadata/file snapshots. Incomplete transactions, `ROLLBACK_FAILED` transactions and unrelated/manual directories are never automatically removed. Pruning runs before a new update and again after a successful update.

V1 deliberately does **not** expose a generic manual rollback-to-old-snapshot command after a successful update: restoring an old database later would discard legitimate changes made since the update. The retained snapshots exist for the bounded automatic recovery path and operator investigation; a future operator-driven point-in-time rollback still needs a stronger data-loss confirmation model.

### Control-plane self-protection

Every Argus-owned Compose service is labelled `com.argus.protected=true`. The privileged Helper checks this label before container or Compose start/stop/restart operations and returns `PERMISSION_DENIED` for protected control-plane containers. This is enforced below the UI/API boundary.

## Managed server model

On a disposable first-server test host, `sudo -E ./scripts/first-server-acceptance.sh product` exercises the installed product through supported APIs. It creates a fresh personal Project without a Client, creates an environment/service/site, verifies audit and domain-event persistence plus Payload project synchronization, proves a persisted default schedule completed after the recorded reboot, runs a scheduled site monitor, executes a read-only typed Agent service action, and confirms the protected Argus control-plane container cannot be restarted through managed Docker actions. It also creates a typed system-config backup, waits for Agent inventory, verifies the checksum/archive, and runs non-mutating restore preflight. The command writes only IDs/artifact names and completion evidence to the root-only acceptance directory. Transactional restore apply still requires explicit maintenance on the disposable host and is not inferred from preflight success.

After `product`, `sudo -E ./scripts/first-server-acceptance.sh content` uses the installed Payload service's internal Argus APIs for that same personal Project. It requires the Project synchronization to exist, creates immediate-write App Data models/records plus a validated relation, then creates a public content model, proves a draft is absent from anonymous reads, publishes the record and verifies its sanitized public response. The script executes inside the Content container only to reach the private service port; it uses supported HTTP APIs and does not mutate PostgreSQL directly.

A managed server is enrolled with an Argus Agent. Heartbeats provide system identity and snapshots used for online/offline state, CPU/RAM/disk/load/uptime information and capability-specific inventories.

The Agent polls the Control API for typed commands and calls the local privileged Helper. A server is not considered controllable merely because a hostname exists in inventory; it must have an authenticated Agent with the required capability.

## Services and systemd

Managed systemd services support typed start, stop and restart actions. Service names are validated and must be present in the Helper allowlist. The Helper calls `systemctl` directly and does not construct shell command strings.

Recent journald output can be collected for managed services. Diagnostics also include failed systemd units and listening TCP ports.

## Package updates and maintenance

APT-based hosts expose update inventory and reboot-required state.

Typed operations include:

- refresh package metadata;
- install security updates through the host's unattended-upgrades configuration;
- install available upgrades;
- request a reboot.

Disruptive update/reboot operations require an active maintenance window at the Control API, not only a disabled/enabled button in the UI. Maintenance windows are persisted with reason and timing and can be ended early.

A command being accepted is not by itself proof that a reboot completed; reconnect/uptime correlation is the stronger operational signal.

## Docker

The Agent reports Docker container inventory when Docker is available. Typed container start, stop and restart actions use validated container references and the local Helper.

Argus does not expose arbitrary `docker` CLI arguments through the command API. Argus control-plane containers carry a protected label and cannot be mutated through these managed actions.

## Compose stacks

Compose projects are represented as first-class stack resources rather than opaque shell commands. Argus discovers/configures stack identity and provides typed start, stop and restart operations.

The Helper resolves known Compose configuration files from Docker's own Compose project inventory before invoking actions. Project names and discovered paths are validated. Arbitrary uploaded Compose execution is not treated as equivalent to an approved stack.

## Monitoring

Site monitoring records operational checks such as DNS/HTTP reachability, response latency, TLS state and selected website metadata. Monitoring includes SSRF protections rather than blindly requesting arbitrary internal targets.

Checks are persisted and scheduled through the Jobs/Worker subsystem. Historical checks drive site state, incident automation and domain TLS lifecycle observations.

## Monitoring schedules and background jobs

Recurring operations are persisted in PostgreSQL job schedules. Workers claim due jobs, execute typed internal handlers and record attempts/results. This is used for tasks including:

- site monitoring;
- incident evaluation;
- notification materialization;
- desired-state reconciliation;
- domain lifecycle evaluation;
- Payload project synchronization.

Jobs have bounded retries and are designed to be idempotent or deduplicated at their mutation boundary. The Jobs administration surface exposes job status rather than hiding recurring work inside long-lived web processes.

## Incidents

Incidents are project-scoped operational records with severity, status, affected resources, timeline and resolution context. Site-monitoring automation can create/evaluate incidents after configured failure conditions rather than treating every individual failed check as a new incident.

The dependency graph and change-correlation data provide context for understanding what might be affected and what changed near the start of an incident.

## Change correlation

Argus records operational/domain events from deployments and other mutations. Change correlation combines relevant recent changes with incident/operational context so an operator can inspect plausible causes without manually searching every subsystem.

Correlation is evidence/context, not automatic root-cause proof.

## Notifications

Notification rules match domain events and materialize operator notifications through background jobs. Notifications can be read and acknowledged in Argus.

Because modules emit normal project-scoped events, features such as domain lifecycle can use the same notification system instead of implementing their own unrelated alert engine.

External channels such as email/chat/webhook/push are future extensions unless explicitly implemented by a provider later.

## Status pages

Projects can expose status-page information from selected operational components and incidents. Status pages are a presentation layer over Argus operational state; they do not bypass incident or monitoring ownership.

## Desired state and drift

Argus stores desired security state separately from actual Agent observations and reports drift for configured fields.

Current automatic reconciliation is deliberately narrow. Firewall `must be active` is the supported enforcement primitive because it has a rollback-safe implementation. Other observed policy fields remain monitor-only until equivalent preflight/rollback guarantees exist.

## Operational non-goals

Argus currently does not claim to provide:

- arbitrary remote shell/terminal as its normal management model;
- full metrics/time-series observability;
- generalized configuration management for every Linux setting;
- automatic provider provisioning;
- automatic DNS/Cloudflare mutation;
- completely autonomous remediation for arbitrary incidents;
- production-grade multi-node/rolling control-plane upgrades or arbitrary point-in-time rollback.

The next deployment proof is the first real clean-server install, smoke test and self-update exercise described in [Roadmap](roadmap.md).
