#!/usr/bin/env bash
set -Eeuo pipefail

readonly CONTROLLER_USER=exposure-controller
readonly EXECUTOR_USER=exposure-executor
readonly SHARED_GROUP=exposure-benchmark
readonly STATE_DIRECTORY="${BENCHMARK_STATE_DIRECTORY:-/var/lib/exposure-benchmark}"
readonly RUNTIME_DIRECTORY=/run/exposure-benchmark
readonly EXECUTOR_DIRECTORY="$RUNTIME_DIRECTORY/executor"
readonly GATEWAY_DIRECTORY="${BENCHMARK_GATEWAY_DIRECTORY:-$RUNTIME_DIRECTORY/gateways}"
readonly EXECUTOR_RUNTIME="$RUNTIME_DIRECTORY/executor-runtime"
readonly PODMAN_STORAGE="$STATE_DIRECTORY/rootful-podman"
readonly PODMAN_RUNTIME="$RUNTIME_DIRECTORY/rootful-podman"
readonly PODMAN_API_DIRECTORY=/tmp/exposure-benchmark-podman
readonly PODMAN_API_SOCKET="$PODMAN_API_DIRECTORY/podman.sock"
readonly EXECUTOR_SOCKET="${BENCHMARK_EXECUTOR_SOCKET:-$EXECUTOR_DIRECTORY/executor.sock}"
readonly EXECUTOR_HOME="$STATE_DIRECTORY/executor"
readonly CONTROLLER_HOME="$STATE_DIRECTORY/controller"
readonly SEED_DIRECTORY=/opt/exposure-benchmark/seed
readonly SANDBOX_REVISION=/opt/exposure-benchmark/sandbox.sha256
readonly SANDBOX_IMAGE="${BENCHMARK_SANDBOX_IMAGE:-localhost/exposure-harness-arc:0.9.9}"
readonly DNS_RESOLVER="${BENCHMARK_DNS_RESOLVER:-}"

log() {
  printf '%s benchmark-container: %s\n' "$(date -u +%FT%TZ)" "$*" >&2
}

die() {
  log "ERROR: $*"
  exit 1
}

[[ $EUID -eq 0 ]] || die "entrypoint must start as root so it can split controller and executor privileges"
[[ -c /dev/fuse ]] || die "/dev/fuse is unavailable; retain the Compose device mapping"
[[ $(stat -fc %T /sys/fs/cgroup) == cgroup2fs ]] || die "the host must use cgroup v2"
if [[ -r /proc/sys/kernel/unprivileged_userns_clone ]] \
  && [[ $(< /proc/sys/kernel/unprivileged_userns_clone) != 1 ]]; then
  die "set kernel.unprivileged_userns_clone=1 on the EC2 host before deploying"
fi
if [[ -n $DNS_RESOLVER ]]; then
  [[ $DNS_RESOLVER =~ ^[0-9A-Fa-f:.]+$ ]] || die "BENCHMARK_DNS_RESOLVER must be an IP address"
  printf 'nameserver %s\noptions timeout:2 attempts:3\n' "$DNS_RESOLVER" > /etc/resolv.conf
fi

controller_uid=$(id -u "$CONTROLLER_USER")

seed_if_missing() {
  local relative=$1
  local target="$STATE_DIRECTORY/$relative"
  [[ -e $target ]] && return
  log "seeding $relative into the persistent worker volume"
  install -d -m 0700 -o "$EXECUTOR_USER" -g "$EXECUTOR_USER" "$(dirname "$target")"
  cp -a "$SEED_DIRECTORY/$relative" "$target"
  chown -R "$EXECUTOR_USER:$EXECUTOR_USER" "$target"
}

install -d -m 0751 -o root -g "$SHARED_GROUP" "$STATE_DIRECTORY"
seed_if_missing venv
seed_if_missing cache
seed_if_missing executor/.local/share/uv/tools/harbor
install -d -m 0700 -o "$EXECUTOR_USER" -g "$EXECUTOR_USER" \
  "$EXECUTOR_HOME" \
  "$EXECUTOR_HOME/.config/containers" \
  "$EXECUTOR_HOME/.local" \
  "$EXECUTOR_HOME/.local/share" \
  "$EXECUTOR_HOME/.local/share/containers" \
  "$STATE_DIRECTORY/runs"
