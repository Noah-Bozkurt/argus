# Roadmap

This roadmap describes the intended order from the current repository state. It is not a promise that every listed feature is already production-ready.

## Current implemented foundation

The repository already contains substantial working slices for:

- persistent project workspace and audit/activity events;
- GitHub repository integration;
- service catalog and environments;
- deployment/release records and readiness;
- sites, domains and domain lifecycle;
- managed Linux Agent/Helper control path;
- package maintenance, service diagnostics and Docker management;
- Compose stack catalog/actions;
- site monitoring and persisted schedules/jobs;
- incidents, incident automation and change correlation;
- notifications and status pages;
- dependency graph;
- desired-state drift and rollback-safe firewall activation/reconciliation;
- backup verification, restore preflight and transactional system-config restore;
- Payload App Data with project/organization scoping;
- Argus Project -> Payload project synchronization;
- committed Payload production migrations;
- CMS content models, drafts/version history, publication and explicit public content reads;
- a first-test hybrid deployment path using Docker Compose for the control plane and native Agent/Helper services;
- private custom images for Argus Web, Control API, Worker, Payload and host-tool artifacts;
- an Ubuntu/Debian amd64 first-test installer with bootstrap, local Agent enrollment, health verification and disposable reset;
- an embedded `argusctl smoke` verification path for a real installed server;
- immutable installed revisions even when `main` is used as the discovery alias;
- main-only image publication that runs only after normal CI succeeds and verifies expected remote GHCR tags;
- a single-server transactional `argusctl update` path with database/file preflight backup and automatic failure rollback.

These capabilities are described by the canonical documents in this directory. They should not be interpreted as a completed production product.

## Next required milestone: first real server test

The deployment/install/smoke/update foundation now has an implementation path. The next proof is to run it on a clean real VPS/VM using only the documented lifecycle and treat every manual workaround as a bug.

Before calling that test meaningful, the merged `main` commit must pass normal CI and its five custom Argus images must publish successfully. PR CI validates the clean database migration/start path and production application builds, but it does not replace a real Linux/network/systemd install.

### Initial supported target

The first supported target is deliberately narrow:

- Ubuntu/Debian-class Linux;
- amd64;
- clean/disposable server;
- direct public DNS to the server;
- inbound HTTP/HTTPS for Caddy;
- Docker Compose control plane;
- native systemd Agent + Helper.

Cloudflare Tunnel, arm64 and provider-specific provisioning should not be added before this first path is proven.

### First server test checklist

1. point the main and content DNS names at a clean test server;
2. run `install.sh` using a private-registry read credential and no manual service setup;
3. confirm the persisted `ARGUS_VERSION` is a full immutable commit SHA rather than `main`;
4. run `sudo argusctl smoke` and require every internal/public check to pass;
5. reboot the server and run `sudo argusctl smoke` again;
6. confirm local managed-node enrollment and heartbeat;
7. create a personal Project with no Client;
8. exercise server/service inventory and a safe typed action;
9. confirm the Argus control-plane containers cannot be stopped/restarted through normal managed Docker/Compose actions;
10. connect/create the minimum project/service/environment/site structure;
11. verify jobs/monitoring execute after restart;
12. confirm Argus Project synchronization reaches Payload;
13. create an application-data model/record;
14. create a content model, save a draft, publish it and read it through the public content endpoint;
15. create/verify a system-config backup;
16. test restore preflight; transactional live restore should only be tested on this disposable host with maintenance active;
17. rerun the installer and confirm it preserves IDs/secrets/data/revision rather than acting as an updater;
18. publish a second green `main` revision, run `sudo -E argusctl update --version main`, and require the update plus post-update smoke verification to succeed;
19. on a disposable update attempt, deliberately create a safe target-start failure and prove the automatic file/database rollback returns the previous revision to a green smoke test;
20. exercise the explicit first-test reset path and perform one second clean install;
21. record every manual workaround as an installer/product bug rather than adding undocumented setup knowledge.

### What is intentionally not required yet

The first test does not require:

- a general production installer for every distro;
- Cloudflare Tunnel automation;
- multi-node control-plane HA or rolling upgrades;
- arbitrary manual point-in-time rollback after a successful update;
- arm64 images;
- a finished end-user login/identity system;
- provider provisioning.

The current Caddy basic-auth layer is explicitly a temporary outer guard for the pre-production operator UI and Payload admin until first-class identity is built.

## After the first server test

Priorities should first be driven by failures and usability gaps found in that test. Once the installation/control-plane foundation is proven, the broader intended order remains:

### Deployment maturity

- turn the successful main-SHA lifecycle into named/versioned release installation rather than relying on `main` for discovery;
- publish release manifests/checksums and pin images by immutable digest where practical;
- add update-backup retention policy and a strongly confirmed operator-driven rollback/recovery workflow;
- add arm64 after the amd64 install/update/reset cycle is stable;
- improve install/update diagnostics and recovery from interrupted host reboots;
- add optional Cloudflare Tunnel/direct-proxy modes without making them core requirements;
- evolve single-server update into production-grade multi-node/rolling control-plane upgrade semantics only when that topology exists.

### Content/product layer

- Argus-native CMS abstraction above Payload internals;
- page/component schemas;
- visual CMS/editor and preview;
- media library, variants and optimization;
- forms and submissions;
- safe public relationship expansion.

### Operations maturity

- broader backup targets (database/volumes/application data) and full disaster recovery;
- secrets manager/rotation;
- richer desired-state enforcement field by field, each with preflight/rollback semantics;
- metrics/time-series observability;
- runbooks and an event/automation engine;
- safe browser terminal only as an escape hatch;
- reliable application-level deployment/update rollback independent of the Argus control-plane self-update path.

### Provisioning and networking

- provider adapter architecture;
- server/VPS provisioning;
- Cloudflare DNS/proxy/TLS automation;
- reusable service templates and project blueprints.

### Optional business layer

- Clients as an optional project context;
- approvals usable both with and without Clients;
- restricted client portal;
- cost/resource lifecycle tracking.

### Identity/security maturity

- first-class encrypted secrets;
- richer RBAC;
- 2FA/passkeys;
- OIDC/SSO where useful;
- stronger managed-node identity such as mTLS/public-key credentials;
- expanded security event/audit tooling.

## Sequencing rules

- do not call the deployment test-ready until normal CI and the main image publication succeed;
- keep Client optional in core models;
- prefer a complete safe vertical slice over a broad but fake feature surface;
- privileged mutations need typed APIs, authorization, audit and failure semantics;
- recovery-sensitive mutations need preflight and rollback before automation;
- CI must be green before merging;
- Docker image publication must consume the exact green `main` commit rather than rebuilding an untested revision;
- installed control-plane versions must be immutable revisions, not a silently moving `main` tag;
- update canonical documentation rather than creating another historical milestone file.
