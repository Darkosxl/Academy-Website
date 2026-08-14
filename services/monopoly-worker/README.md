# AI Monopoly tournament fleet

This service validates team submissions and runs the frozen `ppo-plus-v2` tournament. The
Academy host owns the controller, artifact store, and optional sixth worker. Five named,
CPU-only Colab sessions provide the qualifying fleet when they meet the hardware floor.

## Team contract

Teams submit a public GitHub repository and a relative agent path (default `agent.py`). The
default branch is pinned to its current commit during validation. Git LFS is supported; the
resolved checkout, including LFS objects, must be no larger than 250 MiB.

The selected module exports:

```python
def choose_action(state, player_id, allowed_actions) -> int:
    return allowed_actions[0]
```

`state` contains `schema_version`, `ruleset_version`, the actor-relative 300-float `vector`, a
readable `board` snapshot, legal-action descriptions in `actions`, and a deterministic
`decision_seed`. It never contains the mutable engine.

An optional `requirements.txt` may contain at most 32 direct, wheel-only PyPI requirements.
Validation resolves the full wheel set once and records `requirements.lock`; games have no network
and perform no downloads. Startup has a 60-second limit, each genuine decision has a hard
two-second limit, and submitted agents are limited to 2 GiB of process memory.

See [`examples/minimal-agent/agent.py`](examples/minimal-agent/agent.py) for a baseline.

## Host setup

The Academy and controller must share the persistent artifact directory. Configure:

```text
MONOPOLY_SITE=https://academy.example
WORKER_TOKEN=the-same-secret-as-academy
MONOPOLY_ARTIFACT_DIR=/var/lib/exposure/monopoly-artifacts
```

The host needs Python 3, Git, Git LFS, and Docker. The Google Colab CLI is only needed if you
plan to run `--max-colab` above 0 — see "Optional: the Colab fleet" below. No GPU is requested or
used.

Run local checks and the no-allocation rollout preview:

```sh
python3 services/monopoly-worker/rl_monopoly_runner.py --selftest
python3 -m unittest services/monopoly-worker/test_rl_monopoly.py -v
python3 services/monopoly-worker/monopoly_controller.py --dry-run --max-colab 0
```

Start the controller with Colab disabled and games running directly on this host's Docker:

```sh
python3 services/monopoly-worker/monopoly_controller.py --max-colab 0 --host-workers 2
```

`--host-workers N` runs N `rl_monopoly_runner.py` processes concurrently on this host, each an
independent claim/play/report loop (`/api/worker/rl-monopoly/claim` uses row-locking, so they
never claim the same game). Each game's submitted agents still run inside the same locked-down
Docker sandbox described below — this only controls how many games run at once. Start at 1–2 and
raise it once you've watched real memory/CPU headroom on the box.

The host worker no longer needs 32 GiB RAM / 8 vCPU to qualify — that floor existed to decide
whether an ephemeral Colab VM was worth allocating, which doesn't apply to a fixed host. It now
only checks it isn't obviously starved (`MONOPOLY_MIN_RAM_BYTES`/`MONOPOLY_MIN_VCPUS`, default
2 GiB/1 vCPU, tunable via env). Real protection against overloading the box is:

- each submitted agent's Docker container: `--network none`, read-only rootfs, `--memory 2g`,
  `--cpus 1`, `--pids-limit 64`, no capabilities, `no-new-privileges` (`docker_agent` in
  `rl_monopoly_runner.py`);
- the systemd unit's own `MemoryHigh=`/`MemoryMax=`/`CPUQuota=`, sized for the rest of what runs
  on the box (see `systemd/ai-monopoly-controller.service`);
- `Restart=on-failure` — systemd restarts the whole controller (and therefore its host workers)
  if it dies, no separate watchdog process needed.

### Deploy on Dokploy (recommended — same pattern as `benchmark-node`)