install -d -m 0700 -o root -g root "$PODMAN_STORAGE" "$PODMAN_RUNTIME"
install -d -m 0700 -o root -g root "$PODMAN_API_DIRECTORY"
rm -f "$PODMAN_API_SOCKET"
install -d -m 0700 -o "$CONTROLLER_USER" -g "$CONTROLLER_USER" "$CONTROLLER_HOME"
install -d -m 0751 -o root -g "$SHARED_GROUP" "$RUNTIME_DIRECTORY"
install -d -m 2750 -o "$EXECUTOR_USER" -g "$SHARED_GROUP" "$EXECUTOR_DIRECTORY"
install -d -m 2750 -o "$CONTROLLER_USER" -g "$SHARED_GROUP" "$GATEWAY_DIRECTORY"
install -d -m 0700 -o "$EXECUTOR_USER" -g "$EXECUTOR_USER" "$EXECUTOR_RUNTIME"
rm -f "$EXECUTOR_SOCKET"

# Buildx talks to Podman's Docker-compatible API. That daemon does not
# consistently inherit the executor's per-user config, so set its init path
# globally as well.
install -d -m 0755 /etc/containers
cat >/etc/containers/containers.conf <<'EOF'
[containers]
init_path="/usr/bin/catatonit"
EOF

cat >"$EXECUTOR_HOME/.config/containers/containers.conf" <<'EOF'
[containers]
init_path="/usr/bin/catatonit"

[engine]
cgroup_manager="cgroupfs"
events_logger="file"
EOF
cat >"$EXECUTOR_HOME/.config/containers/storage.conf" <<EOF
[storage]
driver="overlay"
runroot="$PODMAN_RUNTIME/containers"
graphroot="$PODMAN_STORAGE/storage"

[storage.options.overlay]
mount_program="/usr/bin/fuse-overlayfs"
EOF
chown -R "$EXECUTOR_USER:$EXECUTOR_USER" "$EXECUTOR_HOME/.config"
chmod 0700 "$EXECUTOR_HOME" "$EXECUTOR_HOME/.config" "$EXECUTOR_HOME/.config/containers"
chmod 0600 "$EXECUTOR_HOME/.config/containers/containers.conf" "$EXECUTOR_HOME/.config/containers/storage.conf"

as_executor() {
  env -i \
      PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin \
      HOME="$EXECUTOR_HOME" \
      XDG_RUNTIME_DIR="$EXECUTOR_RUNTIME" \
      XDG_CONFIG_HOME="$EXECUTOR_HOME/.config" \
      CONTAINERS_CONF="$EXECUTOR_HOME/.config/containers/containers.conf" \
      CONTAINERS_STORAGE_CONF="$EXECUTOR_HOME/.config/containers/storage.conf" \
      "$@"
}

podman_info=$(as_executor podman --cgroup-manager=cgroupfs info --format '{{.Host.Security.Rootless}} {{.Host.CgroupsVersion}}') \
  || die "rootless Podman could not start; verify privileged, cgroup: host, /dev/fuse, and user namespaces"
[[ $podman_info == 'false v2' ]] || die "expected rootful Podman on cgroup v2, received: $podman_info"
as_executor docker compose version >/dev/null || die "Docker Compose CLI is unavailable"

if ! cmp -s "$SANDBOX_REVISION" "$STATE_DIRECTORY/.sandbox.sha256" \
  || ! as_executor podman --cgroup-manager=cgroupfs image exists "$SANDBOX_IMAGE"; then
  log "building the isolated ARC sandbox image"
  as_executor podman --cgroup-manager=cgroupfs image rm -f "$SANDBOX_IMAGE" >/dev/null 2>&1 || true
  as_executor podman --cgroup-manager=cgroupfs pull docker.io/library/python:3.12.11-slim-bookworm
  as_executor podman --cgroup-manager=cgroupfs build --pull=never \
    --label academy.harness.version=harness-2026-sprint-v3 \
    -t "$SANDBOX_IMAGE" \
    -f /opt/exposure-benchmark/sandbox/Containerfile \
    "$STATE_DIRECTORY/cache/arc-starter"
  cp "$SANDBOX_REVISION" "$STATE_DIRECTORY/.sandbox.sha256"
  chown "$EXECUTOR_USER:$EXECUTOR_USER" "$STATE_DIRECTORY/.sandbox.sha256"
