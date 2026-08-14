#!/usr/bin/env python3
"""Leased ppo-plus-v2 tournament worker for the Academy host or a CPU Colab VM."""

from __future__ import annotations

import argparse
from dataclasses import dataclass, field
import hashlib
import json
import os
from pathlib import Path
import random
import resource
import select
import shutil
import signal
import subprocess
import sys
import tarfile
import threading
import time
import urllib.error
import urllib.request
import uuid

from monopoly_game_engine import (
    TheBlocker,
    TheBuilder,
    TheDealMaker,
    TheGambler,
    TheHoarder,
    TheRailBaron,
    action_to_description,
    build_state_vector,
)
from monopoly_game_engine.actions import ActionType
from shared_game import SharedGame


ROOT = Path(__file__).resolve().parent
SITE = os.environ.get("MONOPOLY_SITE", "http://127.0.0.1:3000").rstrip("/")
WORKER_TOKEN = os.environ.get("WORKER_TOKEN", "")
WORKER_ID = os.environ.get("MONOPOLY_WORKER_ID", f"worker-{os.getpid()}")
WORKER_KIND = os.environ.get("MONOPOLY_WORKER_KIND", "other")
SESSION_NAME = os.environ.get("MONOPOLY_SESSION_NAME")
BACKEND = os.environ.get(
    "MONOPOLY_AGENT_BACKEND", "venv" if WORKER_KIND == "colab" else "docker"
)
CACHE_DIR = Path(os.environ.get("MONOPOLY_CACHE_DIR", ROOT / ".artifact-cache"))
AGENT_RUNNER = ROOT / "agent_image" / "runner.py"

RULESET_VERSION = "ppo-plus-v2"
SCHEMA_VERSION = "exposure-agent-v1"
MAX_ROUNDS = 200
ACTION_CEILING = 50_000
PLAY_LIMIT_US = 600_000_000
DECISION_TIMEOUT_SECONDS = 2.0
STARTUP_TIMEOUT_SECONDS = 60.0
AGENT_MEMORY_BYTES = 2 * 1024**3
EVENT_BATCH_SIZE = 25
POLL_SECONDS = 5

# Colab sessions are hardware-gated remotely (colab_preflight.py) before this
# script ever runs there. This is the floor for a locally-run worker (Academy
# host or any other machine started directly); it's low by default because
# Docker's own per-container caps (see docker_agent) and the host process's
# own cgroup limits are the real backstop against overload, not a
# self-reported RAM/CPU heuristic sized for provisioning a Colab VM.
MIN_RAM_BYTES = int(os.environ.get("MONOPOLY_MIN_RAM_BYTES", str(2 * 1024**3)))
MIN_VCPUS = float(os.environ.get("MONOPOLY_MIN_VCPUS", "1"))

FIXED_AGENTS = {
    "hoarder": TheHoarder,
    "dealmaker": TheDealMaker,
    "gambler": TheGambler,
    "builder": TheBuilder,
    "blocker": TheBlocker,
    "railbaron": TheRailBaron,
}


def agent_image_fingerprint() -> str:
    digest = hashlib.sha256()
    for path in (ROOT / "agent_image" / "Dockerfile", AGENT_RUNNER):
        digest.update(path.read_bytes())
    return digest.hexdigest()


def log(message: str) -> None:
    print(f"[{time.strftime('%H:%M:%S')}] {message}", flush=True)


def splitmix64(value: int) -> int:
    value = (value + 0x9E3779B97F4A7C15) & ((1 << 64) - 1)
    value = ((value ^ (value >> 30)) * 0xBF58476D1CE4E5B9) & ((1 << 64) - 1)
    value = ((value ^ (value >> 27)) * 0x94D049BB133111EB) & ((1 << 64) - 1)
    return value ^ (value >> 31)


class ApiError(RuntimeError):
    def __init__(self, status: int | None, message: str):
        super().__init__(message)
        self.status = status


