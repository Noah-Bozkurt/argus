# Compose Stack Actions V1

This slice adds typed start, stop and restart operations for registered Docker Compose stacks.

## Protocol

Protocol version advances to 1.6 with the `docker.compose.v1` capability and three typed commands:

- `docker.compose.start`
- `docker.compose.stop`
- `docker.compose.restart`

Compose operations share the existing `docker.mutate` conflict group, so container and stack mutations cannot race through the same server command queue.

## Control-plane boundary

The web API operates a stack by Argus stack ID. It does not accept a Compose project name, file path or arbitrary Docker arguments from the action request.

The Control API resolves the registered stack and queues a typed command using its stored Server and validated Compose project name. Archived stacks cannot be operated.

Risk levels:

- start: MEDIUM
- restart: MEDIUM
- stop: HIGH

## Privileged helper boundary

The helper receives only the validated Compose project name. Before executing an action it runs `docker compose ls --format json`, finds that exact project and reads its `ConfigFiles` value.

Only Docker-discovered absolute config-file paths are used to reconstruct the Compose project invocation. The browser and Control API never supply those paths.

The helper then runs the equivalent of:

```text
docker compose -p <registered-project> -f <docker-discovered-file> ... start|stop|restart
```

If the project is not present in Docker Compose discovery, the operation fails with `STACK_NOT_FOUND`.

## Explicit non-goals

This phase still does not expose:

- arbitrary Compose YAML uploads
- `up` / `down`
- pull or build
- exec or shell
- arbitrary Compose or Docker arguments
- arbitrary filesystem paths
- Compose secrets or environment values in the Argus UI

Those operations need an explicit deployment, secrets and privilege policy before they can safely cross the helper boundary.
