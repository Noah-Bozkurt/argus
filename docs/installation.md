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

The installer then asks for the information it needs, including the primary Argus domain and private GHCR credentials. The GitHub token is entered silently in the terminal. Registry credentials are stored root-only in `/etc/argus/registry.env` with mode `0600` so later updates can pull the coordinated Argus image set.

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

The installer is rerunnable. Existing generated IDs, secrets, data and the installed immutable revision are preserved. Rerunning the installer is **not** the update mechanism; use `argusctl update` for version changes.

## Verify the installation

After installation:

```bash
argusctl status
argusctl health
argusctl connection
sudo argusctl smoke
```

`argusctl smoke` exercises the installed control plane more broadly than the local service checks and should be the first command used after installation or an update.

Open the configured Argus URL in a browser once the checks are green. Sign in with the operator credentials configured during installation.

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

## Troubleshooting

Start with:

```bash
argusctl status
argusctl health
argusctl connection
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
