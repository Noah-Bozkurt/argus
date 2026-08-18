# Compose / Stacks V1

Argus now treats a Docker Compose project as a project-owned resource instead of only showing its individual containers.

## Model

A registered stack belongs to an Argus Project and is attached to one managed Server. Its Environment is derived from that Server, so a stack cannot be registered against a conflicting project/environment combination.

Stored metadata:

- display name
- Docker Compose project name
- project
- server
- derived environment
- description
- lifecycle (`ACTIVE`, `PAUSED`, `ARCHIVED`)

Projects and stacks remain client-optional.

## Safety boundary

V1 is deliberately registration/inventory only.

It does **not**:

- accept or store arbitrary `compose.yaml` uploads
- send Compose definitions to the privileged helper
- run `docker compose up`, `down`, `pull`, `build`, `exec` or arbitrary Docker arguments
- expose environment/secrets from Compose definitions
- remove or stop a runtime stack when its Argus record is deleted

This preserves the typed privileged-operation boundary established by Docker Management V1. Arbitrary Compose input can request privileged containers, host mounts and other host-level capabilities, so treating uploaded YAML as a harmless configuration file would effectively introduce a broad privileged execution channel.

## API

- `GET /projects/:project_id/stacks`
- `POST /projects/:project_id/stacks`
- `GET /projects/:project_id/stacks/:stack_id`
- `PUT /projects/:project_id/stacks/:stack_id`
- `DELETE /projects/:project_id/stacks/:stack_id`

Mutations emit normal audit/domain events:

- `stack.created`
- `stack.updated`
- `stack.deleted`

## Next slice

The runtime integration should discover existing Compose projects through the managed helper/agent and add typed stack actions against those discovered identities. Definition deployment should only follow once Argus has an explicit policy for Compose privilege, secrets, mounts and other high-risk options rather than turning the helper into an arbitrary Docker execution channel.
