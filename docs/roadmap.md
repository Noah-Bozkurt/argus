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
- CMS content models, drafts/version history, publication and explicit public content reads.

These capabilities are described by the canonical documents in this directory. They should not be interpreted as a completed production product.

## Next required milestone: first-test installer

**The installer is now a hard prerequisite for the first real server test.**

The first test should not depend on manually reconstructing a development setup from memory. The initial installer should optimize for one reproducible test environment rather than prematurely supporting every distribution/provider.

### Initial supported target

Start with a documented Ubuntu/Debian-class Linux server and a single-node test topology. Expand platforms only after the first install/update/remove cycle is reliable.

### Installer responsibilities

The first-test installer should:

1. perform OS/architecture/prerequisite checks and fail with actionable messages;
2. create the Argus system user/group and protected directories;
3. install or verify required runtime dependencies;
4. install/build the Control API, worker, web app, Payload content app, Agent, Helper and CLI from a pinned Argus revision/release;
5. provision/configure PostgreSQL for the test deployment without exposing it publicly by default;
6. generate/store high-entropy service credentials with restrictive permissions;
7. configure Control API and Payload environment files;
8. apply Control API migrations and committed Payload migrations;
9. install service definitions with explicit dependency/start ordering;
10. configure the Helper Unix socket and Agent permissions;
11. create or guide the minimal first organization/user bootstrap;
12. enroll the local/selected managed node without requiring manual editing of Agent credential files;
13. start services and run health checks for database, Control API, worker, web, Payload, Agent and Helper;
14. print the URL/next steps and enough diagnostics to troubleshoot a failed install;
15. be safe to rerun where practical and never silently overwrite unknown production data.

### Installer safety before first test

Before calling the installer usable, verify at least:

- clean install on a fresh test VM/server;
- interrupted/failed install produces useful recovery instructions;
- secrets are not world-readable or printed into logs unnecessarily;
- PostgreSQL/Helper are not accidentally exposed to the public network;
- migrations can run from an empty database;
- all services survive reboot and respect dependency ordering;
- Agent enrollment and heartbeat work after reboot;
- a minimal uninstall/reset path exists for the disposable test environment.

Full self-update/rollback is not required for the first test installer, but the layout must not make later upgrades impossible.

## First server test

Only after the installer milestone is complete:

1. install Argus on a clean test server using only the documented installer path;
2. log into/open the operator UI;
3. confirm local managed-node enrollment and heartbeat;
4. create a personal Project (no Client required);
5. exercise service/server inventory and a safe typed action;
6. connect/create the minimum project/service/environment/site structure;
7. verify jobs/monitoring execute after restart;
8. confirm Argus Project synchronization reaches Payload;
9. create an application-data model/record;
10. create a content model, save a draft, publish it and read it through the public content endpoint;
11. create/verify a system-config backup;
12. test restore preflight; transactional live restore should only be tested on a disposable host with maintenance active;
13. record every manual workaround as an installer/product bug rather than institutionalizing it as setup documentation.

## After the first server test

Priorities should be driven by failures and usability gaps found in that test. The broader intended order is:

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
- reliable application/update rollback.

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

When development resumes after this documentation pass:

- do not move the first server test ahead of the installer;
- keep Client optional in core models;
- prefer a complete safe vertical slice over a broad but fake feature surface;
- privileged mutations need typed APIs, authorization, audit and failure semantics;
- recovery-sensitive mutations need preflight and rollback before automation;
- CI must be green before merging;
- update canonical documentation rather than creating another historical milestone file.
