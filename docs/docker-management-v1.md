# Docker Management V1

Docker support is intentionally limited to inventory and basic container lifecycle operations.

## Security boundary

The unprivileged `argus-agent` does not receive access to `/var/run/docker.sock`. Docker commands are sent as typed helper requests over the existing Argus Unix socket and executed by the privileged helper.

No arbitrary Docker arguments or shell commands are accepted.

## Inventory

Every 30 seconds the agent requests `docker ps -a --no-trunc --format '{{json .}}'` from the helper. The agent parses the JSON-lines output into structured container records:

- id
- name
- image
- state
- status
- ports

Inventory is limited to 500 containers and helper output is capped at 256 KiB. If Docker is unavailable, the normal server heartbeat remains healthy and Docker is reported as unavailable.

## Actions

Typed commands:

- `docker.start`
- `docker.stop`
- `docker.restart`

Container references are restricted to a bounded ASCII identifier/name character set before being passed as a direct CLI argument.

Start and restart are MEDIUM risk. Stop is HIGH risk.

## Non-goals

V1 deliberately excludes:

- container deletion
- image deletion/pruning
- volume deletion
- Docker Compose / stacks
- image pulling and upgrade automation
- arbitrary exec
- direct Docker socket access from the agent

Compose and deployment-oriented Docker functionality should be a separate follow-up phase after this inventory/control path is stable.