`Dockerfile` + `compose.dokploy.yml` in this directory let Dokploy build and run the controller
the same way it already runs the Academy app and the benchmark worker: add a new Compose
application in Dokploy pointing at `services/monopoly-worker/compose.dokploy.yml`, set
`MONOPOLY_SITE` and `WORKER_TOKEN` (same value as the Academy app's own `WORKER_TOKEN`) in
Dokploy's env vars, and deploy. No SSH, no systemd, no manual venv — pushing to `main` and
redeploying in Dokploy is the whole update flow.

One thing Dokploy won't do for you: the controller and the Academy app are two separate Dokploy
apps, so `MONOPOLY_ARTIFACT_DIR` only actually works if both containers have the **same host
directory** bind-mounted at that path — the compose file already mounts
`/var/lib/exposure/monopoly-artifacts` on the host into itself; add the identical bind mount to
the Academy app in Dokploy's Mounts tab (its own `Dockerfile` doesn't declare one today), pointing
at `/app/var/monopoly-artifacts` with `MONOPOLY_ARTIFACT_DIR=/app/var/monopoly-artifacts` set in
its env vars. Without that, validation will build and smoke-test fine but approval will fail the
artifact-existence check on the Academy side.

The compose file also mounts `/var/run/docker.sock` — Docker-outside-of-Docker, so games run as
sibling containers on the host's real Docker daemon instead of needing Docker-in-Docker. That
socket grant is root-equivalent on the host; it's the same trust boundary the `benchmark-worker`
Dokploy app already runs under (`privileged: true` there), just narrower here (no privileged
flag, only the socket).

### Alternative: bare host, no Dokploy

If you're running this directly on a host instead of through Dokploy, install it as a systemd
unit:

```sh
services/monopoly-worker/systemd/install.sh
```

This creates the `exposure-monopoly` system user, symlinks this checkout to `/opt/exposure-academy`
(the unit's hardcoded path — no need to reclone anywhere), builds the venv, and installs the unit.
It won't start the service: fill in the real `WORKER_TOKEN` (must match the Academy service's own)
and `MONOPOLY_SITE` in `/etc/exposure-academy/monopoly-controller.env`, then:

```sh
sudo systemctl enable --now ai-monopoly-controller.service
```

The script is safe to re-run (e.g. after `git pull`, to pick up a requirements.txt or unit change).
On this path the Academy and controller share one filesystem, so `MONOPOLY_ARTIFACT_DIR` just
needs to be the same path in both — no bind-mount juggling needed, unlike the Dokploy path above.

The Academy HTTP service must not invoke this command or share its process. A normal systemd stop
sends `SIGTERM`; the controller's signal/finally cleanup terminates every host worker (and, if
Colab is enabled, stops all named Colab sessions) before the unit exits.

Server-hosted games run submitted agents in Docker with no network, a read-only root filesystem,
2 GiB memory, one CPU, 64 processes, no capabilities, and `no-new-privileges`. This is the same
sandbox used both for the one-off smoke test during submission validation and for every game a
host worker plays — nothing Colab-specific about it.

### Optional: the Colab fleet

The controller can still scale out to up to five named, CPU-only Colab sessions
(`ai-monopoly-1`..`ai-monopoly-5`) alongside the host workers — useful if `--host-workers` alone
isn't enough throughput. This needs the `colab` CLI on the host with credentials already
configured, and is entirely opt-in via `--max-colab` (0 disables it, the default is 5):

```sh
# single-session qualification smoke — makes one CPU allocation attempt for
# ai-monopoly-1, measures real RAM/cgroup CPU quota/machine shape remotely,
# and rejects it unless it has at least 32 GiB RAM and eight effective vCPUs
python3 services/monopoly-worker/monopoly_controller.py --smoke-colab

# only after that passes: qualify and exercise up to five sessions concurrently
python3 services/monopoly-worker/monopoly_controller.py --smoke-fleet --max-colab 5

# then run the controller with Colab enabled
python3 services/monopoly-worker/monopoly_controller.py --max-colab 5 --host-workers 2
```

Qualified Colab sessions use a per-artifact isolated virtual environment assembled from the
validated wheelhouse (not Docker — Colab sessions don't reliably offer a usable Docker daemon).
To enable it: on Dokploy, change the `CMD` args in `Dockerfile` (or override the container command
in Dokploy) to include `--max-colab 5`; on the bare-host path, update `ExecStart=` in
`systemd/ai-monopoly-controller.service`. Either way the `colab` CLI and its credentials need to be
present wherever the controller actually runs — inside the container on Dokploy, meaning the image
would need the CLI added to the `Dockerfile` too.

`monopoly_ctl.py` and `monopoly_runner.py` are compatibility entry points for the new controller
and leased worker respectively; neither contains the retired singleton/GPU protocol.
