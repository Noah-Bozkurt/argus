# Branch-tracked updates

Argus normally updates from coordinated images published to the configured registry. For preview and development installations, `argusctl` can instead follow a Git branch and build that branch directly on the control-plane host.

## Start following a branch

```bash
sudo argusctl update --branch design/saasframe-redesign
```

The updater:

1. verifies the currently installed control plane before touching it;
2. shallow-fetches the exact branch into a root-only temporary directory;
3. resolves the branch head to an immutable 40-character Git revision;
4. builds the coordinated Web, Control API, Worker, Content and Host Tools images locally;
5. verifies that every image carries the same immutable revision and compatible update protocol;
6. delegates to the target revision's updater;
7. enters the normal transactional snapshot, database backup, install, health-check and rollback path.

Branch images are loaded into the local Docker image store. They are not pushed to the registry.

## Remembered update source

A successful branch update stores the selected branch as `ARGUS_UPDATE_BRANCH` in the installed `/opt/argus/.env` file. A later update with no selector therefore follows the same branch:

```bash
sudo argusctl update
```

This state participates in the normal transactional file snapshot. If an update rolls back, the previous update source is restored with the previous installed environment.

To leave a preview branch and return to published registry updates, explicitly select a registry version or discovery tag:

```bash
sudo argusctl update --version main
```

A successful explicit version update clears the remembered branch, so subsequent `sudo argusctl update` commands use `main` again.

## Requirements and public source access

Branch updates require Git and Docker Buildx on the control-plane host. Fresh Argus installations provision Docker with the Buildx plugin. The update command fails before deployment mutation if either tool is unavailable.

Branch source and coordinated images are fetched anonymously from the public GitHub repository and public GHCR packages. `GIT_TERMINAL_PROMPT=0` prevents an update from blocking on an unexpected authentication prompt; inaccessible source or images fail before deployment mutation with the underlying Git or Docker diagnostic.

## Compatibility boundary

A branch update can replace `argusctl` itself. To make that handoff safe, branch-built Host Tools advertise an explicit branch-update protocol. A branch that predates branch-update support is rejected before the transactional mutation phase.

When starting a long-lived preview branch, branch it from a revision that already contains branch-update support, or merge/rebase that support into the branch before trying to deploy it.

## Security properties

Branch names are validated as Git branch refs before use. Branch fetches are restricted to `refs/heads/<branch>`, builds happen before service downtime, target images must all identify the resolved immutable commit, and the existing rollback/smoke-test guarantees remain in force. Branch mode changes where target artifacts come from; it does not bypass the transactional updater.
