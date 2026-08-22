#!/usr/bin/env bash
set -euo pipefail

if docker info >/dev/null 2>&1; then
  exit 0
fi

if ! command -v sudo >/dev/null 2>&1 || ! sudo -n /usr/bin/docker info >/dev/null 2>&1; then
  echo "Docker is installed but this runner cannot access /var/run/docker.sock." >&2
  id >&2 || true
  stat -c 'docker socket: mode=%a uid=%u gid=%g' /var/run/docker.sock >&2 || true
  exit 1
fi

wrapper_dir="$RUNNER_TEMP/argus-docker-bin"
mkdir -p "$wrapper_dir"
cat >"$wrapper_dir/docker" <<'SH'
#!/usr/bin/env bash
exec sudo -n /usr/bin/docker "$@"
SH
chmod 0755 "$wrapper_dir/docker"
echo "$wrapper_dir" >> "$GITHUB_PATH"
echo "Docker socket requires elevated access; using the runner's passwordless sudo for Docker commands."