class ApiClient:
    def __init__(self, site: str = SITE, token: str = WORKER_TOKEN):
        self.site = site.rstrip("/")
        self.token = token

    def request(
        self, path: str, body: dict | None = None, timeout: float = 60
    ) -> dict | None:
        request = urllib.request.Request(
            self.site + path,
            data=None
            if body is None
            else json.dumps(body, separators=(",", ":")).encode(),
            headers={
                "X-Worker-Token": self.token,
                "Content-Type": "application/json",
                "Accept": "application/json",
                "User-Agent": "exposure-monopoly-worker/1",
            },
            method="GET" if body is None else "POST",
        )
        try:
            with urllib.request.urlopen(request, timeout=timeout) as response:
                raw = response.read()
                return json.loads(raw) if raw else None
        except urllib.error.HTTPError as error:
            detail = error.read(1000).decode(errors="replace")
            raise ApiError(error.code, detail or f"HTTP {error.code}") from error
        except (OSError, TimeoutError) as error:
            raise ApiError(None, str(error)) from error

    def download(self, path: str, destination: Path) -> None:
        request = urllib.request.Request(
            self.site + path,
            headers={
                "X-Worker-Token": self.token,
                "User-Agent": "exposure-monopoly-worker/1",
            },
        )
        try:
            with (
                urllib.request.urlopen(request, timeout=180) as response,
                open(destination, "wb") as output,
            ):
                shutil.copyfileobj(response, output, length=1024 * 1024)
        except urllib.error.HTTPError as error:
            raise ApiError(
                error.code, f"artifact download returned HTTP {error.code}"
            ) from error
        except (OSError, TimeoutError) as error:
            raise ApiError(None, str(error)) from error


def system_ram_bytes() -> int:
    try:
        for line in Path("/proc/meminfo").read_text().splitlines():
            if line.startswith("MemTotal:"):
                return int(line.split()[1]) * 1024
    except OSError:
        pass
    return 0


def effective_vcpus() -> float:
    affinity = getattr(os, "sched_getaffinity", None)
    host = float(len(affinity(0)) if affinity else (os.cpu_count() or 0))
    try:
        quota, period = Path("/sys/fs/cgroup/cpu.max").read_text().split()
        if quota != "max":
            host = min(host, int(quota) / int(period))
    except (OSError, ValueError):
        try:
            quota = int(Path("/sys/fs/cgroup/cpu/cpu.cfs_quota_us").read_text())
            period = int(Path("/sys/fs/cgroup/cpu/cpu.cfs_period_us").read_text())
            if quota > 0 and period > 0:
                host = min(host, quota / period)
        except (OSError, ValueError):
            pass
    return host


def machine_shape() -> str:
    values = []
    for path in [
        Path("/sys/devices/virtual/dmi/id/product_name"),
        Path("/sys/devices/virtual/dmi/id/product_version"),
    ]:
        try:
            value = path.read_text().strip()
            if value and value not in values:
                values.append(value)
        except OSError:
            pass
    if os.environ.get("COLAB_RELEASE_TAG"):
        values.append(f"colab:{os.environ['COLAB_RELEASE_TAG']}")
    return " · ".join(values)[:200] or sys.platform


def resource_payload(
    status: str, *, reason: str | None = None, game_id: str | None = None
) -> dict:
    return {
        "worker_id": WORKER_ID,
        "kind": WORKER_KIND,
        "session_name": SESSION_NAME,
        "status": status,
        "ram_bytes": system_ram_bytes(),
        "effective_vcpus": effective_vcpus(),
        "machine_shape": machine_shape(),
        "preflight_reason": reason,
        "current_game_id": game_id,
    }


def report_resource(
    client: ApiClient,
    status: str,
    *,
    reason: str | None = None,
    game_id: str | None = None,
) -> None:
    client.request(
        "/api/worker/rl-monopoly/resource",
        resource_payload(status, reason=reason, game_id=game_id),
    )


class AgentCallError(RuntimeError):
    def __init__(self, kind: str, message: str):
        super().__init__(message)
        self.kind = kind


def _limit_agent() -> None:
    os.setsid()
    resource.setrlimit(resource.RLIMIT_AS, (AGENT_MEMORY_BYTES, AGENT_MEMORY_BYTES))
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    resource.setrlimit(resource.RLIMIT_NOFILE, (64, 64))


