#!/usr/bin/env bash
set -euo pipefail

[[ $EUID -eq 0 ]] || { echo "run as root during an EC2 canary" >&2; exit 1; }

failed=0
pass() { echo "PASS: $*"; }
fail() { echo "FAIL: $*" >&2; failed=1; }

if runuser -u exposure-executor -- test -r /etc/exposure-benchmark/controller.env; then
  fail "executor can read controller configuration"
else
  pass "executor cannot read controller configuration"
fi

controller_uid=$(id -u exposure-controller)
configured_uid=$(sed -n 's/^BENCHMARK_CONTROLLER_UID=//p' /etc/exposure-benchmark/executor.env)
if [[ $configured_uid == "$controller_uid" ]]; then
  pass "executor peer-credential allowlist matches the controller UID"
else
  fail "executor peer-credential allowlist is missing or stale"
fi

if runuser -u exposure-executor -- test -w /run/exposure-benchmark/gateways; then
  fail "executor can replace controller gateway paths"
else
  pass "executor has traverse-only access to controller gateways"
fi
if runuser -u exposure-controller -- test -w /run/exposure-benchmark/executor; then
  fail "controller can replace the executor socket"
else
  pass "controller has traverse-only access to the executor socket directory"
fi
if [[ -S /run/exposure-benchmark/executor/executor.sock \
      && $(stat -c '%a:%G' /run/exposure-benchmark/executor/executor.sock) == 660:exposure-benchmark ]]; then
  pass "executor socket is shared only through exposure-benchmark"
else
  fail "executor socket mode or group is incorrect"
fi

for unit in benchmark-controller.service benchmark-executor.service; do
  if systemctl show "$unit" \
      -p NoNewPrivileges -p ProtectSystem -p ProtectHome -p CapabilityBoundingSet \
      | grep -q '^NoNewPrivileges=yes$'; then
    pass "$unit has systemd privilege hardening"
  else
    fail "$unit is missing expected systemd hardening"
  fi
done

if [[ $(systemctl show benchmark-controller.service -P ProtectControlGroups) == yes ]]; then
  pass "controller sees the cgroup hierarchy read-only"
else
  fail "controller cgroup protection is disabled"
fi
if [[ $(systemctl show benchmark-executor.service -P Delegate) == yes \
      && $(systemctl show benchmark-executor.service -P ProtectControlGroups) == no ]]; then
  pass "executor has only its delegated writable cgroup subtree"
else
  fail "executor cgroup delegation is incompatible with rootless Podman"
fi

if systemd-run --quiet --wait --collect --pipe \
    --uid=exposure-executor \
    --property=IPAddressDeny=169.254.0.0/16 \
    --property=IPAddressDeny=fe80::/10 \
    python3 -c 'import socket; socket.create_connection(("169.254.169.254", 80), 1)' \
    >/dev/null 2>&1; then
  fail "executor network policy allowed EC2 metadata"
else
  pass "executor network policy blocks EC2 metadata"
fi

mapfile -t containers < <(
  runuser -u exposure-executor -- env \
    HOME=/var/lib/exposure-benchmark/executor \
    XDG_RUNTIME_DIR=/run/exposure-benchmark/executor-runtime \
    podman ps -q
)

if [[ ${#containers[@]} -eq 0 ]]; then
  echo "SKIP: no running student containers; rerun during the canary workload" >&2
else
  for container in "${containers[@]}"; do
    inspect=$(runuser -u exposure-executor -- env \
      HOME=/var/lib/exposure-benchmark/executor \
      XDG_RUNTIME_DIR=/run/exposure-benchmark/executor-runtime \
      podman inspect "$container")
    name=$(jq -r '.[0].Name' <<<"$inspect")

    if jq -e '.[0].EffectiveCaps == [] and .[0].BoundingCaps == []' \
        >/dev/null <<<"$inspect"; then
      pass "$name has no effective or bounding capabilities"
    else
      fail "$name retains Linux capabilities"
    fi

    if jq -e '[.[0].Mounts[]?.Destination]
        | all(. != "/run/podman.sock" and . != "/run/docker.sock"
              and . != "/var/run/docker.sock")' >/dev/null <<<"$inspect"; then
      pass "$name has no Podman/Docker socket mount"
    else
      fail "$name can reach a container-engine socket"
    fi

    pid=$(jq -r '.[0].State.Pid' <<<"$inspect")
    if timeout 2 nsenter -t "$pid" -n \
        python3 -c 'import socket; socket.create_connection(("1.1.1.1", 443), 1)' \
        >/dev/null 2>&1; then
      fail "$name has external network access"
    else
      pass "$name cannot reach external HTTPS"
    fi
  done
fi

exit "$failed"
