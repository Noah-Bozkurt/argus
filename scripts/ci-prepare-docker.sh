#!/usr/bin/env bash
set -euo pipefail

use_sudo=false
if /usr/bin/docker info >/dev/null 2>&1; then
  :
elif command -v sudo >/dev/null 2>&1 && sudo -n /usr/bin/docker info >/dev/null 2>&1; then
  use_sudo=true
else
  echo "Docker is installed but this runner cannot access /var/run/docker.sock." >&2
  id >&2 || true
  stat -c 'docker socket: mode=%a uid=%u gid=%g' /var/run/docker.sock >&2 || true
  exit 1
fi

wrapper_dir="$RUNNER_TEMP/argus-docker-bin"
mkdir -p "$wrapper_dir"
cat >"$wrapper_dir/docker" <<'SH'
#!/usr/bin/env bash
set -euo pipefail

docker_cmd=(/usr/bin/docker)
if [[ "${ARGUS_CI_DOCKER_USE_SUDO:-false}" == "true" ]]; then
  docker_cmd=(sudo -n /usr/bin/docker)
fi

# The runner itself is containerized while Docker runs on the host. Only the
# Postgres *server* command with the CI port mapping is rewritten. Other uses of
# postgres:16 (for example psql client commands) must pass through untouched.
if [[ "${1:-}" == "run" && -n "${HOSTNAME:-}" ]]; then
  args=("$@")
  has_postgres_image=false
  has_ci_postgres_port=false
  for ((i = 0; i < ${#args[@]}; i++)); do
    [[ "${args[$i]}" == "postgres:16" ]] && has_postgres_image=true
    if [[ "${args[$i]}" == "-p" && $((i + 1)) -lt ${#args[@]} && "${args[$((i + 1))]}" == "127.0.0.1::5432" ]]; then
      has_ci_postgres_port=true
    fi
  done

  if [[ "$has_postgres_image" == "true" && "$has_ci_postgres_port" == "true" ]]; then
    if ! runner_id="$("${docker_cmd[@]}" inspect "$HOSTNAME" --format '{{.Id}}' 2>/dev/null)" || [[ -z "$runner_id" ]]; then
      echo "[argus-ci] Docker daemon cannot resolve runner container ${HOSTNAME}" >&2
      exit 1
    fi

    label="argus.ci.runner=${HOSTNAME}"
    stale=()
    while IFS= read -r candidate; do
      [[ -n "$candidate" ]] || continue
      network_mode="$("${docker_cmd[@]}" inspect "$candidate" --format '{{.HostConfig.NetworkMode}}' 2>/dev/null || true)"
      runner_label="$("${docker_cmd[@]}" inspect "$candidate" --format '{{index .Config.Labels "argus.ci.runner"}}' 2>/dev/null || true)"
      image="$("${docker_cmd[@]}" inspect "$candidate" --format '{{.Config.Image}}' 2>/dev/null || true)"
      if [[ "$image" == "postgres:16" && ( "$runner_label" == "$HOSTNAME" || "$network_mode" == "container:${runner_id}" ) ]]; then
        stale+=("$candidate")
      fi
    done < <("${docker_cmd[@]}" ps -aq 2>/dev/null || true)

    if (( ${#stale[@]} > 0 )); then
      echo "[argus-ci] removing ${#stale[@]} stale runner-local Postgres container(s)" >&2
      "${docker_cmd[@]}" rm -f "${stale[@]}" >/dev/null 2>&1 || true
    fi

    rewritten=("run" "--label" "$label")
    i=1
    while (( i < ${#args[@]} )); do
      if [[ "${args[$i]}" == "-p" && $((i + 1)) -lt ${#args[@]} && "${args[$((i + 1))]}" == "127.0.0.1::5432" ]]; then
        rewritten+=("--network" "container:${runner_id}")
        i=$((i + 2))
        continue
      fi
      rewritten+=("${args[$i]}")
      i=$((i + 1))
    done

    echo "[argus-ci] starting Postgres inside runner ${HOSTNAME} network namespace" >&2
    if ! container_id="$("${docker_cmd[@]}" "${rewritten[@]}")"; then
      echo "[argus-ci] failed to start runner-local Postgres" >&2
      exit 1
    fi
    status="$("${docker_cmd[@]}" inspect "$container_id" --format '{{.State.Status}}/{{.HostConfig.NetworkMode}}' 2>/dev/null || true)"
    echo "[argus-ci] Postgres container ${container_id:0:12} status=${status:-unknown}" >&2
    printf '%s\n' "$container_id"
    exit 0
  fi
fi

# Kept for compatibility with older CI snippets that queried the published port.
if [[ "${1:-}" == "port" && "${3:-}" == "5432/tcp" ]]; then
  network_mode="$("${docker_cmd[@]}" inspect "${2:-}" --format '{{.HostConfig.NetworkMode}}' 2>/dev/null || true)"
  if [[ "$network_mode" == container:* ]]; then
    printf '127.0.0.1:5432\n'
    exit 0
  fi
fi

exec "${docker_cmd[@]}" "$@"
SH
chmod 0755 "$wrapper_dir/docker"
echo "ARGUS_CI_DOCKER_USE_SUDO=$use_sudo" >> "$GITHUB_ENV"
echo "$wrapper_dir" >> "$GITHUB_PATH"

if [[ "$use_sudo" == "true" ]]; then
  echo "Docker socket requires elevated access; using the runner's passwordless sudo for Docker commands."
fi
