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
/etc/argus/       host and service configuration
/var/lib/argus/   persistent state and backups
/var/log/argus/   installer and host-update logs
/usr/local/bin/   argus-agent, argus-helper, argusctl
```

Only Caddy is publicly exposed. The Control API has a host-loopback path for the local Agent. PostgreSQL and the application containers are not published directly.

## Host lifecycle

### Primary troubleshooting flow

The normal operator flow is:

```bash
argusctl status
argusctl doctor
argusctl logs
sudo argusctl repair
```

`status` answers whether the local Agent/Helper are running and which server/control plane this host belongs to. `doctor` performs the broader diagnosis. `logs` exposes the underlying runtime output when more detail is needed. `repair` is the corrective step for damaged installed files or services.

### Local status

```bash
argusctl status
```

Shows native Agent/Helper service state and the enrolled Agent/Server/control-plane identity.

### One-command diagnosis

```bash
argusctl doctor
```

Doctor checks installation files and permissions, Agent and Helper services, the Helper socket, Compose containers, disk capacity, DNS, trusted public HTTPS and the authenticated Agent connection. It keeps checking after failures so one run gives a useful picture of the host.

Use `argusctl doctor --offline` when external network checks are not wanted. `argusctl --json doctor` returns stable machine-readable output.

### Logs

```bash
argusctl logs
```

On a managed node this shows the Agent and Helper journals. On a control-plane host it also shows the Web, Control API, Worker, Content, Caddy and PostgreSQL Compose logs.

Target individual sources when needed:

```bash
argusctl logs host
argusctl logs agent
argusctl logs helper
argusctl logs control-plane
argusctl logs web
argusctl logs control-api
argusctl logs worker
argusctl logs content
argusctl logs caddy
argusctl logs postgres
argusctl logs installer
argusctl logs update
```

Common options:

```bash
argusctl logs --tail 500
argusctl logs control-api --since 1h
argusctl logs web -f
```

`--follow`/`-f` follows new output. `--since` is passed to journald or Docker Compose and therefore applies to runtime logs, not the installer/update flat files. Log lines matching common credential shapes such as authorization headers, password/token/secret assignments and database URLs are redacted before printing.

### Repair

```bash
sudo argusctl repair
```

Repair restores the installed revision's binaries, service units and deployment files, then verifies the host. It preserves configuration, IDs, credentials, Caddy data, database volumes and media. If repair fails, the previous files and services are restored.

If `argusctl` cannot run, launch the public installer and choose **Repair this installation**. The downloaded installer provides the same recovery path without relying on the installed CLI.

### Advanced compatibility checks

The older commands remain available for scripts and precise low-level checks, but are intentionally hidden from normal `argusctl --help` output because `doctor` already covers their checks:

```bash
argusctl health
argusctl connection
sudo argusctl smoke
```

`health` checks the native services, Helper socket and host collection. `connection` performs only the authenticated Agent identity request. `smoke` runs the strict installed control-plane verification used by installer/update internals. They are not the recommended first-line troubleshooting workflow.

### System snapshot

```bash
argusctl system info
```

Returns a local JSON snapshot including hostname, OS/kernel, architecture, CPU/RAM/disk usage, load and uptime.

### Version

```bash
argusctl version
```

### Retrieve the generated login

The normal installer summary does not print passwords. If Argus generated the initial login password, root can retrieve it explicitly:

```bash
sudo argusctl credentials
```

Treat that output like any other credential and do not paste it into diagnostics.

## Installation behavior

The public installer detects an existing installation and offers repair, update or uninstall. It does not silently overwrite an installed host. Repair is not an update request; use:

```bash
sudo argusctl update --version main
```

for a normal update.

The installer and updater pull the public Argus image set anonymously. Current releases do not request or store GitHub package credentials. During upgrade, the updater removes the obsolete `/etc/argus/registry.env` file from older installations without reading or logging its contents.

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

Normal mode reports the failing update phase, image reference, Docker exit status and a concise underlying error. Add `--verbose` to stream complete secret-safe Docker diagnostics when investigating registry, network, daemon or storage failures.

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
10. requires service health and the internal strict smoke verification before success.

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

Package operations retain redacted APT output and expose their current phase in the server activity view. The Servers UI consumes targeted event streams instead of refreshing the entire page. On a control-plane host, an owner can schedule the existing transactional Argus updater from the System page; local recovery continues to use `argusctl`.

## Agent/Helper trust boundary

The Agent is the network-facing managed-node component. It is not root.

The Helper is root and listens only on a local Unix socket. It supports fixed, validated operation classes. Argus does not construct arbitrary shell commands from operator input for normal infrastructure management.

Argus-owned control-plane containers are marked with `com.argus.protected=true`. Managed Docker/Compose operations reject stop/restart actions against protected containers so the normal infrastructure surface cannot turn off its own control plane.

## Systemd services

Managed systemd services support typed start, stop and restart operations. Service names must pass validation and Helper allowlisting.

The Agent can also collect relevant service/journald diagnostics. Host-side troubleshooting for Argus itself normally uses:

```bash
argusctl logs host
argusctl logs agent -f
argusctl logs helper -f
```

Direct `systemctl`/`journalctl` remains available for low-level host debugging when the CLI itself is unavailable.

## Package maintenance

APT-based managed hosts expose update/reboot information and support typed maintenance operations such as metadata refresh and supported upgrade flows.

Disruptive operations are maintenance-gated where required by Control API policy. UI button state is not the enforcement boundary.
