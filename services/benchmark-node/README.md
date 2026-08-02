# Benchmark node

The benchmark node is the only part of Agentic Harness that moves to EC2. Academy and
Supabase remain the durable control plane; browsers never connect to this host.

## Trust boundary

- `benchmark-controller` owns the Academy worker credential, leases, Secrets Manager,
  the Bedrock-compatible Unix-socket gateway, frame buffering, health, and metrics.
- `benchmark-executor` runs as a different system user. It receives one run description
  and one short-lived model capability over bounded NDJSON, then launches the Python SDK
  adapters and rootless Podman/Bubblewrap sandboxes.
- The executor has no Academy, Supabase, AWS, or model-provider credential. Student
  runtime containers have no external network, Linux capabilities, or Podman socket.
- Python remains an adapter for ARC, Harbor, RAM-bench, Frontier, and Kaggle. Queue and
  credential ownership stays in Rust.

The controller-to-executor socket and each run-scoped model socket are group-readable only
by `exposure-benchmark`. Mutable data lives under `/var/lib/exposure-benchmark`; deployed
artifacts live under `/opt/exposure-benchmark`. Cerebras requests share the controller's
four-key pool; only the per-run socket capability crosses into the executor.

## Build and test

Run from the repository root:

```bash
cargo test --workspace --all-targets
python services/benchmark-node/adapters/contract_test.py
python services/benchmark-node/adapters/arc_game.py --self-check
docker build --target artifacts \
  --output type=local,dest=/tmp/exposure-benchmark-artifacts \
  -f services/benchmark-node/Dockerfile .
(cd /tmp/exposure-benchmark-artifacts && sha256sum -c SHA256SUMS)
```

The Docker target is an artifact envelope, not the production runtime. Production runs the
two binaries host-native under systemd; do not put rootless Podman inside an outer container.

## Local games

Set `ENVIRONMENT=DEV` in the repository `.env`. An EC2 instance is not required for any
local workflow.

```bash
# Free engine-only loop; no Academy, container, or model call.
make game-engine GAME=ls20 STEPS=50

# Optionally use another non-model local agent with the offline engine.
make game-engine GAME=ls20 ENGINE_SUBMISSION_REPO=/path/to/submission

# Production-like sandbox and model gateway. This makes real provider calls.
make game GAME=ls20 GAME_SECONDS=45

# Test another submission directly without pushing it to GitHub first.
make game GAME=ls20 SUBMISSION_REPO=/path/to/submission

# Academy + Rust controller + restricted executor on this machine.
make harness-e2e
```

The full stack uses the Supabase database configured in `.env`; use a development Supabase
project because the controller claims queued rows from that database. It defaults to
`127.0.0.1:3000`; use `BENCHMARK_DEV_ACADEMY_URL` and
`BENCHMARK_DEV_ACADEMY_BIND` only when a different local address is needed. Ctrl-C stops
all three processes.

## Host deployment

Provision `infra/ec2/stack.yaml` with autoscaling disabled, then transfer the artifact
directory over SSM or an approved private artifact store. On the initial image-builder
node:

```bash
sudo /path/to/artifacts/infra/install-artifacts.sh /path/to/artifacts
```

Cloud-init creates the controller env with the Academy URL, Secrets Manager identifier,
ASG name, lifecycle hook, and the instance's IMDSv2-derived ID. The executor env contains
paths and the controller UID only. Store `worker_token`, `bedrock_api_key`, and the four
`cerebras_api_keys` in the JSON secret described in `infra/ec2/README.md`.

Recreate these pinned caches on EC2 instead of copying local virtual environments:

- `/var/lib/exposure-benchmark/cache/arc-starter` at ARC starter commit
  `eeb1535404f321d280a8f9194bbc1d7aca5f05fc`, ARC Agents commit
  `10213de83f01df0ef4f0149ee9f8408dcc3772fb`, and `arc-agi==0.9.9`.
- `/var/lib/exposure-benchmark/cache/frontier-bench/frontier-bench` at dataset commit
  `3d694e919871dbf21ea5ff618782c99a3cb3663f` with Harbor `0.20.0` installed for the
  executor user.

The installer verifies `SHA256SUMS`, recreates the adapter venv from the hash-locked local
wheelhouse, installs systemd assets, and builds the sandbox image as the unprivileged
executor user. It does not start services unless passed `--start`. Capture a clean golden
AMI only after those artifacts and caches are verified, then enable the ASG in stages at
2, 3, and 5 nodes; the complete image workflow and quota gates are in
`infra/ec2/README.md`.

## Operations

Only loopback health and Prometheus endpoints exist:

```bash
curl --fail http://127.0.0.1:9108/healthz
curl --fail http://127.0.0.1:9108/metrics
journalctl -u benchmark-controller -u benchmark-executor
```

Frames use a four-batch non-blocking queue. A full queue, Academy error, or stale lease
drops visualization frames and increments `exposure_benchmark_frames_dropped_total`; it
never blocks the scoring event loop.

Frontier runs its five selected tasks concurrently. Agent and verifier phases are capped at
120 and 60 seconds respectively. Harbor execution, including cold environment builds, gets a
15-minute total budget; bounded process and container cleanup follows that cutoff.

Run `/opt`-independent `artifacts/infra/verify-isolation.sh` during a canary while student
containers are active.
Run `scripts/frame_load_test.py` against a dedicated test run to verify all 25 boards,
the five-active-game traffic shape, browser polling, and the 2.5-second
p95 visibility gate. The script requires test-only worker/session credentials through
environment variables and never terminally updates the run.

## Rollout and rollback

1. Deploy the backward-compatible Academy API and migration first.
2. Provision the one-node ASG with autoscaling disabled and build the verified worker AMI.
3. Start the Rust services for one canary cohort and run the load/isolation checks.
4. Enable autoscaling with a maximum of 2, then 3, then 5 after the model/frame gates pass.
5. Stop claiming new work in the old Python worker, let its active lease drain, then stop it.

Rollback is deliberately small: stop `benchmark-controller`, then restart the legacy
`runner.py` worker with its previous environment. Academy and Supabase need no data move or
rollback; queued and expired leased runs remain claimable through the same API.
