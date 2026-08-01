#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "usage: sudo $0 ARTIFACT_DIRECTORY [--start]" >&2
  exit 2
}

[[ $EUID -eq 0 ]] || { echo "run as root" >&2; exit 1; }
[[ $# -ge 1 && $# -le 2 ]] || usage

bundle=$1
start=${2:-}
[[ -d $bundle && -f $bundle/SHA256SUMS ]] || usage
[[ -z $start || $start == --start ]] || usage

for account in exposure-controller exposure-executor; do
  id "$account" >/dev/null 2>&1 || {
    echo "missing $account; provision the host with cloud-init first" >&2
    exit 1
  }
done

(
  cd "$bundle"
  sha256sum -c SHA256SUMS
)

install -d -m 0755 -o root -g root \
  /opt/exposure-benchmark/bin \
  /opt/exposure-benchmark/adapters \
  /opt/exposure-benchmark/sandbox
install -m 0755 -o root -g root \
  "$bundle/bin/benchmark-controller" /opt/exposure-benchmark/bin/benchmark-controller
install -m 0755 -o root -g root \
  "$bundle/bin/benchmark-executor" /opt/exposure-benchmark/bin/benchmark-executor
cp -a "$bundle/adapters/." /opt/exposure-benchmark/adapters/
cp -a "$bundle/sandbox/." /opt/exposure-benchmark/sandbox/
chown -R root:root /opt/exposure-benchmark

install -d -m 0750 -o root -g exposure-benchmark /var/lib/exposure-benchmark
python3.12 -m venv --clear /var/lib/exposure-benchmark/venv
/var/lib/exposure-benchmark/venv/bin/python -m pip install \
  --disable-pip-version-check --no-index --no-deps --require-hashes \
  --find-links "$bundle/python/wheels" \
  -r "$bundle/python/requirements.lock"
chown -R root:exposure-executor /var/lib/exposure-benchmark/venv

install -m 0644 -o root -g root \
  "$bundle/systemd/benchmark-controller.service" /etc/systemd/system/benchmark-controller.service
install -m 0644 -o root -g root \
  "$bundle/systemd/benchmark-executor.service" /etc/systemd/system/benchmark-executor.service
install -m 0644 -o root -g root \
  "$bundle/systemd/exposure-benchmark.conf" /usr/lib/tmpfiles.d/exposure-benchmark.conf
install -d -m 0750 -o root -g exposure-benchmark /etc/exposure-benchmark

if [[ ! -f /etc/exposure-benchmark/executor.env ]]; then
  controller_uid=$(id -u exposure-controller)
  sed "s/^BENCHMARK_CONTROLLER_UID=.*/BENCHMARK_CONTROLLER_UID=$controller_uid/" \
    "$bundle/systemd/executor.env.example" > /etc/exposure-benchmark/executor.env
  chown root:exposure-executor /etc/exposure-benchmark/executor.env
  chmod 0640 /etc/exposure-benchmark/executor.env
fi

systemd-tmpfiles --create /usr/lib/tmpfiles.d/exposure-benchmark.conf
systemctl daemon-reload
systemctl enable benchmark-executor.service benchmark-controller.service

arc_cache=/var/lib/exposure-benchmark/cache/arc-starter
if [[ -d $arc_cache/vendor/ARC-AGI-3-Agents ]]; then
  runuser -u exposure-executor -- env \
    HOME=/var/lib/exposure-benchmark/executor \
    XDG_RUNTIME_DIR=/run/exposure-benchmark/executor-runtime \
    podman build --pull=never \
      --label academy.harness.version=harness-2026-sprint-v2 \
      -t localhost/exposure-harness-arc:0.9.9 \
      -f /opt/exposure-benchmark/sandbox/Containerfile "$arc_cache"
else
  echo "ARC cache is not populated; sandbox image build deferred" >&2
fi

if [[ $start == --start ]]; then
  [[ -f /etc/exposure-benchmark/controller.env ]] || {
    echo "refusing to start without /etc/exposure-benchmark/controller.env" >&2
    exit 1
  }
  systemctl restart benchmark-executor.service benchmark-controller.service
fi

echo "benchmark artifacts installed and verified"