class AgentProcess:
    """Persistent JSON-lines process; stdout is protocol-only and stderr is private."""

    def __init__(
        self,
        command: list[str],
        *,
        environment: dict[str, str] | None = None,
        docker_name: str | None = None,
        cwd: Path | None = None,
    ):
        self.command = command
        self.environment = environment
        self.docker_name = docker_name
        self.stderr_buffer = bytearray()
        self.stderr_lock = threading.Lock()
        self.buffer = b""
        try:
            self.process = subprocess.Popen(
                command,
                stdin=subprocess.PIPE,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                env=environment,
                cwd=cwd,
                preexec_fn=None if docker_name else _limit_agent,
            )
        except (OSError, subprocess.SubprocessError) as error:
            raise AgentCallError("startup", str(error)) from error
        self.stderr_thread = threading.Thread(
            target=self._drain_stderr,
            name=f"agent-stderr-{self.process.pid}",
            daemon=True,
        )
        self.stderr_thread.start()
        try:
            ready = self._read_line(STARTUP_TIMEOUT_SECONDS, "startup")
        except Exception:
            self.close()
            raise
        if ready != {"ready": True}:
            self.close()
            raise AgentCallError("startup", f"invalid readiness response: {ready!r}")

    def _drain_stderr(self) -> None:
        assert self.process.stderr is not None
        fd = self.process.stderr.fileno()
        try:
            while chunk := os.read(fd, 4096):
                with self.stderr_lock:
                    self.stderr_buffer.extend(chunk)
                    if len(self.stderr_buffer) > 8192:
                        del self.stderr_buffer[:-8192]
        except OSError:
            pass

    def _stderr_tail(self) -> str:
        with self.stderr_lock:
            return bytes(self.stderr_buffer[-4000:]).decode(errors="replace")

    def _read_line(self, timeout: float, kind: str) -> dict:
        assert self.process.stdout is not None
        deadline = time.monotonic() + timeout
        fd = self.process.stdout.fileno()
        while time.monotonic() < deadline:
            if self.process.poll() is not None:
                raise AgentCallError(
                    kind,
                    f"agent exited ({self.process.returncode}): {self._stderr_tail()}",
                )
            ready, _, _ = select.select(
                [fd], [], [], max(0.0, deadline - time.monotonic())
            )
            if not ready:
                break
            chunk = os.read(fd, 4096)
            if not chunk:
                raise AgentCallError(
                    kind, f"agent closed stdout: {self._stderr_tail()}"
                )
            self.buffer += chunk
            if len(self.buffer) > 64 * 1024:
                raise AgentCallError(kind, "agent response exceeded 64 KiB")
            if b"\n" not in self.buffer:
                continue
            raw, self.buffer = self.buffer.split(b"\n", 1)
            try:
                value = json.loads(raw)
            except json.JSONDecodeError as error:
                raise AgentCallError(
                    kind, f"invalid JSON response: {raw[:200]!r}"
                ) from error
            if not isinstance(value, dict):
                raise AgentCallError(kind, "agent response must be a JSON object")
            return value
        raise AgentCallError("timeout", f"agent exceeded {timeout:.3f}s")

    def choose_action(
        self, state: dict, player_id: int, allowed_actions: list[int]
    ) -> int:
        if self.process.poll() is not None:
            raise AgentCallError("crash", f"agent exited: {self._stderr_tail()}")
        assert self.process.stdin is not None
        payload = (
            json.dumps(
                {
                    "state": state,
                    "player_id": player_id,
                    "allowed_actions": allowed_actions,
                },
                separators=(",", ":"),
            ).encode()
            + b"\n"
        )
        try:
            self.process.stdin.write(payload)
            self.process.stdin.flush()
        except (BrokenPipeError, OSError) as error:
            raise AgentCallError(
                "crash", f"agent pipe failed: {self._stderr_tail()}"
            ) from error
        response = self._read_line(DECISION_TIMEOUT_SECONDS, "crash")
        if response.get("error"):
            raise AgentCallError("crash", str(response["error"])[:1000])
        action = response.get("action")
        if (
            isinstance(action, bool)
            or not isinstance(action, int)
            or action not in allowed_actions
        ):
            raise AgentCallError(
                "illegal", f"illegal action {action!r}; allowed={allowed_actions}"
            )
        return action

    def close(self) -> str:
        if self.process.poll() is None:
            try:
                if self.docker_name:
                    self.process.terminate()
                else:
                    os.killpg(self.process.pid, signal.SIGTERM)
                self.process.wait(timeout=2)
            except (OSError, subprocess.TimeoutExpired):
                try:
                    if self.docker_name:
                        self.process.kill()
                    else:
                        os.killpg(self.process.pid, signal.SIGKILL)
                    self.process.wait(timeout=2)
                except (OSError, subprocess.TimeoutExpired):
                    pass
        if self.docker_name:
            subprocess.run(
                ["docker", "rm", "-f", self.docker_name],
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
                timeout=10,
                check=False,
            )
        for pipe in (self.process.stdin, self.process.stdout, self.process.stderr):
            if pipe is not None:
                try:
                    pipe.close()
                except OSError:
                    pass
        self.stderr_thread.join(timeout=2)
        return self._stderr_tail()


def _safe_archive_member(member: tarfile.TarInfo) -> bool:
    path = Path(member.name)
    return (
        not path.is_absolute()
        and ".." not in path.parts
        and not member.issym()
        and not member.islnk()
        and (member.isfile() or member.isdir())
    )


