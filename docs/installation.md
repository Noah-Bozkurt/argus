# Installation

Argus currently supports a guided single-server installation on **Ubuntu or Debian amd64**. The public entry point is:

**https://install.noahbozkurt.nl**

## Before you start

Prepare:

- a clean Ubuntu/Debian amd64 server with root or sudo access;
- DNS for the main Argus domain, for example `argus.example.com`;
- DNS for a separate content domain, for example `content.argus.example.com`;
- inbound TCP 80 and 443 available;
- a GitHub classic PAT with `read:packages` and access to the private Argus GHCR packages.

Both domains must already resolve through DNS before a control-plane installation can continue. A records, AAAA records and CNAME chains are supported. Cloudflare-proxied records are also supported: Argus checks that the hostname resolves, not that the returned address equals the origin server's public IP.

The review screen also asks for a certificate contact email. On ordinary DNS records Caddy requests from Let's Encrypt first and falls back to ZeroSSL. When both hostnames expose Cloudflare's managed `/cdn-cgi/trace` endpoint, the installer offers Cloudflare Origin CA. That option needs a scoped API token with `Zone / SSL and Certificates / Edit`; the token is held only for the installation and is not written to `.env`. Keep the Cloudflare SSL/TLS mode on **Full (strict)**. Origin CA certificates are trusted by Cloudflare, not directly by browsers, so disabling the proxy later will produce a certificate warning.

For unattended installs, set `ARGUS_ACME_EMAIL`. You may also set `ARGUS_CLOUDFLARE_API_TOKEN`; it is only used when both domains are positively detected as proxied. If detection is mixed or the token is absent, the installer safely stays with public ACME.

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

The installer asks for the primary domain, content domain, login details and private GHCR credentials. The content domain defaults to `content.<primary-domain>`. It then opens a review screen: use the arrow keys to move between values and press Enter to edit the selected row. Tokens and passwords remain masked. Nothing is installed until **Install Argus** is selected and the final checks pass.

The GitHub token is entered silently. Registry credentials are stored root-only in `/etc/argus/registry.env` with mode `0600` so later updates and repairs can pull the coordinated Argus image set.

The installer also asks for the initial operator credentials. These seed the first Argus `owner` account used by the first-party login page and the Payload CMS. The older `ARGUS_BASIC_AUTH_*` configuration names are retained for upgrade compatibility, but Caddy no longer performs HTTP Basic Auth and browsers no longer show a native Basic Auth prompt.

### What gets installed

The default paths are:

```text
/opt/argus/       control-plane Compose files and runtime environment
/etc/argus/       host configuration and stored registry credentials
/var/lib/argus/   persistent Argus host state and backups
/var/log/argus/   installer logs
/usr/local/bin/   argus-agent, argus-helper and argusctl
```

The control plane runs its application services with Docker Compose. The Agent and privileged Helper run as native systemd services.

Running the public installer on an existing host opens a recovery menu instead of silently reinstalling over it. From there you can repair, update or uninstall the existing installation. Repair keeps the installed immutable revision and preserves IDs, secrets, certificates, data and media.

## Verify the installation

After installation:

```bash
argusctl status
argusctl health
argusctl connection
argusctl doctor
sudo argusctl smoke
```

`argusctl smoke` exercises the installed control plane more broadly than the local service checks and should be the first command used after installation or an update.

`argusctl doctor` is the best starting point when a host is not behaving as expected. It continues through failed checks and reports practical next steps:

```bash
argusctl doctor
argusctl doctor --offline
argusctl --json doctor
```

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
9. Provide the private GHCR credentials when requested so the host tools can be pulled.

The installer enrolls the Agent, writes its local configuration, removes the one-time enrollment token from the persistent environment and starts `argus-agent.service` and `argus-helper.service`.

Check the new node with:

```bash
argusctl status
argusctl health
argusctl connection
```

## Update Argus

The normal update target is the newest coordinated release currently promoted as `main`:

```bash
sudo argusctl update --version main
```

Argus resolves the release to an immutable Git revision. The update path performs preflight checks, keeps rollback material and verifies the resulting installation instead of simply replacing running containers in place.

To install a specific published revision:

```bash
sudo argusctl update --version <40-character-git-sha>
```

Interactive updates show a confirmation before changing the host. Automation must add `--yes`.

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

## Rotate GHCR credentials

If the package PAT expires or is replaced:

```bash
sudo argusctl registry-login
```

Optionally provide the GitHub username non-interactively on the command line while still entering the token securely:

```bash
sudo argusctl registry-login --username <github-user>
```

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

Start with:

```bash
argusctl status
argusctl health
argusctl connection
argusctl domain check
sudo argusctl smoke
```

Useful host-level checks include:

```bash
systemctl status argus-agent.service
systemctl status argus-helper.service
journalctl -u argus-agent.service
journalctl -u argus-helper.service
```

Installer logs are written under `/var/log/argus/`.

If an update cannot pull images because the stored package credential is no longer valid, rotate it with `sudo argusctl registry-login` before retrying.

## Current installation limits

The current path is deliberately narrow:

- Ubuntu/Debian only;
- amd64 only;
- single control-plane host;
- direct HTTP/HTTPS ingress through Caddy;
- private GHCR access is required;
- first-party login is implemented, but per-user identity is not yet propagated through every Web -> Control API audit path.

See [Authentication](authentication.md) for the current session, role and identity model.
