# Installation

Argus currently supports a guided single-server installation on **Ubuntu or Debian amd64**. The public entry point is:

**https://install.noahbozkurt.nl**

## Before you start

Prepare:

- a clean Ubuntu/Debian amd64 server with root or sudo access;
- DNS for the main Argus domain, for example `argus.example.com`;
- DNS for a separate content domain, for example `content.argus.example.com`;
- inbound TCP 80 and 443 available;
- outbound HTTPS access to the public Argus packages on GHCR.

Both domains must already resolve through DNS before a control-plane installation can continue. A records, AAAA records and CNAME chains are supported. Cloudflare-proxied records are also supported: Argus checks that the hostname resolves, not that the returned address equals the origin server's public IP.

The review screen also asks for a certificate contact email. On ordinary DNS records Caddy requests from Let's Encrypt first and falls back to ZeroSSL. When both hostnames expose Cloudflare's managed `/cdn-cgi/trace` endpoint, the installer asks whether to use that public ACME route or Cloudflare Origin CA. The Cloudflare option needs a scoped API token with `Zone / SSL and Certificates / Edit`. Argus saves it in `/etc/argus/cloudflare.env` with mode `0600`, separate from the runtime `.env`, so repair can replace a missing certificate. Keep the Cloudflare SSL/TLS mode on **Full (strict)**. Origin CA certificates are trusted by Cloudflare, not directly by browsers, so disabling the proxy later will produce a certificate warning.

For unattended installs, set `ARGUS_ACME_EMAIL`. Set `ARGUS_TLS_MODE=cloudflare-origin` together with `ARGUS_CLOUDFLARE_API_TOKEN` to select Origin CA. It is only used when both domains are positively detected as proxied. Otherwise the installer stays with public ACME.

The control-plane installer installs required host packages and Docker when Docker is not already installed. It intentionally refuses unsupported distributions, unsupported architectures and conflicting container stacks instead of modifying them automatically.

## Install a control plane

Open the installer site or run its bootstrap directly:

```bash
curl -fsSL https://install.noahbozkurt.nl/install | sudo bash
```

The bootstrap performs three important steps before the installer runs:

1. downloads the canonical `install.sh`;
2. downloads `install.sh.sha256`;
3. verifies the SHA-256 checksum and only then executes the installer.

Choose **Install an Argus control plane here** when prompted.

The installer asks for the primary domain, content domain and login details. The content domain defaults to `content.<primary-domain>`. It then opens a review screen: use the arrow keys to move between values and press Enter to edit the selected row. Passwords remain masked. Nothing is installed until **Install Argus** is selected and the final checks pass. Coordinated Argus images are pulled anonymously from public GHCR packages.

The installer also asks for the initial operator credentials. These seed the first Argus `owner` account used by the first-party login page and the Payload CMS. The older `ARGUS_BASIC_AUTH_*` configuration names are retained for upgrade compatibility, but Caddy no longer performs HTTP Basic Auth and browsers no longer show a native Basic Auth prompt.

### What gets installed

The default paths are:

```text
/opt/argus/       control-plane Compose files and runtime environment
/etc/argus/       host and service configuration
/var/lib/argus/   persistent Argus host state and backups
/var/log/argus/   installer and host-update logs
/usr/local/bin/   argus-agent, argus-helper and argusctl
```

The control plane runs its application services with Docker Compose. The Agent and privileged Helper run as native systemd services.

Running the public installer on an existing host opens a recovery menu instead of silently reinstalling over it. From there you can repair, update or uninstall the existing installation. Repair keeps the installed immutable revision and preserves IDs, secrets, certificates, data and media.

## Verify the installation

After installation, the normal checks are:

```bash
argusctl status
argusctl doctor
```

`status` gives the quick service/enrollment overview. `doctor` performs the broader installation, Agent, container, DNS and HTTPS verification in one pass:

```bash
argusctl doctor
argusctl doctor --offline
argusctl --json doctor
```

If a check fails and you need the underlying output, use:

```bash
argusctl logs
argusctl logs control-plane
argusctl logs agent -f
```

The low-level `health`, `connection` and `smoke` commands still exist for compatibility and scripts, but are intentionally not part of the normal verification flow. The installer and updater continue to use strict smoke verification internally where required.

Open the configured Argus URL in a browser once the checks are green. Sign in with the operator credentials configured during installation.

If the installer generated the login password, retrieve it explicitly as root so it does not appear in normal installer logs or transcripts:

```bash
sudo argusctl credentials
```

## Add another managed server

Managed servers are enrolled from an existing Argus project.