def cache_artifact(client: ApiClient, sha256: str) -> Path:
    if len(sha256) != 64 or any(
        character not in "0123456789abcdef" for character in sha256
    ):
        raise RuntimeError("claim supplied an invalid artifact hash")
    CACHE_DIR.mkdir(parents=True, exist_ok=True)
    archive = CACHE_DIR / f"{sha256}.tar.gz"
    root = CACHE_DIR / sha256
    ready = root / ".ready"
    if ready.is_file():
        return root
    temporary_archive = archive.with_suffix(".tmp")

    def digest(path: Path) -> str:
        value = hashlib.sha256()
        with open(path, "rb") as source:
            for chunk in iter(lambda: source.read(1024 * 1024), b""):
                value.update(chunk)
        return value.hexdigest()

    if not archive.is_file() or digest(archive) != sha256:
        temporary_archive.unlink(missing_ok=True)
        client.download(f"/api/worker/rl-monopoly/artifact/{sha256}", temporary_archive)
        if digest(temporary_archive) != sha256:
            temporary_archive.unlink(missing_ok=True)
            raise RuntimeError("artifact hash mismatch")
        os.replace(temporary_archive, archive)
    temporary_root = CACHE_DIR / f".{sha256}-{os.getpid()}"
    shutil.rmtree(temporary_root, ignore_errors=True)
    temporary_root.mkdir()
    with tarfile.open(archive, "r:gz") as bundle:
        members = bundle.getmembers()
        if not members or any(not _safe_archive_member(member) for member in members):
            raise RuntimeError("artifact contains an unsafe member")
        bundle.extractall(temporary_root, members=members, filter="data")
    metadata = json.loads((temporary_root / "metadata.json").read_text())
    if metadata.get("artifact_schema") != 1:
        raise RuntimeError("unsupported artifact schema")
    if BACKEND == "venv":
        venv = temporary_root / "venv"
        subprocess.run(
            [sys.executable, "-m", "venv", str(venv)], check=True, timeout=120
        )
        lock = temporary_root / "requirements.lock"
        if lock.read_text().strip():
            subprocess.run(
                [
                    str(venv / "bin" / "python"),
                    "-m",
                    "pip",
                    "install",
                    "--no-index",
                    "--only-binary=:all:",
                    f"--find-links={temporary_root / 'wheelhouse'}",
                    "--requirement",
                    str(lock),
                ],
                check=True,
                timeout=300,
            )
    (temporary_root / ".ready").write_text("ok\n")
    if root.exists():
        shutil.rmtree(temporary_root)
    else:
        os.replace(temporary_root, root)
    return root


def docker_agent(sha256: str, agent_path: str) -> AgentProcess:
    name = f"exposure-monopoly-{os.getpid()}-{uuid.uuid4().hex[:8]}"
    tag = f"exposure-monopoly-agent:{sha256}"
    command = [
        "docker",
        "run",
        "--rm",
        "-i",
        "--name",
        name,
        "--network",
        "none",
        "--read-only",
        "--tmpfs",
        "/tmp:rw,noexec,nosuid,size=64m",
        "--memory",
        "2g",
        "--memory-swap",
        "2g",
        "--cpus",
        "1",
        "--pids-limit",
        "64",
        "--ulimit",
        "nofile=64:64",
        "--cap-drop",
        "ALL",
        "--security-opt",
        "no-new-privileges=true",
        "--user",
        "10001:10001",
        "--log-driver",
        "none",
        "-e",
        f"EXPOSURE_AGENT_PATH={agent_path}",
        tag,
    ]
    return AgentProcess(command, docker_name=name)


def ensure_docker_image(client: ApiClient, sha256: str) -> None:
    """Rebuild a validated image from its archive after a host/image-store replacement."""
    tag = f"exposure-monopoly-agent:{sha256}"
    runtime = agent_image_fingerprint()
    inspected = subprocess.run(
        [
            "docker",
            "image",
            "inspect",
            "--format",
            '{{ index .Config.Labels "exposure.runtime" }}',
            tag,
        ],
        capture_output=True,
        text=True,
    )
    if inspected.returncode == 0 and inspected.stdout.strip() == runtime:
        return
    root = cache_artifact(client, sha256)
    subprocess.run(
        [
            "docker",
            "build",
            "--pull=false",
            "--network=none",
            "--tag",
            tag,
            "--label",
            f"exposure.artifact={sha256}",
            "--label",
            f"exposure.runtime={runtime}",
            "--file",
            str(ROOT / "agent_image" / "Dockerfile"),
            str(root),
        ],
        check=True,
        timeout=1200,
    )


