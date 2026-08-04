#!/usr/bin/env bash
set -Eeuo pipefail

[[ $EUID -eq 0 ]] || { echo "run as root on a launched worker" >&2; exit 1; }
[[ $# -eq 1 && $1 =~ ^[0-9a-f]{40}$ ]] || {
  echo "usage: sudo $0 EXPECTED_COMMIT" >&2
  exit 2
}

expected_sha=$1
metadata=/etc/exposure-benchmark/image-build.json
controller_env=/etc/exposure-benchmark/controller.env

# SSM commands run from a root-only orchestration directory by default.
cd /

for unit in benchmark-executor.service benchmark-controller.service; do
  systemctl is-enabled --quiet "$unit" || { echo "STOP: $unit is disabled" >&2; exit 1; }
  systemctl is-active --quiet "$unit" || { echo "STOP: $unit is inactive" >&2; exit 1; }
done

actual_sha=$(jq -er .source_commit "$metadata")
[[ $actual_sha == "$expected_sha" ]] || {
  echo "STOP: running commit $actual_sha does not match $expected_sha" >&2
  exit 1
}

curl --fail --silent --show-error --max-time 5 http://127.0.0.1:9108/healthz >/dev/null

/var/lib/exposure-benchmark/venv/bin/python - "$controller_env" <<'PY'
import json
import sys

import boto3
from dotenv import dotenv_values

config = dotenv_values(sys.argv[1])
secret = json.loads(
    boto3.client("secretsmanager", region_name=config["AWS_REGION"])
    .get_secret_value(SecretId=config["BENCHMARK_SECRET_ID"])["SecretString"]
)
assert set(secret) == {
    "worker_token",
    "bedrock_api_key",
    "cerebras_api_keys",
    "deepinfra_api_key",
}, "unexpected secret schema"
assert len(secret["worker_token"]) >= 32, "worker token is too short"
assert len(secret["bedrock_api_key"]) >= 20, "Bedrock API key is too short"
assert len(secret["cerebras_api_keys"]) == 4, "exactly four Cerebras API keys are required"
assert len(set(secret["cerebras_api_keys"])) == 4, "Cerebras API keys must be distinct"
assert all(len(key) >= 20 for key in secret["cerebras_api_keys"]), "Cerebras API key is too short"
assert len(secret["deepinfra_api_key"]) >= 20, "DeepInfra API key is too short"
PY

runuser -u exposure-executor -- env \
  HOME=/var/lib/exposure-benchmark/executor \
  XDG_RUNTIME_DIR=/run/exposure-benchmark/executor-runtime \
  podman --cgroup-manager=cgroupfs image exists localhost/exposure-harness-arc:0.9.9

echo "READY: running worker verified at source commit $actual_sha"
