#!/usr/bin/env bash
set -Eeuo pipefail

usage() {
  echo "usage: sudo $0 SOURCE_DIRECTORY [EXPECTED_COMMIT]" >&2
  exit 2
}

[[ $EUID -eq 0 ]] || { echo "run as root on a dedicated builder" >&2; exit 1; }
[[ $# -ge 1 && $# -le 2 ]] || usage

source_directory=$(realpath "$1")
expected_sha=${2:-}
if [[ -e $source_directory/.git ]]; then
  deploy_sha=$(git -C "$source_directory" rev-parse HEAD)
  [[ -z $(git -C "$source_directory" status --porcelain --untracked-files=no) ]] || {
    echo "STOP: tracked source files are modified" >&2
    exit 1
  }
elif [[ -f $source_directory/.deploy-commit ]]; then
  deploy_sha=$(<"$source_directory/.deploy-commit")
else
  echo "STOP: source has neither Git metadata nor .deploy-commit" >&2
  exit 1
fi
[[ $deploy_sha =~ ^[0-9a-f]{40}$ ]] || { echo "STOP: invalid source commit" >&2; exit 1; }
[[ -z $expected_sha || $deploy_sha == "$expected_sha" ]] || {
  echo "STOP: source commit $deploy_sha does not match expected commit $expected_sha" >&2
  exit 1
}

for required_file in \
  Cargo.lock \
  services/benchmark-node/Dockerfile \
  services/benchmark-node/adapters/runner.py \
  infra/ec2/install-artifacts.sh \
  infra/ec2/verify-worker-image.sh; do
  [[ -f $source_directory/$required_file ]] || {
    echo "STOP: required source file is missing: $required_file" >&2
    exit 1
  }
done

for unit in benchmark-controller.service benchmark-executor.service; do
  systemctl disable --now "$unit" 2>/dev/null || true
done

for command in docker git jq podman python3.12 runuser sha256sum; do
  command -v "$command" >/dev/null || { echo "STOP: missing command: $command" >&2; exit 1; }
done
docker buildx version >/dev/null || { echo "STOP: Docker Buildx is unavailable" >&2; exit 1; }

cache=/var/lib/exposure-benchmark/cache
executor_home=/var/lib/exposure-benchmark/executor
executor_runtime=/run/exposure-benchmark/executor-runtime
artifacts=/var/tmp/exposure-artifacts-$deploy_sha
arc=$cache/arc-starter
agents=$arc/vendor/ARC-AGI-3-Agents
frontier_root=$cache/frontier-bench
frontier_repo=$frontier_root/repo
frontier_dataset=$frontier_root/frontier-bench
harbor=$executor_home/.local/share/uv/tools/harbor

arc_sha=eeb1535404f321d280a8f9194bbc1d7aca5f05fc
agents_sha=10213de83f01df0ef4f0149ee9f8408dcc3772fb
frontier_sha=3d694e919871dbf21ea5ff618782c99a3cb3663f
games=(ls20 vc33 ar25 cn04 s5i5 sp80 bp35 ft09 m0r0 re86 cd82 sb26 r11l)
frontier_tasks=(
  html-js-filter
  vllm-deepseek-streaming
  session-window-debug
  mvcc-lsm-compaction
  embedding-drift-monitor
)

[[ ! -e $artifacts ]] || { echo "STOP: artifact path already exists: $artifacts" >&2; exit 1; }
[[ ! -e $arc && ! -e $frontier_root && ! -e $harbor ]] || {
  echo "STOP: benchmark caches already exist; use a fresh dedicated builder" >&2
  exit 1
}

install -d -m 0750 -o root -g exposure-benchmark /var/lib/exposure-benchmark
install -d -m 0700 -o exposure-executor -g exposure-executor \
  "$cache" "$executor_home" "$executor_runtime"

run_executor() {
  runuser -u exposure-executor -- env \
    HOME="$executor_home" XDG_RUNTIME_DIR="$executor_runtime" "$@"
}

clone_exact() {
  local repository=$1 destination=$2 commit=$3
  run_executor git clone --filter=blob:none --no-checkout "$repository" "$destination"
  run_executor git -C "$destination" checkout --detach "$commit"
  [[ $(run_executor git -C "$destination" rev-parse HEAD) == "$commit" ]] || {
    echo "STOP: checkout verification failed for $destination" >&2
    exit 1
  }
}

cleanup_docker() {
  systemctl stop docker.service docker.socket 2>/dev/null || true
}
trap cleanup_docker EXIT

systemctl start docker.service
docker buildx build \
  --target artifacts \
  --output "type=local,dest=$artifacts" \
  -f "$source_directory/services/benchmark-node/Dockerfile" \
  "$source_directory"
(cd "$artifacts" && sha256sum -c SHA256SUMS)

# Docker's local exporter can create root-only files. The unprivileged executor
# must be able to read the pinned wheel bundle during offline environment setup.
chmod -R a+rX "$artifacts"
run_executor test -r "$artifacts/python/requirements.lock"

clone_exact \
  https://github.com/arcprize/ARC-AGI-3-Kaggle-Starter.git \
  "$arc" "$arc_sha"
install -d -m 0700 -o exposure-executor -g exposure-executor "$arc/vendor"
clone_exact \
  https://github.com/arcprize/ARC-AGI-3-Agents.git \
  "$agents" "$agents_sha"

run_executor python3.12 -m venv "$arc/.venv"
run_executor "$arc/.venv/bin/python" -m pip install \
  --disable-pip-version-check --no-index --no-deps --require-hashes \
  --find-links "$artifacts/python/wheels" \
  -r "$artifacts/python/requirements.lock"

games_csv=$(IFS=,; echo "${games[*]}")
run_executor env ARC_CACHE="$arc" ARC_GAMES="$games_csv" \
  "$arc/.venv/bin/python" - <<'PY'
import os
import arc_agi
from arc_agi import OperationMode

arcade = arc_agi.Arcade(
    operation_mode=OperationMode.NORMAL,
    environments_dir=os.path.join(os.environ["ARC_CACHE"], "environment_files"),
)
for game in os.environ["ARC_GAMES"].split(","):
    assert arcade.make(game) is not None
    print(f"ARC READY: {game}")
PY

install -d -m 0700 -o exposure-executor -g exposure-executor "$frontier_root"
clone_exact \
  https://github.com/harbor-framework/frontier-bench.git \
  "$frontier_repo" "$frontier_sha"
run_executor ln -s repo/tasks "$frontier_dataset"
for task in "${frontier_tasks[@]}"; do
  [[ -f $frontier_dataset/$task/task.toml ]] || {
    echo "STOP: Frontier task is missing: $task" >&2
    exit 1
  }
  echo "FRONTIER READY: $task"
done

install -d -m 0700 -o exposure-executor -g exposure-executor \
  "$executor_home/.local" \
  "$executor_home/.local/share" \
  "$executor_home/.local/share/containers" \
  "$executor_home/.local/share/uv" \
  "$(dirname "$harbor")"
run_executor python3.12 -m venv "$harbor"
run_executor "$harbor/bin/python" -m pip install \
  --disable-pip-version-check "harbor==0.20.0"
run_executor "$harbor/bin/harbor" --help >/dev/null

"$artifacts/infra/install-artifacts.sh" "$artifacts"
"$artifacts/infra/verify-worker-image.sh"

install -d -m 0755 -o root -g root /etc/exposure-benchmark
cat >/etc/exposure-benchmark/image-build.json <<EOF
{"source_commit":"$deploy_sha","arc_commit":"$arc_sha","agents_commit":"$agents_sha","frontier_commit":"$frontier_sha","harbor_version":"0.20.0"}
EOF
chmod 0644 /etc/exposure-benchmark/image-build.json

rm -f /etc/exposure-benchmark/controller.env
rm -rf /var/lib/exposure-benchmark/runs/*
systemctl disable --now benchmark-controller.service benchmark-executor.service

echo "READY: worker image prepared at source commit $deploy_sha"