def venv_agent(root: Path, agent_path: str) -> AgentProcess:
    home = root / "agent-home"
    temporary = root / "agent-tmp"
    home.mkdir(exist_ok=True)
    temporary.mkdir(exist_ok=True)
    environment = {
        "PATH": f"{root / 'venv' / 'bin'}:/usr/local/bin:/usr/bin:/bin",
        "HOME": str(home),
        "TMPDIR": str(temporary),
        "LANG": os.environ.get("LANG", "C.UTF-8"),
        "EXPOSURE_SUBMISSION_DIR": str(root / "submission"),
        "EXPOSURE_AGENT_PATH": agent_path,
        "PYTHONNOUSERSITE": "1",
        "PYTHONDONTWRITEBYTECODE": "1",
    }
    return AgentProcess(
        [str(root / "venv" / "bin" / "python"), "-I", str(root / "agent_runner.py")],
        environment=environment,
        cwd=root / "submission",
    )


@dataclass
class SeatRuntime:
    seat: int
    label: str
    entry_id: str | None
    bot_key: str | None
    artifact_sha256: str | None
    agent_path: str | None
    factory: object | None = None
    process: AgentProcess | None = None
    fixed: object | None = None
    strikes: int = 0
    decision_count: int = 0
    decision_total_us: int = 0
    decision_min_us: int | None = None
    decision_max_us: int | None = None
    disqualified: bool = False
    replacement_bot: str | None = None
    logs: list[str] = field(default_factory=list)

    @property
    def submitted(self) -> bool:
        return self.entry_id is not None

    def record_decision(self, duration_us: int) -> None:
        self.decision_count += 1
        self.decision_total_us += duration_us
        self.decision_min_us = (
            duration_us
            if self.decision_min_us is None
            else min(self.decision_min_us, duration_us)
        )
        self.decision_max_us = (
            duration_us
            if self.decision_max_us is None
            else max(self.decision_max_us, duration_us)
        )

    def close(self) -> None:
        if self.process is not None:
            tail = self.process.close().strip()
            if tail:
                self.logs.append(tail)
            self.process = None

    def restart(self) -> None:
        self.close()
        if self.factory is None:
            raise RuntimeError("submitted seat has no process factory")
        self.process = self.factory()

    def replace_with_bot(self) -> None:
        self.close()
        self.disqualified = True
        self.replacement_bot = list(FIXED_AGENTS)[self.seat % len(FIXED_AGENTS)]
        self.fixed = FIXED_AGENTS[self.replacement_bot](self.seat)

    def runtime_log(self) -> str | None:
        live = self.process._stderr_tail() if self.process is not None else ""
        text = "\n".join([*self.logs, live]).strip()
        return text[-4000:] or None


def prepare_seats(job: dict, client: ApiClient) -> list[SeatRuntime]:
    seats: list[SeatRuntime] = []
    try:
        for row in job["seats"]:
            seat = int(row["player_id"])
            runtime = SeatRuntime(
                seat=seat,
                label=str(row["label"]),
                entry_id=row.get("entry_id"),
                bot_key=row.get("bot_key"),
                artifact_sha256=row.get("artifact_sha256"),
                agent_path=row.get("agent_path"),
            )
            if runtime.submitted:
                if not runtime.artifact_sha256 or not runtime.agent_path:
                    raise RuntimeError("submitted seat is missing artifact metadata")
                if BACKEND == "docker":
                    ensure_docker_image(client, runtime.artifact_sha256)
                    runtime.factory = (
                        lambda sha=runtime.artifact_sha256,
                        path=runtime.agent_path: docker_agent(sha, path)
                    )
                elif BACKEND == "venv":
                    root = cache_artifact(client, runtime.artifact_sha256)
                    runtime.factory = (
                        lambda root=root, path=runtime.agent_path: venv_agent(
                            root, path
                        )
                    )
                else:
                    raise RuntimeError(f"unknown agent backend: {BACKEND}")
                runtime.restart()
            else:
                if runtime.bot_key not in FIXED_AGENTS:
                    raise RuntimeError(f"unknown filler bot: {runtime.bot_key!r}")
                runtime.fixed = FIXED_AGENTS[runtime.bot_key](seat)
            seats.append(runtime)
        if sorted(seat.seat for seat in seats) != [0, 1, 2, 3]:
            raise RuntimeError("claim does not contain exactly four physical seats")
        return sorted(seats, key=lambda seat: seat.seat)
    except Exception:
        for runtime in seats:
            runtime.close()
        raise