1. Open the project.
2. Go to **Infrastructure**.
3. Create/select the target environment.
4. In **Add server**, enter the hostname and create a setup code.
5. Copy the setup code immediately. It is single-use and expires after 15 minutes.
6. On the server you want to manage, run:

```bash
curl -fsSL https://install.noahbozkurt.nl/install | sudo bash
```

7. Choose **Connect this server to an existing Argus instance**.
8. Paste the setup code when requested.
9. The installer pulls the public managed-node host tools from GHCR.

The installer enrolls the Agent, writes its local configuration, removes the one-time enrollment token from the persistent environment and starts `argus-agent.service` and `argus-helper.service`.

Check the new node with:

```bash
argusctl status
argusctl doctor
```

## Update Argus

The normal update target is the newest coordinated release currently promoted as `main`:

```bash
sudo argusctl update --version main
```

Argus resolves the release to an immutable Git revision. The update path performs preflight checks, keeps rollback material and verifies the resulting installation instead of simply replacing running containers in place.

Before changing the installed deployment, the current updater extracts the target revision's `argusctl` from the verified host-tools image and delegates the transaction to it. The current process keeps the update lock while the target runner verifies its revision, checksum and process identity. This lets each release interpret its own deployment templates and migration rules without replacing the installed CLI outside the rollback-protected transaction.

Targets that do not advertise a compatible update-runner protocol are rejected before the updater creates a transaction or stops services; use a supported bridge release instead of forcing an incompatible downgrade.

To install a specific published revision:

```bash
sudo argusctl update --version <40-character-git-sha>
```

Interactive updates show a confirmation before changing the host. Automation must add `--yes`.

Update output triggered through the browser is retained on the host and can be inspected with:

```bash
argusctl logs update
```

## Repair Argus

Use repair when installation files, service units or containers are missing or damaged:

```bash
sudo argusctl repair
```

Repair downloads the same immutable revision already installed on the host. It does not regenerate secrets, change domains, replace data or perform an update. Existing files are snapshotted first and restored if the repaired installation does not pass health checks.

If the local CLI or installer binary is missing, use the public installer:

```bash
curl -fsSL https://install.noahbozkurt.nl/install | sudo bash
```

Choose **Repair this installation**. For unattended recovery, use `--mode repair` after supplying the required registry environment variables.

## Manage the control-plane domains

Show the currently configured web and content domains:

```bash
argusctl domain show
```

Check their current DNS resolution:

```bash
argusctl domain check
```

Change the primary domain and use the default `content.<new-domain>` content hostname:

```bash
sudo argusctl domain set argus.example.com
```

Or set both hostnames explicitly:

```bash
sudo argusctl domain set argus.example.com --content-domain cms.example.com
```

`domain set` validates both hostnames and requires both to resolve before it changes `.env` or Caddy configuration. Cloudflare-proxied records are accepted because the check does not compare returned addresses with the origin IP. Argus validates the rendered Caddy configuration, recreates the domain-dependent Web, Content and Caddy services, and restores the previous `.env` and Caddy configuration if the apply step fails.

Interactive domain changes show the old and new values before applying them. Automation must add `--yes`. The command verifies trusted HTTPS on both new domains before reporting success. Certificate-authority rate limits are reported with their retry time, and a failed change restores the previous domains.

Changing the public control-plane hostname while additional managed agents are enrolled is currently blocked. Those agents persist the public control-plane URL locally, and switching the hostname without migrating that state would disconnect them. Automatic managed-agent URL migration should be implemented before this guard is relaxed.

## Uninstall

Interactive uninstall:

```bash
sudo argusctl uninstall
```

For automation, confirmation can be supplied with:

```bash
sudo argusctl uninstall --yes
```

By default, uninstall is conservative about persistent data. To explicitly remove retained Argus data as well:

```bash
sudo argusctl uninstall --purge-data
```

Do not use `--purge-data` unless the installation state and backups are intentionally disposable.

When data is preserved, Argus also writes a root-only recovery bundle under `/var/lib/argus/uninstall-recovery`. Running the public installer again will detect it and offer repair. Purging removes the recovery bundle, database volume, media, backups and logs permanently.

## Troubleshooting

Use the primary flow first:

```bash
argusctl status
argusctl doctor
argusctl logs
```

Target the failing layer rather than dropping directly to Docker/systemd commands:

```bash
argusctl logs control-api --tail 500
argusctl logs caddy --since 1h
argusctl logs host -f
argusctl logs installer
argusctl logs update
```

If `argusctl` itself cannot run, direct host tools remain the fallback:

```bash
systemctl status argus-agent.service
systemctl status argus-helper.service
journalctl -u argus-agent.service
journalctl -u argus-helper.service
```
