#!/usr/bin/env bash
set -Eeuo pipefail

[[ $EUID -eq 0 ]] || { echo "run as root on the dedicated image builder" >&2; exit 1; }
[[ $# -eq 1 ]] || { echo "usage: sudo $0 /var/tmp/exposure-source" >&2; exit 2; }

source_directory=$(realpath "$1")
[[ $source_directory == /var/tmp/exposure-source ]] || {
  echo "STOP: refusing to remove unexpected source path: $source_directory" >&2
  exit 1
}

metadata=/etc/exposure-benchmark/image-build.json
[[ -f $metadata && -f $source_directory/.deploy-commit ]] || {
  echo "STOP: image metadata or source commit marker is missing" >&2
  exit 1
}

deploy_sha=$(jq -er .source_commit "$metadata")
source_sha=$(<"$source_directory/.deploy-commit")
[[ $deploy_sha =~ ^[0-9a-f]{40}$ && $deploy_sha == "$source_sha" ]] || {
  echo "STOP: image/source commit mismatch" >&2
  exit 1
}

systemctl disable --now benchmark-controller.service benchmark-executor.service
[[ $(systemctl is-active benchmark-controller.service) == inactive ]]
[[ $(systemctl is-active benchmark-executor.service) == inactive ]]

executor_home=/var/lib/exposure-benchmark/executor
executor_runtime=/run/exposure-benchmark/executor-runtime
running_containers=$(runuser -u exposure-executor -- env \
  HOME="$executor_home" XDG_RUNTIME_DIR="$executor_runtime" \
  podman --cgroup-manager=cgroupfs ps -q)
[[ -z $running_containers ]] || {
  echo "STOP: rootless benchmark containers are still running" >&2
  exit 1
}

rm -f /etc/exposure-benchmark/controller.env
find /var/lib/exposure-benchmark/runs -mindepth 1 -maxdepth 1 -exec rm -rf -- {} +
rm -rf -- "/var/tmp/exposure-artifacts-$deploy_sha"

systemctl start docker.service
docker builder prune --all --force
docker image prune --all --force
systemctl disable --now docker.service docker.socket

apt-get clean
rm -f /tmp/exposure-source.tar.gz
cd /
rm -rf -- "$source_directory"

cloud-init clean --logs --machine-id
echo "READY: image sealed at source commit $deploy_sha"