def build_snapshot(env) -> dict:
    return {
        "round": env.round,
        "phase": env.phase,
        "current_turn": env.whose_turn(),
        "active_player": env.active_player_id(),
        "has_rolled": env.has_rolled,
        "last_dice": list(env.last_dice) if env.last_dice else None,
        "done": env.done,
        "houses_available": env.houses_available,
        "hotels_available": env.hotels_available,
        "auction": None
        if env.phase != "auction"
        else {
            "property_id": env.auction_property_id,
            "bidders": env.auction_bidders,
            "current_bidder": env.auction_current_pid,
            "high_bid": env.auction_high_bid,
            "high_bidder": env.auction_high_bidder,
        },
        "players": [
            {
                "player_id": player.player_id,
                "cash": player.cash,
                "position": player.position,
                "in_jail": player.in_jail,
                "jail_turns": player.jail_turns,
                "gooj_card": player.gooj_card,
                "bankrupt": player.bankrupt,
                "net_worth": player.net_worth(),
                "properties": [prop.square_id for prop in player.properties],
            }
            for player in env.players
        ],
        "properties": [
            {
                "square_id": square,
                "owner": prop.owner,
                "mortgaged": prop.mortgaged,
                "houses": prop.houses,
                "is_monopoly": prop.is_monopoly,
            }
            for square, prop in env.properties.items()
        ],
    }


def decision_state(env, player_id: int, allowed: list[int], decision_seed: int) -> dict:
    vector = build_state_vector(
        env.players, env.properties, agent_id=player_id, env=env
    )
    if tuple(vector.shape) != (300,):
        raise RuntimeError(
            f"engine returned observation shape {vector.shape}, expected (300,)"
        )
    return {
        "schema_version": SCHEMA_VERSION,
        "ruleset_version": RULESET_VERSION,
        "vector": vector.tolist(),
        "board": build_snapshot(env),
        "actions": {str(action): action_to_description(action) for action in allowed},
        "decision_seed": decision_seed,
    }


def deterministic_fallback(allowed: list[int], decision_seed: int) -> int:
    return random.Random(decision_seed).choice(sorted(allowed))


def promoted_winner(seats: list[SeatRuntime], env) -> SeatRuntime | None:
    eligible = [seat for seat in seats if seat.submitted and not seat.disqualified]
    if not eligible:
        return None
    return max(
        eligible, key=lambda seat: (env.players[seat.seat].net_worth(), -seat.seat)
    )


def slowest_eligible(seats: list[SeatRuntime]) -> SeatRuntime | None:
    eligible = [
        seat
        for seat in seats
        if seat.submitted and not seat.disqualified and seat.decision_count > 0
    ]
    if not eligible:
        return None
    return max(
        eligible,
        key=lambda seat: (seat.decision_total_us / seat.decision_count, -seat.seat),
    )


def register_strike(runtime: SeatRuntime) -> bool:
    """Record one failed genuine decision; return whether the third strike replaced it."""
    runtime.strikes += 1
    if runtime.strikes >= 3:
        runtime.replace_with_bot()
        return True
    return False


class LeaseKeeper:
    def __init__(self, client: ApiClient, lease: dict):
        self.client = client
        self.lease = lease
        self.stop_event = threading.Event()
        self.error: Exception | None = None
        self.thread = threading.Thread(
            target=self._run, name="monopoly-heartbeat", daemon=True
        )

    def _run(self) -> None:
        while not self.stop_event.wait(30):
            try:
                self.client.request(
                    "/api/worker/rl-monopoly/heartbeat", self.lease, timeout=30
                )
            except (
                Exception
            ) as error:  # surfaced synchronously at the next action boundary
                self.error = error
                self.stop_event.set()

    def __enter__(self):
        self.thread.start()
        return self

    def check(self) -> None:
        if self.error is not None:
            raise self.error

    def __exit__(self, *_):
        self.stop_event.set()
        self.thread.join(timeout=2)


def validate_job(job: dict) -> None:
    expected = {
        "ruleset_version": RULESET_VERSION,
        "schema_version": SCHEMA_VERSION,
        "max_rounds": MAX_ROUNDS,
        "action_ceiling": ACTION_CEILING,
        "play_limit_us": PLAY_LIMIT_US,
    }
    for key, value in expected.items():
        if job.get(key) != value:
            raise RuntimeError(f"claim {key}={job.get(key)!r}, expected {value!r}")


