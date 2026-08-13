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

The host needs Python 3, Git, Git LFS, Docker, and the Google Colab CLI. Colab credentials must
already be configured. No GPU is requested or used.

Run local checks and the no-allocation rollout preview:

```sh
python3 services/monopoly-worker/rl_monopoly_runner.py --selftest
python3 -m unittest services/monopoly-worker/test_rl_monopoly.py -v
python3 services/monopoly-worker/monopoly_controller.py --dry-run
```

Then perform the single-session qualification smoke:

```sh
python3 services/monopoly-worker/monopoly_controller.py --smoke-colab
```

The controller makes one CPU allocation attempt for `ai-monopoly-1`, measures actual system RAM,
cgroup CPU quota, and machine shape remotely, and stops the session unless it has at least 32 GiB
RAM and eight effective vCPUs. It always stops the named session in cleanup and verifies that no
named session remains.

Only after that succeeds, exercise the available qualified slots concurrently and verify cleanup:

```sh
python3 services/monopoly-worker/monopoly_controller.py --smoke-fleet --max-colab 5
```

Start the separate long-running controller only after that smoke passes:

```sh
python3 services/monopoly-worker/monopoly_controller.py --max-colab 5
```

For the Academy host, install
`systemd/ai-monopoly-controller.service` as a distinct systemd unit and copy
`systemd/monopoly-controller.env.example` to
`/etc/exposure-academy/monopoly-controller.env` with the production secret. The unit expects the
checkout at `/opt/exposure-academy`, a system user named `exposure-monopoly` with home
`/var/lib/exposure-monopoly`, membership in the `docker` group, and the shared artifact disk at
`/var/lib/exposure/monopoly-artifacts`. Enable it only after the smoke gate succeeds:

```sh
sudo python3 -m venv /opt/exposure-academy/.venv-monopoly
sudo /opt/exposure-academy/.venv-monopoly/bin/pip install \
  -r /opt/exposure-academy/services/monopoly-worker/requirements.txt
sudo systemctl daemon-reload
sudo systemctl enable --now ai-monopoly-controller.service
```

The Academy HTTP service must not invoke this command or share its process. A normal systemd stop
sends `SIGTERM`; the controller's signal/finally cleanup stops all five named sessions before the
unit exits.

The controller processes validation jobs sequentially, creates `ai-monopoly-1` through
`ai-monopoly-5` at most once per demand cycle, rejects undersized sessions immediately, and
continues with whatever qualified capacity is available. The host joins as worker six only when
its measured resources meet the same 32 GiB/eight-vCPU floor. Signal handling and final cleanup
stop and verify all five named sessions.

Server-hosted games run submitted agents in Docker with no network, a read-only root filesystem,
2 GiB memory, one CPU, 64 processes, no capabilities, and `no-new-privileges`. Qualified Colab
workers use a per-artifact isolated virtual environment assembled entirely from the validated
wheelhouse.

`monopoly_ctl.py` and `monopoly_runner.py` are compatibility entry points for the new controller
and leased worker respectively; neither contains the retired singleton/GPU protocol.
