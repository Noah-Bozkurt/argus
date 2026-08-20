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
- an Argus-native project Content workflow for scalar content types, drafts and publication;
- protected generic draft preview with explicit links to published public content;
- project-owned collection, page and component schemas with validated ordered block layouts and a native block editor;
- project-scoped image library with explicit public delivery, persistent storage and bounded optimized variants;
- published typed forms with private paginated submissions, honeypot handling and durable privacy-preserving rate limits;
- native project-scoped relationship fields/pickers with explicit one-level published-only public expansion;
- a first-test hybrid deployment path using Docker Compose for the control plane and native Agent/Helper services;
- private custom images for Argus Web, Control API, Worker, Payload and host-tool artifacts;
- an Ubuntu/Debian amd64 first-test installer with bootstrap, local Agent enrollment, health verification and disposable reset;
- an embedded `argusctl smoke` verification path for a real installed server;
- immutable installed revisions even when `main` is used as the discovery alias;
- main-only image publication that runs only after normal CI succeeds and verifies expected remote GHCR tags;
- a single-server transactional `argusctl update` path with sealed file/database rollback material, durable crash phases, disk-space preflight, bounded successful-snapshot retention and fail-closed interrupted-update recovery;
- a checkpoint-based first-server lifecycle acceptance runner that can record install, real reboot, installer-rerun and update evidence without persisting plaintext Argus secrets in its report.

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

### Lifecycle acceptance runner

`scripts/first-server-acceptance.sh` turns the repeatable host-lifecycle portion of the first test into explicit checkpoints. It is evidence tooling, not a substitute for the real server: `install` still runs the real installer, `post-reboot` requires the Linux boot ID to have actually changed, and `update` requires a different immutable revision plus a matching durable `SUCCEEDED` update transaction.

Run it from the authenticated source checkout on the disposable server using the same domain/private-registry environment required by the normal lifecycle:

```bash
sudo -E ./scripts/first-server-acceptance.sh install
# reboot the host
sudo -E ./scripts/first-server-acceptance.sh post-reboot
sudo -E ./scripts/first-server-acceptance.sh product
sudo -E ./scripts/first-server-acceptance.sh rerun-installer
# after a newer green main revision has been published:
sudo -E ./scripts/first-server-acceptance.sh update
sudo ./scripts/first-server-acceptance.sh report
```

The runner stores root-only checkpoint state under `/var/lib/argus/acceptance/first-server/` by default. It fingerprints the generated high-entropy IDs/secrets to prove installer reruns and reboots preserve identity without writing those plaintext values into the final report. The report records immutable revisions, the real-reboot proof, smoke-test checkpoints and the exact successful update transaction. Registry credentials are never copied into acceptance state.

This only covers the reproducible host lifecycle subset. It does **not** mark project/CMS/backup/restore/protected-container/manual-failure checklist items as passed; those still require explicit execution on the real disposable server.

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

The lifecycle acceptance runner directly records evidence for checklist items 2-5, 7-10, 12, 17 and 18. The `product` stage creates a new personal Project through the authenticated Control API, requires its `client_id` to remain null, creates environment/service/site structures through supported APIs, verifies persisted audit/domain events and Payload synchronization, exercises a safe typed Agent action, and proves a protected control-plane container rejects a normal managed Docker action. SQL is used only to verify persisted effects. The remaining items stay separate acceptance work and must not be inferred from a lifecycle `PASS` report.

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
- add a strongly confirmed operator-driven rollback/recovery workflow for retained update snapshots;
- add arm64 after the amd64 install/update/reset cycle is stable;
- improve install/update diagnostics based on failures observed during the real lifecycle test;
- add optional Cloudflare Tunnel/direct-proxy modes without making them core requirements;
- evolve single-server update into production-grade multi-node/rolling control-plane upgrade semantics only when that topology exists.

### Content/product layer

- expand the Argus-native CMS abstraction with richer field settings (native project-safe relationship pickers are implemented);
- richer visual CMS/editor and site-template-aware preview (the typed page/component block editor and safe generic preview are implemented);
- richer media workflows (the image library, persistent originals and bounded thumbnail/medium/large variants are implemented; field pickers and external object storage remain);
- richer forms workflows (typed public forms, private submissions, durable throttling and bounded formula-safe CSV exports are implemented; notifications and conditional fields remain);
- richer/recursive relationship APIs where a bounded one-level published-only expansion is insufficient;

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
- do not call the first-server milestone passed until the real VPS/VM checkpoints and remaining checklist evidence have actually been executed;
- keep Client optional in core models;
- prefer a complete safe vertical slice over a broad but fake feature surface;
- privileged mutations need typed APIs, authorization, audit and failure semantics;
- recovery-sensitive mutations need preflight and rollback before automation;
- CI must be green before merging;
- Docker image publication must consume the exact green `main` commit rather than rebuilding an untested revision;
- installed control-plane versions must be immutable revisions, not a silently moving `main` tag;
- update canonical documentation rather than creating another historical milestone file.