def drive_game(job: dict, client: ApiClient, *, upload_events: bool = True) -> dict:
    validate_job(job)
    lease = {
        "worker_id": WORKER_ID,
        "game_id": job["id"],
        "attempt_no": job["attempt_no"],
        "lease_id": job["lease_id"],
    }
    with (
        LeaseKeeper(client, lease) if upload_events else _NullKeeper()
    ) as preparation_keeper:
        seats = prepare_seats(job, client)
        preparation_keeper.check()
    agents_ready_ns = time.monotonic_ns()
    game = SharedGame.new(int(job["seed"]), MAX_ROUNDS)
    env = game.env
    events: list[dict] = []
    sequence = 0
    excluded_ns = 0
    start_ns = agents_ready_ns
    end_reason = "normal"
    play_duration_us: int | None = None

    def active_us() -> int:
        return max(0, (time.monotonic_ns() - start_ns - excluded_ns) // 1000)

    def flush() -> None:
        nonlocal excluded_ns, events
        if not upload_events or not events:
            events = []
            return
        upload_start = time.monotonic_ns()
        client.request(
            "/api/worker/rl-monopoly/events", {**lease, "events": events}, timeout=60
        )
        excluded_ns += time.monotonic_ns() - upload_start
        events = []

    try:
        with LeaseKeeper(client, lease) if upload_events else _NullKeeper() as keeper:
            while not env.done:
                keeper.check()
                if sequence >= ACTION_CEILING:
                    end_reason = "action_ceiling"
                    break
                if active_us() >= PLAY_LIMIT_US:
                    slowest = slowest_eligible(seats)
                    if slowest is not None:
                        slowest.replace_with_bot()
                    end_reason = "ten_minute"
                    break

                player_id = env.whose_turn()
                runtime = seats[player_id]
                allowed = [int(action) for action in env.get_allowed_actions(player_id)]
                if not allowed:
                    allowed = [int(ActionType.DO_NOTHING)]
                decision_seed = splitmix64(
                    (int(job["seed"]) & ((1 << 64) - 1)) ^ (sequence << 8) ^ player_id
                )
                decision_us: int | None = None
                strike = False
                preparation_ns = 0

                if len(allowed) == 1:
                    action = allowed[0]
                elif runtime.submitted and not runtime.disqualified:
                    state = decision_state(env, player_id, allowed, decision_seed)
                    decision_start: int | None = None
                    try:
                        if runtime.process is None:
                            preparation_start = time.monotonic_ns()
                            try:
                                runtime.restart()
                            finally:
                                preparation_ns += (
                                    time.monotonic_ns() - preparation_start
                                )
                        assert runtime.process is not None
                        decision_start = time.monotonic_ns()
                        action = runtime.process.choose_action(
                            state, player_id, allowed
                        )
                        decision_us = min(
                            2_000_000, (time.monotonic_ns() - decision_start) // 1000
                        )
                    except AgentCallError as error:
                        measured = (
                            0
                            if decision_start is None
                            else (time.monotonic_ns() - decision_start) // 1000
                        )
                        decision_us = (
                            2_000_000
                            if error.kind == "timeout"
                            else min(2_000_000, measured)
                        )
                        runtime.logs.append(f"{error.kind}: {error}")
                        strike = True
                        action = deterministic_fallback(allowed, decision_seed)
                        if not register_strike(runtime) and error.kind != "startup":
                            preparation_start = time.monotonic_ns()
                            try:
                                runtime.restart()
                            except AgentCallError as restart_error:
                                runtime.logs.append(f"restart: {restart_error}")
                            finally:
                                preparation_ns += (
                                    time.monotonic_ns() - preparation_start
                                )
                    runtime.record_decision(decision_us)
                else:
                    assert runtime.fixed is not None
                    action = int(runtime.fixed.choose_action(env))
                    if action not in allowed:
                        action = deterministic_fallback(allowed, decision_seed)

                excluded_ns += preparation_ns
                phase = env.phase
                _state, _reward, _done, info = game.step(action)
                sequence += 1
                events.append(
                    {
                        "seq": sequence,
                        "acted_player": player_id,
                        "round": env.round,
                        "phase": str(info.get("phase", phase)),
                        "action_idx": action,
                        "action_desc": action_to_description(action),
                        "decision_us": decision_us,
                        "strike": strike,
                        "info": info,
                        "snapshot": build_snapshot(env),
                    }
                )
                if len(events) >= EVENT_BATCH_SIZE:
                    flush()
                if sequence >= ACTION_CEILING:
                    end_reason = "action_ceiling"
                    break
                if active_us() >= PLAY_LIMIT_US:
                    slowest = slowest_eligible(seats)
                    if slowest is not None:
                        slowest.replace_with_bot()
                    end_reason = "ten_minute"
                    break
            flush()
            play_duration_us = active_us()

        winner = promoted_winner(seats, env)
        final_snapshot = build_snapshot(env)
        assert play_duration_us is not None
        result = {
            **lease,
            "outcome": "completed",
            "end_reason": end_reason,
            "winner_seat": None if winner is None else winner.seat,
            "winner_entry_id": None if winner is None else winner.entry_id,
            "duration_us": play_duration_us,
            "round": min(MAX_ROUNDS, int(env.round)),
            "action_count": sequence,
            "seats": [
                {
                    "seat": runtime.seat,
                    "final_cash": float(env.players[runtime.seat].cash),
                    "final_net_worth": float(env.players[runtime.seat].net_worth()),
                    "strikes": runtime.strikes,
                    "decision_count": runtime.decision_count,
                    "decision_total_us": runtime.decision_total_us,
                    "decision_min_us": runtime.decision_min_us,
                    "decision_max_us": runtime.decision_max_us,
                    "disqualified": runtime.disqualified,
                    "replacement_bot": runtime.replacement_bot,
                    "runtime_log": runtime.runtime_log(),
                    "final_snapshot": final_snapshot,
                }
                for runtime in seats
            ],
        }
        if upload_events:
            client.request("/api/worker/rl-monopoly/result", result, timeout=60)
        return result
    finally:
        for runtime in seats:
            runtime.close()


class _NullKeeper:
    def __enter__(self):
        return self

    def check(self) -> None:
        pass

    def __exit__(self, *_):
        pass


def report_infrastructure_failure(
    client: ApiClient, job: dict, error: Exception
) -> None:
    body = {
        "worker_id": WORKER_ID,
        "game_id": job["id"],
        "attempt_no": job["attempt_no"],
        "lease_id": job["lease_id"],
        "outcome": "infra_failed",
        "end_reason": None,
        "winner_seat": None,
        "winner_entry_id": None,
        "duration_us": None,
        "round": None,
        "action_count": None,
        "seats": [],
        "error_log": f"{type(error).__name__}: {error}"[-4000:],
    }
    try:
        client.request("/api/worker/rl-monopoly/result", body, timeout=60)
    except Exception as report_error:
        log(f"could not report infrastructure failure: {report_error}")


def selftest() -> None:
    assert effective_vcpus() >= 0
    assert splitmix64(42) == splitmix64(42)
    allowed = [9, 3, 7]
    assert deterministic_fallback(allowed, 123) == deterministic_fallback(allowed, 123)
    game = SharedGame.new(42, max_rounds=2)
    steps = 0
    while not game.env.done and steps < 5000:
        player_id = game.env.whose_turn()
        choices = game.env.get_allowed_actions(player_id) or [
            int(ActionType.DO_NOTHING)
        ]
        game.step(int(choices[0]))
        steps += 1
    assert game.env.done and game.env.round <= 2
    vector = build_state_vector(
        game.env.players, game.env.properties, agent_id=0, env=game.env
    )
    assert tuple(vector.shape) == (300,)
    print("selftest ok")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--once", action="store_true", help="claim at most one game")
    parser.add_argument(
        "--selftest", action="store_true", help="run local engine checks"
    )
    args = parser.parse_args()
    if args.selftest:
        selftest()
        return
    if not WORKER_TOKEN:
        raise SystemExit("WORKER_TOKEN is required")
    if BACKEND not in {"docker", "venv"}:
        raise SystemExit("MONOPOLY_AGENT_BACKEND must be docker or venv")
    client = ApiClient()
    ram, cpus = system_ram_bytes(), effective_vcpus()
    if ram < MIN_RAM_BYTES or cpus < MIN_VCPUS:
        reason = (
            f"requires {MIN_RAM_BYTES / 1024**3:.1f} GiB/{MIN_VCPUS:.1f} vCPU; "
            f"found {ram / 1024**3:.1f} GiB/{cpus:.1f} vCPU"
        )
        report_resource(client, "rejected", reason=reason)
        raise SystemExit(reason)
    report_resource(client, "ready")
    try:
        while True:
            response = (
                client.request(
                    "/api/worker/rl-monopoly/claim", {"worker_id": WORKER_ID}
                )
                or {}
            )
            job = response.get("game")
            if job:
                report_resource(client, "busy", game_id=job["id"])
                log(
                    f"claimed tournament game {job['game_no']} attempt {job['attempt_no']}"
                )
                try:
                    drive_game(job, client)
                except Exception as error:
                    log(f"game failed: {type(error).__name__}: {error}")
                    report_infrastructure_failure(client, job, error)
                report_resource(client, "ready")
            elif args.once:
                break
            else:
                time.sleep(POLL_SECONDS)
            if args.once:
                break
    finally:
        try:
            report_resource(client, "stopped")
        except Exception:
            pass


if __name__ == "__main__":
    main()