fi

start_executor() {
  env -i \
      PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin \
      HOME="$EXECUTOR_HOME" \
      XDG_RUNTIME_DIR="$EXECUTOR_RUNTIME" \
      XDG_CONFIG_HOME="$EXECUTOR_HOME/.config" \
      CONTAINERS_CONF="$EXECUTOR_HOME/.config/containers/containers.conf" \
      CONTAINERS_STORAGE_CONF="$EXECUTOR_HOME/.config/containers/storage.conf" \
      ENVIRONMENT="${ENVIRONMENT:-PROD}" \
      HARNESS_HARBOR_USER=root \
      HARNESS_PODMAN_ROOTFUL=1 \
      BENCHMARK_CONTROLLER_UID="$controller_uid" \
      BENCHMARK_EXECUTOR_SOCKET="$EXECUTOR_SOCKET" \
      BENCHMARK_STATE_DIRECTORY="$STATE_DIRECTORY" \
      BENCHMARK_ADAPTER="${BENCHMARK_ADAPTER:-/opt/exposure-benchmark/adapters/runner.py}" \
      BENCHMARK_PYTHON="${BENCHMARK_PYTHON:-$STATE_DIRECTORY/venv/bin/python}" \
      BENCHMARK_SANDBOX_IMAGE="$SANDBOX_IMAGE" \
      /opt/exposure-benchmark/bin/benchmark-executor &
  executor_pid=$!
}

start_podman_api() {
  as_executor podman --cgroup-manager=cgroupfs system service --time=0 "unix://$PODMAN_API_SOCKET" &
  podman_service_pid=$!
  for _ in $(seq 1 50); do
    [[ -S $PODMAN_API_SOCKET ]] && break
    if ! kill -0 "$podman_service_pid" 2>/dev/null; then
      wait "$podman_service_pid" || true
      die "rootful Podman API exited before opening its socket"
    fi
    sleep 0.1
  done
  [[ -S $PODMAN_API_SOCKET ]] || die "rootful Podman API did not open $PODMAN_API_SOCKET"
  as_executor env "DOCKER_HOST=unix://$PODMAN_API_SOCKET" docker version --format '{{.Server.Version}}' >/dev/null \
    || die "rootful Podman API is not reachable through Docker"
}

start_controller() {
  setpriv --reuid="$controller_uid" --regid="$(id -g "$CONTROLLER_USER")" --init-groups -- \
    env \
      HOME="$CONTROLLER_HOME" \
      XDG_RUNTIME_DIR="$RUNTIME_DIRECTORY/controller-runtime" \
      BENCHMARK_EXECUTOR_SOCKET="$EXECUTOR_SOCKET" \
      BENCHMARK_GATEWAY_DIRECTORY="$GATEWAY_DIRECTORY" \
      /opt/exposure-benchmark/bin/benchmark-controller &
  controller_pid=$!
}

start_podman_api
start_executor
for _ in $(seq 1 100); do
  [[ -S $EXECUTOR_SOCKET ]] && break
  if ! kill -0 "$executor_pid" 2>/dev/null; then
    wait "$executor_pid" || true
    die "benchmark-executor exited before opening its socket"
  fi
  sleep 0.1
done
[[ -S $EXECUTOR_SOCKET ]] || die "benchmark-executor did not open $EXECUTOR_SOCKET"

start_controller
stop() {
  log "stopping benchmark worker"
  kill -TERM "$controller_pid" "$executor_pid" "$podman_service_pid" 2>/dev/null || true
  wait "$controller_pid" 2>/dev/null || true
  wait "$executor_pid" 2>/dev/null || true
  wait "$podman_service_pid" 2>/dev/null || true
}
trap 'stop; exit 0' INT TERM

set +e
wait -n "$controller_pid" "$executor_pid" "$podman_service_pid"
status=$?
set -e
stop
exit "$status"
