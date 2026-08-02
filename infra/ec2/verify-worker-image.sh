#!/usr/bin/env bash
set -Eeuo pipefail

[[ $EUID -eq 0 ]] || { echo "run as root on the dedicated image builder" >&2; exit 1; }

adapter_python=/var/lib/exposure-benchmark/venv/bin/python
arc_python=/var/lib/exposure-benchmark/cache/arc-starter/.venv/bin/python
adapters=/opt/exposure-benchmark/adapters
arc_cache=/var/lib/exposure-benchmark/cache/arc-starter
executor_home=/var/lib/exposure-benchmark/executor
executor_runtime=/run/exposure-benchmark/executor-runtime
image=localhost/exposure-harness-arc:0.9.9

systemctl stop benchmark-controller.service benchmark-executor.service

for path in "$adapter_python" "$arc_python" "$adapters/contract_test.py" \
  "$adapters/arc_game.py" "$arc_cache/environment_files"; do
  [[ -e $path ]] || { echo "STOP: required image path is missing: $path" >&2; exit 1; }
done

HARNESS_ENV=executor "$adapter_python" "$adapters/contract_test.py"
"$adapter_python" "$adapters/arc_game.py" --self-check

runuser -u exposure-executor -- env HARNESS_ENV=executor ARC_CACHE="$arc_cache" ADAPTERS="$adapters" \
  "$arc_python" - <<'PY'
import os
import sys
import arc_agi
from arc_agi import OperationMode

sys.path.insert(0, os.environ["ADAPTERS"])
from runner import ARC_GAMES

arcade = arc_agi.Arcade(
    operation_mode=OperationMode.OFFLINE,
    environments_dir=os.path.join(os.environ["ARC_CACHE"], "environment_files"),
)
for game in ARC_GAMES:
    assert arcade.make(game) is not None
print(f"cached ARC engine ok: {len(ARC_GAMES)} games")
PY

runuser -u exposure-executor -- env \
  HOME="$executor_home" XDG_RUNTIME_DIR="$executor_runtime" \
  podman --cgroup-manager=cgroupfs image exists "$image"

runuser -u exposure-executor -- env \
  HOME="$executor_home" XDG_RUNTIME_DIR="$executor_runtime" \
  podman --cgroup-manager=cgroupfs run \
    --rm \
    --network=none \
    --cap-drop=all \
    --security-opt=no-new-privileges \
    --read-only \
    --tmpfs=/tmp:rw,noexec,nosuid,size=64m \
    "$image" \
    python -c 'import arc_agi, openai; print("sandbox imports ok")'

echo "READY: offline worker image verification passed"
