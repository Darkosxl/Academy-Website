#!/usr/bin/env python3
"""Versioned, Bedrock-backed Agentic Harness worker.

The website owns queue state and leases. This process owns trusted scoring,
short-lived Bedrock gateways, and rootless execution boundaries. It never imports
student modules in the worker interpreter.
"""
from __future__ import annotations

import contextlib
import datetime as dt
import fcntl
import hashlib
import http.client
import ipaddress
import json
import math
import os
import pwd
import re
import secrets
import shlex
import shutil
import signal
import socket
import subprocess
import sys
import tempfile
import threading
import time
import urllib.error
import urllib.request
import uuid
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parent
REPOSITORY_ROOT = ROOT.parents[2]
ENV_FILE = Path(os.environ.get("HARNESS_ENV_FILE", REPOSITORY_ROOT / ".env"))
HARBOR_ROOT = Path.home() / ".local" / "share" / "uv" / "tools" / "harbor"
HARBOR_CLI = HARBOR_ROOT / "bin" / "harbor"
HARNESS_VERSION = "harness-2026-sprint-v3"
POLL_SECONDS = 2
ARC_CONCURRENCY = 5
RUN_DEADLINE_SECONDS = 9 * 60 * 60
FRONTIER_DEADLINE_SECONDS = 15 * 60

ARC_GAMES = (
    "bp35", "m0r0", "ft09", "ar25", "s5i5", "sp80", "g50t", "lp85", "r11l", "sc25",
    "tn36", "sk48", "re86", "wa30", "cn04", "tu93", "tr87", "sb26", "su15", "ls20",
    "ka59", "cd82", "dc22", "vc33", "lf52",
)
FRONTIER_TASKS = (
    "html-js-filter",
    "vllm-deepseek-streaming",
    "session-window-debug",
    "mvcc-lsm-compaction",
    "embedding-drift-monitor",
)
REQUIRED_FILES = ("agent/my_agent.py", "agent/harbor_agent.py", "main.py", "requirements.txt")
TERMINAL = {"done", "partial", "failed", "infra_failed"}
MAX_REPO_BYTES = 100 * 1024 * 1024
MAX_FILE_BYTES = 10 * 1024 * 1024
MAX_REQUIREMENTS_BYTES = 32 * 1024
BUILTIN_SUBMISSIONS = {
    "builtin://forge": ROOT / "builtin_submissions" / "forge",
    "builtin://reki": ROOT / "builtin_submissions" / "reki",
    "builtin://terminus-2": ROOT / "builtin_submissions" / "terminus-2",
}


def env_file() -> dict[str, str]:
    # The restricted executor deliberately clears its environment. Do not undo that
    # isolation by discovering a developer's repository .env from the adapter path.
    if os.environ.get("HARNESS_ENV") == "executor":
        return {}
    values: dict[str, str] = {}
    if not ENV_FILE.exists():
        return values
    for raw in ENV_FILE.read_text().splitlines():
        line = raw.strip()
        if line and not line.startswith("#") and "=" in line:
            key, value = line.split("=", 1)
            values[key] = value.strip().strip('"').strip("'")
    return values


CONFIG = {**env_file(), **os.environ}


def deployment_environment(config: dict[str, str]) -> str:
    """Normalize the public DEV/PROD setting and the old local/production spelling."""
    raw = config.get("ENVIRONMENT") or config.get("HARNESS_ENV") or "PROD"
    return {
        "DEV": "DEV",
        "LOCAL": "DEV",
        "PROD": "PROD",
        "PRODUCTION": "PROD",
        "EXECUTOR": "EXECUTOR",
    }.get(raw.strip().upper(), raw.strip().upper())


ENVIRONMENT = deployment_environment(CONFIG)
DEFAULT_CACHE = (
    REPOSITORY_ROOT / "worker" / "cache"
    if ENVIRONMENT == "DEV"
    else Path("/var/lib/exposure-benchmark/cache")
)
CACHE = Path(CONFIG.get("HARNESS_CACHE_DIRECTORY", DEFAULT_CACHE))
ARC_STARTER = CACHE / "arc-starter"
FRONTIER_SOURCE = CACHE / "frontier-bench" / "frontier-bench"
ARC_PYTHON = ARC_STARTER / ".venv" / "bin" / "python"
KAGGLE_CLI = ARC_STARTER / ".venv" / "bin" / "kaggle"
HARNESS_IMAGE = CONFIG.get("HARNESS_IMAGE", "localhost/exposure-harness-arc:0.9.9")
SANDBOX_CONTAINERFILE = Path(CONFIG.get(
    "HARNESS_SANDBOX_CONTAINERFILE", ROOT.parent / "sandbox" / "Containerfile"
))
SITE = CONFIG.get("HARNESS_SITE", "http://127.0.0.1:3000").rstrip("/")
WORKER_TOKEN = CONFIG.get("WORKER_TOKEN", "")
BEDROCK_API_KEY = CONFIG.get("BEDROCK_API_KEY", "")
AWS_REGION = CONFIG.get("AWS_REGION", "") or ("us-east-1" if BEDROCK_API_KEY else "")
BEDROCK_MODEL_ID = (
    CONFIG.get("BEDROCK_MODEL_ID", "")
    or CONFIG.get("BEDROCK_INFERENCE_PROFILE_ARN", "")
    or ("xai.grok-4.3" if BEDROCK_API_KEY else "")
)
BEDROCK_PROFILE_NAME = CONFIG.get("BEDROCK_PROFILE_NAME", "") or BEDROCK_MODEL_ID or "bedrock-harness"
BEDROCK_LATENCY = CONFIG.get("BEDROCK_LATENCY", "standard").strip().lower()
BEDROCK_REASONING_EFFORT = CONFIG.get("BEDROCK_REASONING_EFFORT", "none").strip().lower()
FRAME_PREFIX = "@@EXPOSURE_ARC_FRAMES@@"
EMIT_LOCK = threading.Lock()
KAGGLE_NDJSON = False
NDJSON_MODE = False


class RunFailed(Exception):
    """The submission failed its contract or agent execution."""


class InfrastructureFailed(Exception):
    """The harness, Bedrock, or host failed; do not penalize the submission."""


class LeaseLost(InfrastructureFailed):
    pass


@contextlib.contextmanager
def ram_lane(deadline: float):
    """Serialize host-level RAM measurements across concurrent harness runs."""
    lock_path = Path(CONFIG.get("HARNESS_RAM_LOCK", "/tmp/exposure-benchmark-ram.lock"))
    lock_path.parent.mkdir(parents=True, exist_ok=True)
    with lock_path.open("a+") as lock:
        while True:
            try:
                fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
                break
            except BlockingIOError:
                if time.monotonic() >= deadline:
                    raise InfrastructureFailed("the benchmark deadline expired waiting for the RAM lane")
                time.sleep(0.1)
        try:
            yield
        finally:
            fcntl.flock(lock, fcntl.LOCK_UN)


def log(message: str) -> None:
    if NDJSON_MODE:
        emit({"type": "log", "level": "info", "message": message})
    else:
        print(f"[{time.strftime('%H:%M:%S')}] {message}", flush=True)


def emit(payload: dict[str, Any]) -> None:
    """One bounded protocol line for the Rust executor; never include provider secrets."""
    encoded = json.dumps(payload, separators=(",", ":"))
    if len(encoded.encode()) > 8 * 1024 * 1024:
        raise InfrastructureFailed("adapter NDJSON message exceeded 8 MiB")
    with EMIT_LOCK:
        print(encoded, flush=True)


def api(path: str, body: Any | None = None) -> Any:
    data = json.dumps(body).encode() if body is not None else None
    request = urllib.request.Request(
        SITE + path,
        data=data,
        headers={"X-Worker-Token": WORKER_TOKEN, "Content-Type": "application/json"},
        method="POST" if body is not None else "GET",
    )
    try:
        with urllib.request.urlopen(request, timeout=15) as response:
            raw = response.read()
            return json.loads(raw) if raw else None
    except urllib.error.HTTPError as exc:
        if exc.code == 409:
            raise LeaseLost("worker lease was rejected") from exc
        detail = exc.read().decode(errors="replace")[-1000:]
        raise InfrastructureFailed(f"site API {path} returned HTTP {exc.code}: {detail}") from exc


def bounded_timeout(deadline: float, cap: float) -> float:
    remaining = deadline - time.monotonic()
    if remaining <= 0:
        raise InfrastructureFailed("the benchmark deadline expired")
    return max(0.1, min(cap, remaining))


def buildkit_nameservers(paths: tuple[Path, ...] = (
    Path("/run/systemd/resolve/resolv.conf"),
    Path("/etc/resolv.conf"),
)) -> list[str]:
    nameservers: list[str] = []

    def add(value: str) -> None:
        with contextlib.suppress(ValueError):
            address = ipaddress.ip_address(value.split("%", 1)[0])
            if not address.is_loopback and not address.is_unspecified:
                value = str(address)
                if value not in nameservers:
                    nameservers.append(value)

    for path in paths:
        with contextlib.suppress(OSError):
            for line in path.read_text().splitlines():
                parts = line.split()
                if len(parts) == 2 and parts[0] == "nameserver":
                    add(parts[1])
                elif line.startswith("# ExtServers: [") and line.endswith("]"):
                    for server in line.removeprefix("# ExtServers: [").removesuffix("]").replace(",", " ").split():
                        add(server)
        if nameservers:
            return nameservers
    raise InfrastructureFailed("no non-loopback DNS resolver is available for BuildKit")


def parse_deadline(value: str) -> float:
    parsed = dt.datetime.fromisoformat(value.replace("Z", "+00:00"))
    seconds = (parsed - dt.datetime.now(dt.timezone.utc)).total_seconds()
    if not 0 < seconds <= RUN_DEADLINE_SECONDS + 10:
        raise InfrastructureFailed("claim returned an invalid deadline")
    return time.monotonic() + seconds


class Lease:
    def __init__(self, run_id: str, token: str, deadline: float):
        self.run_id = run_id
        self.token = token
        self.deadline = deadline
        self.stop_event = threading.Event()
        self.lost = threading.Event()
        self.thread = threading.Thread(target=self._run, name=f"lease-{run_id[:8]}", daemon=True)

    def __enter__(self) -> "Lease":
        self.thread.start()
        return self

    def __exit__(self, *_args: Any) -> None:
        self.stop_event.set()
        self.thread.join(timeout=2)

    def _run(self) -> None:
        failures = 0
        while not self.stop_event.wait(5):
            if time.monotonic() >= self.deadline:
                return
            try:
                api("/api/worker/harness/heartbeat", {"id": self.run_id, "lease_token": self.token})
                failures = 0
            except Exception as exc:
                failures += 1
                log(f"heartbeat failed ({failures}/3): {exc}")
                if isinstance(exc, LeaseLost) or failures >= 3:
                    self.lost.set()
                    return

    def check(self) -> None:
        if self.lost.is_set():
            raise LeaseLost("worker lost its run lease")
        if time.monotonic() >= self.deadline:
            raise InfrastructureFailed("the benchmark deadline expired")


class Reporter:
    def __init__(self, lease: Lease):
        self.lease = lease
        self.lock = threading.Lock()
        self.state: dict[str, dict[str, Any]] = {
            "arc": {"status": "pending"},
            "frontier": {"status": "pending"},
            "ram": {"status": "pending"},
        }

    def update(self, benchmark: str, **values: Any) -> None:
        if self.lease.lost.is_set():
            raise LeaseLost("worker lost its run lease")
        with self.lock:
            self.state[benchmark] = {**self.state[benchmark], **values}
            snapshot = dict(self.state[benchmark])
        # The terminal result endpoint deliberately accepts this lease after the
        # hard deadline. Keep the final local snapshot, but do not send stale progress.
        if time.monotonic() >= self.lease.deadline:
            return
        api("/api/worker/harness/progress", {
            "id": self.lease.run_id,
            "lease_token": self.lease.token,
            "benchmark": benchmark,
            "state": snapshot,
        })

    def snapshot(self) -> dict[str, dict[str, Any]]:
        with self.lock:
            return json.loads(json.dumps(self.state))


class NdjsonReporter:
    """Benchmark SDK progress only; Rust owns Academy state and lease decisions."""

    def __init__(self, run_id: str, deadline: float, benchmark_kind: str = "bundled"):
        if benchmark_kind not in {"arc", "frontier", "bundled"}:
            raise InfrastructureFailed("executor request contained an invalid benchmark kind")
        self.run_id = run_id
        self.deadline = deadline
        self.lock = threading.Lock()
        self.state: dict[str, dict[str, Any]] = {
            "arc": {"status": "skipped" if benchmark_kind == "frontier" else "pending"},
            "frontier": {"status": "skipped" if benchmark_kind == "arc" else "pending"},
            "ram": {"status": "pending"},
        }

    def update(self, benchmark: str, **values: Any) -> None:
        with self.lock:
            self.state[benchmark] = {**self.state[benchmark], **values}
            snapshot = dict(self.state[benchmark])
        if time.monotonic() < self.deadline:
            emit({"type": "progress", "benchmark": benchmark, "state": snapshot})

    def frames(self, rows: list[dict[str, Any]]) -> None:
        if rows:
            emit({"type": "frames", "frames": rows})

    def snapshot(self) -> dict[str, dict[str, Any]]:
        with self.lock:
            return json.loads(json.dumps(self.state))


class UnixHTTPConnection(http.client.HTTPConnection):
    def __init__(self, socket_path: Path):
        super().__init__("localhost", timeout=5)
        self.socket_path = socket_path

    def connect(self) -> None:
        self.sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self.sock.settimeout(self.timeout)
        self.sock.connect(str(self.socket_path))


class Gateway:
    def __init__(self, name: str, parent: Path):
        self.name = name
        self.directory = parent / f"gateway-{name}"
        self.directory.mkdir(mode=0o755)
        self.socket_path = self.directory / "bedrock.sock"
        self.token = secrets.token_hex(32)
        env = os.environ.copy()
        for key, value in CONFIG.items():
            if key.startswith("AWS_"):
                env[key] = value
        if BEDROCK_API_KEY:
            # AWS SDKs recognize this official bearer-token variable.
            env["AWS_BEARER_TOKEN_BEDROCK"] = BEDROCK_API_KEY
        env.update({
            "AWS_REGION": AWS_REGION,
            "BEDROCK_MODEL_ID": BEDROCK_MODEL_ID,
            "BEDROCK_PROFILE_NAME": BEDROCK_PROFILE_NAME,
            "BEDROCK_LATENCY": BEDROCK_LATENCY,
            "BEDROCK_REASONING_EFFORT": BEDROCK_REASONING_EFFORT,
            "BEDROCK_GATEWAY_TOKEN": self.token,
            "BEDROCK_MAX_CONCURRENCY": CONFIG.get("BEDROCK_MAX_CONCURRENCY", "32"),
            "HARNESS_BENCHMARK_VERSION": HARNESS_VERSION,
        })
        self.process = subprocess.Popen(
            [sys.executable, str(ROOT / "bedrock_gateway.py"), "--socket", str(self.socket_path)],
            env=env,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.PIPE,
            text=True,
        )
        deadline = time.monotonic() + 10
        while not self.socket_path.exists() and self.process.poll() is None and time.monotonic() < deadline:
            time.sleep(0.05)
        if not self.socket_path.exists():
            detail = self.process.stderr.read()[-2000:] if self.process.stderr else ""
            raise InfrastructureFailed(f"Bedrock gateway {name} did not start: {detail}")

    def metrics(self) -> dict[str, Any]:
        connection = UnixHTTPConnection(self.socket_path)
        connection.request("GET", "/metrics", headers={"Authorization": f"Bearer {self.token}"})
        response = connection.getresponse()
        raw = response.read()
        connection.close()
        if response.status != 200:
            raise InfrastructureFailed(f"Bedrock gateway metrics returned HTTP {response.status}")
        return json.loads(raw)

    def stop(self) -> None:
        if self.process.poll() is None:
            self.process.terminate()
            try:
                self.process.wait(timeout=3)
            except subprocess.TimeoutExpired:
                self.process.kill()
                self.process.wait()


class ExternalGateway:
    """Run-scoped Rust gateway capability. No AWS or provider credential is present here."""

    def __init__(self, socket_path: Path, token: str):
        self.socket_path = socket_path
        self.directory = socket_path.parent
        self.token = token

    def metrics(self) -> dict[str, Any]:
        connection = UnixHTTPConnection(self.socket_path)
        connection.request("GET", "/metrics", headers={"Authorization": f"Bearer {self.token}"})
        response = connection.getresponse()
        raw = response.read()
        connection.close()
        if response.status != 200:
            raise InfrastructureFailed(f"Rust model gateway metrics returned HTTP {response.status}")
        return json.loads(raw)

    def stop(self) -> None:
        return


def run_checked(command: list[str], *, timeout: float, cwd: Path | None = None, env: dict[str, str] | None = None) -> subprocess.CompletedProcess:
    try:
        result = subprocess.run(command, cwd=cwd, env=env, capture_output=True, text=True, timeout=timeout)
    except subprocess.TimeoutExpired as exc:
        raise RunFailed(f"command exceeded {timeout:.0f}s: {command[0]}") from exc
    if result.returncode != 0:
        detail = (result.stderr or result.stdout)[-3000:]
        raise RunFailed(f"{command[0]} failed with exit code {result.returncode}:\n{detail}")
    return result


def ensure_arc_host(*, require_worker_token: bool = False) -> None:
    """Preflight only what the one-game development loop actually uses."""
    if require_worker_token and not WORKER_TOKEN:
        raise SystemExit("WORKER_TOKEN is required")
    if not AWS_REGION or not BEDROCK_MODEL_ID:
        raise SystemExit("AWS_REGION and BEDROCK_MODEL_ID are required")
    if not re.fullmatch(r"[A-Za-z0-9._-]{1,64}", BEDROCK_PROFILE_NAME):
        raise SystemExit("BEDROCK_PROFILE_NAME must contain only letters, digits, dot, underscore, or dash")
    if BEDROCK_LATENCY not in ("standard", "optimized"):
        raise SystemExit("BEDROCK_LATENCY must be standard or optimized")
    if BEDROCK_REASONING_EFFORT not in ("none", "low", "medium", "high"):
        raise SystemExit("BEDROCK_REASONING_EFFORT must be none, low, medium, or high")
    if ENVIRONMENT not in ("DEV", "PROD"):
        raise SystemExit("ENVIRONMENT must be DEV or PROD")
    for path in (ARC_PYTHON, ARC_STARTER / "environment_files"):
        if not path.exists():
            raise SystemExit(f"required ARC cache is missing: {path}")
    info = run_checked(
        ["podman", "info", "--format", "{{.Host.Security.Rootless}} {{.Host.CgroupsVersion}}"],
        timeout=15,
    )
    expected = "false v2" if CONFIG.get("HARNESS_PODMAN_ROOTFUL") == "1" else "true v2"
    if info.stdout.strip() != expected:
        raise SystemExit(f"the harness requires Podman {expected}")


def ensure_host() -> None:
    ensure_arc_host(require_worker_token=True)
    if ENVIRONMENT == "PROD":
        expected_user = CONFIG.get("HARNESS_HARBOR_USER", "")
        current_user = pwd.getpwuid(os.getuid()).pw_name
        if not expected_user or current_user != expected_user:
            raise SystemExit(
                "production worker must run as the dedicated HARNESS_HARBOR_USER account"
            )
    for path in (
        KAGGLE_CLI, ARC_STARTER / "scripts" / "build_notebook.py", FRONTIER_SOURCE,
        HARBOR_CLI,
    ):
        if not path.exists():
            raise SystemExit(f"required benchmark cache is missing: {path}")
    if shutil.which("bwrap") is None or shutil.which("socat") is None or shutil.which("docker") is None:
        raise SystemExit("bwrap, socat, and the Docker CLI with Compose are required")
    run_checked(["docker", "compose", "version"], timeout=10)


def ensure_image() -> None:
    exists = subprocess.run(["podman", "image", "exists", HARNESS_IMAGE], capture_output=True).returncode == 0
    if exists:
        return
    log(f"building sandbox image {HARNESS_IMAGE}")
    run_checked([
        "podman", "build", "--label", f"academy.harness.version={HARNESS_VERSION}",
        "-t", HARNESS_IMAGE, "-f", str(SANDBOX_CONTAINERFILE), str(ARC_STARTER),
    ], timeout=900)


def podman_base(work: Path, *, network: str, memory: str = "2g", cpus: str = "2") -> list[str]:
    return [
        "podman", "run", "--rm", f"--network={network}", "--cap-drop=all",
        "--security-opt=no-new-privileges", "--pids-limit=256", f"--memory={memory}",
        f"--cpus={cpus}", "--mount", f"type=bind,src={work},dst=/work",
        HARNESS_IMAGE,
    ]


def valid_repo_url(repo_url: str) -> bool:
    if not re.fullmatch(r"https://github\.com/[A-Za-z0-9._-]+/[A-Za-z0-9._-]+", repo_url):
        return False
    owner, repo = repo_url.removeprefix("https://github.com/").split("/", 1)
    return owner not in (".", "..") and repo not in (".", "..")


def builtin_submission(repo_url: str) -> Path | None:
    return BUILTIN_SUBMISSIONS.get(repo_url)


def submission_digest(repo: Path) -> str:
    digest = hashlib.sha256()
    for path in sorted(
        candidate
        for candidate in repo.rglob("*")
        if candidate.is_file()
        and candidate.suffix != ".pyc"
        and "__pycache__" not in candidate.parts
    ):
        digest.update(path.relative_to(repo).as_posix().encode())
        digest.update(b"\0")
        digest.update(path.read_bytes())
    return digest.hexdigest()[:40]


def clone_submission(repo_url: str, work: Path, deadline: float) -> tuple[Path, str, Path]:
    bundled = builtin_submission(repo_url)
    if bundled is not None:
        if not bundled.is_dir():
            raise InfrastructureFailed(f"built-in harness artifact is missing: {repo_url}")
        repo = shutil.copytree(
            bundled,
            work / "repo",
            ignore=shutil.ignore_patterns("__pycache__", "*.pyc"),
        )
        sha = submission_digest(repo)
        build_network = "none"
    else:
        if not valid_repo_url(repo_url):
            raise RunFailed("claim contained an invalid submission source")
        command = podman_base(work, network="slirp4netns") + [
            "git", "-c", "protocol.file.allow=never", "clone", "--depth=1", "--filter=blob:none",
            "--no-tags", repo_url, "/work/repo",
        ]
        run_checked(command, timeout=bounded_timeout(deadline, 30))
        repo = work / "repo"
        result = run_checked(["git", "-c", f"safe.directory={repo}", "-C", str(repo), "rev-parse", "HEAD"], timeout=5)
        sha = result.stdout.strip().lower()
        if not re.fullmatch(r"[0-9a-f]{40}", sha):
            raise RunFailed("git did not resolve a valid commit SHA")
        build_network = "slirp4netns"
    validate_submission(repo)
    venv = work / "venv"
    build = podman_base(work, network=build_network, memory="4g", cpus="2") + [
        "sh", "-lc",
        "python -m venv --system-site-packages /work/venv && "
        "/work/venv/bin/python -m pip install --disable-pip-version-check --no-input "
        "-r /work/repo/requirements.txt && "
        "/work/venv/bin/python -c \"from pathlib import Path; "
        "[compile(Path(p).read_bytes(),p,'exec') for p in "
        "('/work/repo/main.py','/work/repo/agent/my_agent.py','/work/repo/agent/harbor_agent.py')]\"",
    ]
    run_checked(build, timeout=bounded_timeout(deadline, 90))
    return repo, sha, venv


def clone_exact_submission(repo_url: str, commit_sha: str, work: Path) -> Path:
    """Fetch only the commit already scored locally; never resubmit a moving HEAD."""
    if not valid_repo_url(repo_url) or not re.fullmatch(r"[0-9a-f]{40}", commit_sha):
        raise RunFailed("official submission claim contained an invalid repository or commit")
    base = podman_base(work, network="slirp4netns")
    run_checked(base + ["git", "init", "/work/repo"], timeout=10)
    run_checked(base + ["git", "-C", "/work/repo", "remote", "add", "origin", repo_url], timeout=5)
    run_checked(base + [
        "git", "-c", "protocol.file.allow=never", "-C", "/work/repo", "fetch",
        "--depth=1", "--no-tags", "origin", commit_sha,
    ], timeout=45)
    run_checked(base + ["git", "-C", "/work/repo", "checkout", "--detach", "FETCH_HEAD"], timeout=10)
    repo = work / "repo"
    resolved = run_checked([
        "git", "-c", f"safe.directory={repo}", "-C", str(repo), "rev-parse", "HEAD",
    ], timeout=5).stdout.strip().lower()
    if resolved != commit_sha:
        raise RunFailed("official submission checkout did not match the scored commit")
    validate_submission(repo)
    return repo


def validate_submission(repo: Path) -> None:
    if (repo / ".gitmodules").exists():
        raise RunFailed("git submodules are not allowed")
    total = 0
    for root, directories, files in os.walk(repo, followlinks=False):
        if Path(root) == repo and ".git" in directories:
            directories.remove(".git")
        for name in [*directories, *files]:
            path = Path(root) / name
            if path.is_symlink():
                raise RunFailed(f"symlinks are not allowed: {path.relative_to(repo)}")
        for name in files:
            path = Path(root) / name
            size = path.stat().st_size
            if size > MAX_FILE_BYTES:
                raise RunFailed(f"file exceeds 10 MiB: {path.relative_to(repo)}")
            total += size
            if total > MAX_REPO_BYTES:
                raise RunFailed("repository exceeds 100 MiB")
    missing = [name for name in REQUIRED_FILES if not (repo / name).is_file()]
    if missing:
        raise RunFailed("missing required files: " + ", ".join(missing))
    requirements = repo / "requirements.txt"
    if requirements.stat().st_size > MAX_REQUIREMENTS_BYTES:
        raise RunFailed("requirements.txt exceeds 32 KiB")
    for number, raw in enumerate(requirements.read_text(errors="strict").splitlines(), 1):
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        lowered = line.lower()
        if line.startswith("-") or " @ " in line or "://" in line or "git+" in lowered \
                or lowered.startswith(("file:", ".", "/")) or "\\" in line:
            raise RunFailed(f"requirements.txt line {number} uses a forbidden direct source or option")


def gateway_env(gateway: Gateway) -> dict[str, str]:
    return {
        "OPENAI_BASE_URL": "http://127.0.0.1:8000/v1",
        "OPENAI_API_BASE": "http://127.0.0.1:8000/v1",
        "OPENAI_API_KEY": gateway.token,
        "HARNESS_LLM_BASE": "http://127.0.0.1:8000/v1",
        "HARNESS_LLM_KEY": gateway.token,
        "HARNESS_LLM_MODEL": BEDROCK_PROFILE_NAME,
    }


def cgroup_path(pid: int) -> Path:
    for line in Path(f"/proc/{pid}/cgroup").read_text().splitlines():
        parts = line.split(":", 2)
        if len(parts) == 3 and parts[0] == "0":
            return Path("/sys/fs/cgroup") / parts[2].lstrip("/")
    raise InfrastructureFailed("could not resolve the container cgroup")


def pss_kb(cgroup: Path) -> int:
    try:
        pids = cgroup.joinpath("cgroup.procs").read_text().split()
    except OSError:
        return 0
    total = 0
    for pid in pids:
        try:
            for line in Path(f"/proc/{pid}/smaps_rollup").read_text().splitlines():
                if line.startswith("Pss:"):
                    total += int(line.split()[1])
                    break
        except (OSError, ValueError):
            continue
    return total


def run_ram_scenario(run_id: str, sessions: int, repo: Path, venv: Path, gateway: Gateway, deadline: float) -> tuple[float, float]:
    before = gateway.metrics()
    name = f"academy-ram-{run_id[:8]}-{sessions}-{uuid.uuid4().hex[:6]}"
    memory = "2g" if sessions == 1 else "8g"
    cpus = "1" if sessions == 1 else "10"
    command = [
        "podman", "run", "-d", "--name", name, "--network=none", "--cap-drop=all",
        "--group-add", "keep-groups",
        "--security-opt=no-new-privileges", "--pids-limit", "128" if sessions == 1 else "512",
        "--memory", memory, "--cpus", cpus, "--read-only",
        "--tmpfs", "/tmp:rw,noexec,nosuid,size=256m",
        "--label", f"academy.harness.run={run_id}", "--label", "academy.harness.benchmark=ram",
        "--mount", f"type=bind,src={repo},dst=/submission,ro=true",
        "--mount", f"type=bind,src={venv},dst=/venv,ro=true",
        "--mount", f"type=bind,src={gateway.directory},dst=/run/harness,ro=true",
        "--mount", f"type=bind,src={ROOT},dst=/opt/harness,ro=true",
        "-e", "HOME=/tmp/home",
    ]
    for key, value in gateway_env(gateway).items():
        command += ["-e", f"{key}={value}"]
    command += [
        HARNESS_IMAGE, "sh", "-lc",
        "mkdir -p /tmp/home && "
        "socat TCP-LISTEN:8000,bind=127.0.0.1,reuseaddr,fork UNIX-CONNECT:/run/harness/bedrock.sock & "
        "sleep 0.1 && "
        f"exec /venv/bin/python /opt/harness/ram_session.py --sessions {sessions}",
    ]
    # First use may initialize persistent Podman storage before returning the container ID.
    # The measured scenario still has its separate strict ten-second deadline below.
    run_checked(command, timeout=30)
    peak = 0
    memory_peak = 0
    try:
        pid = 0
        start_deadline = min(deadline, time.monotonic() + 10)
        while pid <= 0 and time.monotonic() < start_deadline:
            inspected = run_checked(["podman", "inspect", "--format", "{{.State.Pid}}", name], timeout=5)
            pid = int(inspected.stdout.strip())
            if pid <= 0:
                time.sleep(0.02)
        if pid <= 0:
            raise InfrastructureFailed("RAM container did not start")
        group = cgroup_path(pid)
        scenario_deadline = min(deadline, time.monotonic() + 10.5)
        def alive() -> bool:
            try:
                return Path(f"/proc/{pid}/stat").read_text().split()[2] != "Z"
            except (OSError, IndexError):
                return False
        while alive() and time.monotonic() < scenario_deadline:
            peak = max(peak, pss_kb(group))
            time.sleep(0.02)
        if alive():
            raise RunFailed(f"RAM {sessions}-session scenario exceeded ten seconds")
        with contextlib.suppress(OSError, ValueError):
            memory_peak = int(group.joinpath("memory.peak").read_text())
        status = run_checked(["podman", "wait", name], timeout=5).stdout.strip()
        logs = run_checked(["podman", "logs", name], timeout=5).stdout
        if status != "0":
            raise RunFailed(f"RAM {sessions}-session contract failed:\n{logs[-2000:]}")
        payload = json.loads(logs.strip().splitlines()[-1])
        if not payload.get("ok"):
            raise RunFailed(f"RAM {sessions}-session contract failed")
    finally:
        subprocess.run(["podman", "rm", "-f", name], capture_output=True)
    after = gateway.metrics()
    request_delta = after["requests"] - before["requests"]
    error_delta = after["errors"] - before["errors"]
    if request_delta != sessions or error_delta != 0:
        raise RunFailed(
            f"RAM {sessions}-session scenario must make exactly one successful Bedrock request per session "
            f"(saw {request_delta} requests, {error_delta} errors)"
        )
    if peak <= 0:
        raise InfrastructureFailed("RAM PSS sampling produced no measurement")
    return round(peak / 1024, 1), round(memory_peak / 1024 / 1024, 1)


def run_ram(run_id: str, repo: Path, venv: Path, gateway: Gateway, reporter: Reporter, deadline: float) -> tuple[float, float]:
    reporter.update("ram", status="running", scenario="one_session", done=0, total=2)
    one, one_cgroup = run_ram_scenario(run_id, 1, repo, venv, gateway, deadline)
    reporter.update("ram", status="running", scenario="ten_sessions", done=1, total=2,
                    one_session_mb=one, one_session_cgroup_peak_mb=one_cgroup)
    ten, ten_cgroup = run_ram_scenario(run_id, 10, repo, venv, gateway, deadline)
    reporter.update("ram", status="done", done=2, total=2, one_session_mb=one,
                    ten_session_mb=ten, one_session_cgroup_peak_mb=one_cgroup,
                    ten_session_cgroup_peak_mb=ten_cgroup)
    return one, ten


def parse_last_json(text: str) -> dict[str, Any]:
    for line in reversed(text.splitlines()):
        with contextlib.suppress(json.JSONDecodeError):
            value = json.loads(line)
            if isinstance(value, dict):
                return value
    raise RunFailed("benchmark process did not emit a JSON result")


class ArcStderrDrain:
    """Keeps ARC stderr flowing and peels trusted frame batches off for Rust."""

    def __init__(self, stream, reporter: Reporter | NdjsonReporter):
        self.stream = stream
        self.reporter = reporter
        self.tail = ""
        self.thread = threading.Thread(target=self._run, name="arc-stderr", daemon=True)
        self.thread.start()

    def _run(self) -> None:
        for line in self.stream:
            if line.startswith(FRAME_PREFIX) and isinstance(self.reporter, NdjsonReporter):
                try:
                    payload = json.loads(line.removeprefix(FRAME_PREFIX))
                    rows = payload.get("frames")
                    if isinstance(rows, list):
                        self.reporter.frames(rows)
                        continue
                except Exception:
                    pass
            self.tail = (self.tail + line)[-4000:]

    def finish(self) -> str:
        self.thread.join(timeout=2)
        return self.tail


def cleanup_run_containers(run_id: str) -> None:
    try:
        result = subprocess.run(
            ["podman", "ps", "-aq", "--filter", f"label=academy.harness.run={run_id}"],
            capture_output=True, text=True, timeout=5,
        )
    except subprocess.TimeoutExpired:
        return
    ids = [line for line in result.stdout.splitlines() if re.fullmatch(r"[0-9a-f]+", line)]
    if ids:
        with contextlib.suppress(subprocess.TimeoutExpired):
            subprocess.run(["podman", "rm", "-f", *ids], capture_output=True, timeout=15)


def harbor_compose_scopes(jobs: Path) -> tuple[set[str], set[str]]:
    projects: set[str] = set()
    verifier_prefixes: set[str] = set()
    if not jobs.exists():
        return projects, verifier_prefixes
    for job in jobs.iterdir():
        if not job.is_dir():
            continue
        for trial in job.iterdir():
            if not trial.is_dir():
                continue
            name = trial.name.lower()
            if not re.match(r"^[a-z0-9]", name):
                name = "0" + name
            name = re.sub(r"[^a-z0-9_-]", "-", name)
            projects.add(f"{name}__env")
            verifier_prefixes.add(f"{name}__verifier__")
    return projects, verifier_prefixes


def cleanup_harbor_containers(jobs: Path) -> None:
    projects, verifier_prefixes = harbor_compose_scopes(jobs)
    if not projects:
        return
    try:
        result = subprocess.run(
            [
                "podman", "ps", "-a", "--filter", "label=com.docker.compose.project",
                "--format", '{{.ID}}\t{{.Label "com.docker.compose.project"}}',
            ],
            capture_output=True, text=True, timeout=5,
        )
    except subprocess.TimeoutExpired:
        return
    ids: set[str] = set()
    if result.returncode == 0:
        for line in result.stdout.splitlines():
            container_id, separator, project = line.partition("\t")
            if (separator and re.fullmatch(r"[0-9a-f]+", container_id)
                    and (project in projects
                         or any(project.startswith(prefix) for prefix in verifier_prefixes))):
                ids.add(container_id)
    if ids:
        with contextlib.suppress(subprocess.TimeoutExpired):
            subprocess.run(
                ["podman", "rm", "-f", *sorted(ids)], capture_output=True, timeout=15,
            )


def run_arc(run_id: str, repo: Path, venv: Path, gateway: Gateway, reporter: Reporter, deadline: float) -> float:
    reporter.update(
        "arc", status="running", done=0, total=len(ARC_GAMES), games={},
        active=0, queued=len(ARC_GAMES), rate=0,
    )
    env = os.environ.copy()
    env.update({
        "HARNESS_ARC_STARTER": str(ARC_STARTER),
        "BEDROCK_GATEWAY_TOKEN": gateway.token,
        "BEDROCK_PROFILE_NAME": BEDROCK_PROFILE_NAME,
        # The live board feed needs these; CONFIG merges .env, so os.environ may not carry them.
        "HARNESS_SITE": SITE,
        "WORKER_TOKEN": WORKER_TOKEN,
    })
    if isinstance(reporter, NdjsonReporter):
        env["HARNESS_FRAME_SINK"] = "ndjson"
        env.pop("HARNESS_SITE", None)
        env.pop("WORKER_TOKEN", None)
    else:
        env["HARNESS_LEASE_TOKEN"] = reporter.lease.token
    processes: dict[str, subprocess.Popen] = {}
    stderr_drains: dict[str, ArcStderrDrain] = {}
    pending = iter(ARC_GAMES)

    def fill_slots() -> None:
        while len(processes) < ARC_CONCURRENCY:
            game = next(pending, None)
            if game is None:
                return
            process = subprocess.Popen([
                str(ARC_PYTHON), str(ROOT / "arc_game.py"), "--game", game,
                "--deadline-monotonic", str(deadline), "--repo", str(repo), "--venv", str(venv),
                "--gateway-dir", str(gateway.directory), "--image", HARNESS_IMAGE,
                "--worker-dir", str(ROOT), "--run-id", run_id,
            ], env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
            processes[game] = process
            stderr_drains[game] = ArcStderrDrain(process.stderr, reporter)

    results: dict[str, dict[str, Any]] = {}
    started = time.monotonic()
    next_rate_check = started + 30
    try:
        fill_slots()
        reporter.update(
            "arc", status="running", done=0, total=len(ARC_GAMES), games={},
            active=len(processes), queued=len(ARC_GAMES) - len(processes), rate=0,
        )
        while processes:
            if time.monotonic() >= deadline:
                for game in ARC_GAMES:
                    if game in results:
                        continue
                    results[game] = {"game": game, "status": "timeout", "score": 0.0}
                break
            completed = False
            for game, process in list(processes.items()):
                if process.poll() is None:
                    continue
                stdout = process.stdout.read() if process.stdout else ""
                stderr = stderr_drains.pop(game).finish()
                try:
                    result = parse_last_json(stdout)
                except RunFailed:
                    result = {"game": game, "status": "failed", "score": 0.0,
                              "error": (stderr or stdout)[-1000:]}
                score = result.get("score")
                if not isinstance(score, (int, float)) or not math.isfinite(score) or not 0 <= score <= 100:
                    result = {"game": game, "status": "failed", "score": 0.0, "error": "invalid score"}
                results[game] = result
                del processes[game]
                completed = True
            if completed:
                fill_slots()
                reporter.update(
                    "arc", status="running", done=len(results), total=len(ARC_GAMES),
                    games=results, active=len(processes),
                    queued=len(ARC_GAMES) - len(results) - len(processes),
                )
            now = time.monotonic()
            if processes and now >= next_rate_check:
                metrics = gateway.metrics()
                request_rate = int(metrics["completed_last_30s"])
                reporter.update("arc", status="running", done=len(results), total=len(ARC_GAMES),
                                games=results, active=len(processes), rate=request_rate,
                                queued=len(ARC_GAMES) - len(results) - len(processes))
                next_rate_check = now + 5
            time.sleep(0.2)
    finally:
        for process in processes.values():
            if process.poll() is None:
                process.terminate()
        for process in processes.values():
            with contextlib.suppress(subprocess.TimeoutExpired):
                process.wait(timeout=2)
            if process.poll() is None:
                process.kill()
        for drain in stderr_drains.values():
            drain.finish()
        cleanup_run_containers(run_id)
    score = round(sum(float(results.get(game, {}).get("score", 0.0)) for game in ARC_GAMES) / len(ARC_GAMES), 1)
    reporter.update("arc", status="done", done=len(ARC_GAMES), total=len(ARC_GAMES), games=results,
                    score=score, gateway=gateway.metrics())
    return score


def rewrite_timeouts(path: Path) -> None:
    section = ""
    output: list[str] = []
    protected_sections = {"agent", "verifier", "environment"}
    found_sections: set[str] = set()
    network_written = False

    def finish_section() -> None:
        nonlocal network_written
        if section in protected_sections and not network_written:
            output.append('network_mode = "no-network"')

    for line in path.read_text().splitlines():
        match = re.fullmatch(r"\[([^]]+)]", line.strip())
        if match:
            finish_section()
            section = match.group(1)
            network_written = False
            if section in protected_sections:
                found_sections.add(section)
        if line.strip().startswith("timeout_sec") and section == "agent":
            line = "timeout_sec = 120.0"
        elif line.strip().startswith("timeout_sec") and section == "verifier":
            line = "timeout_sec = 60.0"
        elif line.strip().startswith("network_mode") and section in protected_sections:
            line = 'network_mode = "no-network"'
            network_written = True
        elif line.strip().startswith("allowed_hosts") and section in protected_sections:
            line = "allowed_hosts = []"
        output.append(line)
    finish_section()
    if found_sections != protected_sections:
        missing = ", ".join(sorted(protected_sections - found_sections))
        raise InfrastructureFailed(f"Frontier task is missing required TOML sections: {missing}")
    path.write_text("\n".join(output) + "\n")


def sprint_dataset(work: Path) -> Path:
    target = work / "frontier-sprint"
    target.mkdir()
    for task in FRONTIER_TASKS:
        source = FRONTIER_SOURCE / task
        destination = target / task
        # Work directories may live on a different filesystem from the pinned cache.
        shutil.copytree(source, destination, copy_function=shutil.copy2, symlinks=True)
        task_config = destination / "task.toml"
        rewrite_timeouts(task_config)
        shutil.rmtree(destination / "solution", ignore_errors=True)
    return target


def scan_frontier(jobs: Path) -> tuple[dict[str, dict[str, Any]], int]:
    results: dict[str, dict[str, Any]] = {}
    active = 0
    if not jobs.exists():
        return results, len(FRONTIER_TASKS)
    job = next((path for path in jobs.iterdir() if path.is_dir()), None)
    if job is None:
        return results, len(FRONTIER_TASKS)
    for trial in job.iterdir():
        if not trial.is_dir():
            continue
        task = trial.name.split("__", 1)[0]
        result_file = trial / "result.json"
        if not result_file.exists():
            active += 1
            continue
        try:
            value = json.loads(result_file.read_text())
        except (OSError, json.JSONDecodeError):
            active += 1
            continue
        rewards = ((value.get("verifier_result") or {}).get("rewards") or {})
        reward = rewards.get("reward")
        if reward is None and rewards:
            reward = next(iter(rewards.values()))
        try:
            reward = float(reward or 0.0)
        except (TypeError, ValueError):
            reward = 0.0
        reward = reward if math.isfinite(reward) and 0 <= reward <= 1 else 0.0
        exception = value.get("exception_info") or {}
        results[task] = {
            "status": "failed" if exception else "done",
            "reward": reward,
            "error": str(exception.get("exception_message") or "")[:1000] or None,
        }
    active += max(0, len(FRONTIER_TASKS) - len(results) - active)
    return results, active


def bubblewrap_harbor(
    repo: Path,
    dataset: Path,
    jobs: Path,
    gateway: Gateway,
    podman_socket: Path,
    docker_config: Path,
    builder_name: str,
) -> list[str]:
    home = Path.home()
    sandbox_root = jobs.parent.resolve()
    try:
        repo.resolve().relative_to(sandbox_root)
        dataset.resolve().relative_to(sandbox_root)
        jobs.resolve().relative_to(sandbox_root)
    except ValueError as exc:
        raise InfrastructureFailed("Frontier paths must share the run work directory") from exc
    repo_mount = repo.resolve()
    dataset_mount = dataset.resolve()
    jobs_mount = jobs.resolve()
    try:
        docker_config.resolve().relative_to(podman_socket.parent.resolve())
    except ValueError as exc:
        raise InfrastructureFailed("Buildx config must share the Podman socket directory") from exc
    command = [
        "bwrap", "--die-with-parent", "--new-session", "--unshare-pid", "--unshare-uts",
        "--unshare-ipc", "--unshare-net", "--ro-bind", "/", "/", "--tmpfs", "/home",
        "--dir", str(home), "--dir", str(home / ".local"), "--dir", str(home / ".local/share"),
        "--dir", str(home / ".local/share/uv"), "--dir", str(home / ".local/share/uv/tools"),
        "--ro-bind", str(HARBOR_ROOT), str(HARBOR_ROOT), "--tmpfs", "/run",
        "--dir", "/run/harness", "--ro-bind", str(gateway.directory), "/run/harness",
        "--tmpfs", "/tmp", "--dir", str(sandbox_root),
        "--ro-bind", str(repo_mount), str(repo_mount),
        "--ro-bind", str(dataset_mount), str(dataset_mount),
        "--bind", str(jobs_mount), str(jobs_mount),
        "--dir", str(podman_socket.parent),
        "--bind", str(podman_socket.parent), str(podman_socket.parent),
        "--proc", "/proc", "--dev", "/dev",
        "--setenv", "HOME", "/tmp/home", "--setenv", "DOCKER_HOST", f"unix://{podman_socket}",
        "--setenv", "DOCKER_CONFIG", str(docker_config),
        "--setenv", "BUILDX_BUILDER", builder_name,
        "--setenv", "PYTHONPATH", str(repo_mount), "--setenv", "OPENAI_API_KEY", gateway.token,
        "--setenv", "OPENAI_API_BASE", "http://127.0.0.1:8000/v1",
        "--setenv", "OPENAI_BASE_URL", "http://127.0.0.1:8000/v1",
        "--chdir", str(repo_mount), "sh", "-lc",
        "mkdir -p /tmp/home && "
        "socat TCP-LISTEN:8000,bind=127.0.0.1,reuseaddr,fork UNIX-CONNECT:/run/harness/bedrock.sock & "
        f"exec {shlex.quote(str(HARBOR_ROOT / 'bin/python'))} "
        f"{shlex.quote(str(HARBOR_CLI))} run -p {shlex.quote(str(dataset_mount))} "
        "-a agent.harbor_agent:HarborAgent "
        f"-m openai/{BEDROCK_PROFILE_NAME} -n 5 -o {shlex.quote(str(jobs_mount))} "
        "-y --no-force-build "
        "--ak enable_summarize=false --ak proactive_summarization_threshold=0 "
        "--ak max_turns=1000000 --ak temperature=0 --ak api_base=http://127.0.0.1:8000/v1",
    ]
    return command


def frontier_cutoff(global_deadline: float, now: float | None = None) -> float:
    started = time.monotonic() if now is None else now
    return min(global_deadline, started + FRONTIER_DEADLINE_SECONDS)


def run_buildx_command(
    command: list[str], env: dict[str, str], deadline: float, cap: float, operation: str,
) -> subprocess.CompletedProcess[str]:
    try:
        result = subprocess.run(
            command, env=env, capture_output=True, text=True,
            timeout=bounded_timeout(deadline, cap),
        )
    except subprocess.TimeoutExpired as exc:
        raise InfrastructureFailed(f"Buildx builder {operation} timed out") from exc
    except OSError as exc:
        raise InfrastructureFailed(f"Buildx builder {operation} could not run: {exc}") from exc
    if result.returncode != 0:
        detail = (result.stderr or result.stdout)[-2000:]
        raise InfrastructureFailed(f"Buildx builder {operation} failed: {detail}")
    return result


def cleanup_buildx_builder(builder_name: str, builder_env: dict[str, str]) -> None:
    builder_container = f"buildx_buildkit_{builder_name}0"
    cleanup_ok = False
    with contextlib.suppress(OSError, subprocess.TimeoutExpired):
        cleanup_ok = subprocess.run(
            ["docker", "buildx", "rm", "--force", builder_name],
            env=builder_env, capture_output=True, timeout=15,
        ).returncode == 0
    if cleanup_ok:
        return
    with contextlib.suppress(OSError, subprocess.TimeoutExpired):
        subprocess.run(
            ["podman", "rm", "-f", builder_container],
            capture_output=True, timeout=15,
        )
    with contextlib.suppress(OSError, subprocess.TimeoutExpired):
        subprocess.run(
            ["podman", "volume", "rm", "-f", f"{builder_container}_state"],
            capture_output=True, timeout=15,
        )


def run_frontier(run_id: str, repo: Path, work: Path, gateway: Gateway, reporter: Reporter, deadline: float) -> float:
    dataset = sprint_dataset(work)
    jobs = work / "frontier-jobs"
    jobs.mkdir()
    # AF_UNIX paths are limited to 107 bytes on Linux. Run work directories include a
    # UUID and can exceed that locally, so keep the private Podman API socket short.
    socket_directory = Path(tempfile.mkdtemp(prefix=f"exposure-hb-{run_id[:8]}-", dir="/tmp"))
    socket_path = socket_directory / "podman.sock"
    docker_config = socket_directory / "docker"
    builder_name = f"exposure-harbor-{run_id[:8]}-{secrets.token_hex(3)}"
    builder_env = os.environ.copy()
    builder_env.update({
        "DOCKER_HOST": f"unix://{socket_path}",
        "DOCKER_CONFIG": str(docker_config),
        "BUILDX_BUILDER": builder_name,
    })
    service: subprocess.Popen | None = None
    process: subprocess.Popen | None = None
    stage_timed_out = False
    log_file = work / "frontier.log"
    try:
        service = subprocess.Popen(
            ["podman", "system", "service", "--time=0", f"unix://{socket_path}"],
            stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, text=True,
        )
        service_ready_deadline = min(deadline, time.monotonic() + 5)
        while (not socket_path.exists() and service.poll() is None
               and time.monotonic() < service_ready_deadline):
            time.sleep(0.05)
        if not socket_path.exists():
            if service.poll() is None:
                service.terminate()
                with contextlib.suppress(subprocess.TimeoutExpired):
                    service.wait(timeout=2)
            if service.poll() is None:
                service.kill()
                with contextlib.suppress(subprocess.TimeoutExpired):
                    service.wait(timeout=2)
            detail = ""
            if service.stderr:
                with contextlib.suppress(OSError):
                    os.set_blocking(service.stderr.fileno(), False)
                with contextlib.suppress(OSError, BlockingIOError):
                    detail = (service.stderr.read() or "")[-1000:]
            raise InfrastructureFailed(f"rootless Podman API did not start: {detail}")
        docker_config.mkdir(mode=0o700)
        buildkit_config = socket_directory / "buildkitd.toml"
        buildkit_config.write_text(
            "[dns]\n  nameservers = " + json.dumps(buildkit_nameservers()) + "\n"
        )
        run_buildx_command(
            [
                "docker", "buildx", "create", "--name", builder_name,
                "--driver", "docker-container", "--driver-opt", "network=host",
                "--driver-opt", "memory=2g", "--driver-opt", "memory-swap=2g",
                "--driver-opt", "cpu-quota=200000",
                "--buildkitd-config", str(buildkit_config),
                "--use", builder_env["DOCKER_HOST"],
            ],
            builder_env, deadline, 10, "creation",
        )
        run_buildx_command(
            ["docker", "buildx", "inspect", "--bootstrap", builder_name],
            builder_env, deadline, 60, "startup",
        )
        reporter.update(
            "frontier", status="running", done=0, total=len(FRONTIER_TASKS),
            tasks={}, rate=0, max_seconds=FRONTIER_DEADLINE_SECONDS,
        )
        with log_file.open("w") as output:
            frontier_deadline = frontier_cutoff(deadline)
            process = subprocess.Popen(
                bubblewrap_harbor(
                    repo, dataset, jobs, gateway, socket_path, docker_config, builder_name,
                ),
                stdout=output, stderr=subprocess.STDOUT, text=True, start_new_session=True,
            )
        next_rate_check = time.monotonic() + 30
        while process.poll() is None:
            if time.monotonic() >= frontier_deadline:
                stage_timed_out = True
                with contextlib.suppress(ProcessLookupError):
                    os.killpg(process.pid, signal.SIGTERM)
                break
            results, active = scan_frontier(jobs)
            reporter.update("frontier", status="running", done=len(results), total=len(FRONTIER_TASKS),
                            tasks=results, active=active)
            now = time.monotonic()
            if active and now >= next_rate_check:
                metrics = gateway.metrics()
                completed = int(metrics["completed_last_30s"])
                reporter.update("frontier", status="running", done=len(results), total=len(FRONTIER_TASKS),
                                tasks=results, active=active, rate=completed)
                next_rate_check = now + 5
            time.sleep(2)
        with contextlib.suppress(subprocess.TimeoutExpired):
            process.wait(timeout=5)
    finally:
        if process is not None and process.poll() is None:
            with contextlib.suppress(ProcessLookupError):
                os.killpg(process.pid, signal.SIGTERM)
            with contextlib.suppress(subprocess.TimeoutExpired):
                process.wait(timeout=3)
            if process.poll() is None:
                with contextlib.suppress(ProcessLookupError):
                    os.killpg(process.pid, signal.SIGKILL)
                with contextlib.suppress(subprocess.TimeoutExpired):
                    process.wait(timeout=2)
        if docker_config.exists():
            cleanup_buildx_builder(builder_name, builder_env)
        if service is not None and service.poll() is None:
            service.terminate()
            with contextlib.suppress(subprocess.TimeoutExpired):
                service.wait(timeout=3)
            if service.poll() is None:
                service.kill()
                with contextlib.suppress(subprocess.TimeoutExpired):
                    service.wait(timeout=2)
        cleanup_harbor_containers(jobs)
        shutil.rmtree(socket_directory, ignore_errors=True)
    results, _ = scan_frontier(jobs)
    assert process is not None
    if not stage_timed_out and process.returncode not in (0, -signal.SIGTERM) and not results:
        raise RunFailed("Frontier Sprint could not start:\n" + log_file.read_text(errors="replace")[-3000:])
    for task in FRONTIER_TASKS:
        results.setdefault(task, {"status": "timeout", "reward": 0.0})
    score = round(100 * sum(float(results[task]["reward"]) for task in FRONTIER_TASKS) / len(FRONTIER_TASKS), 1)
    reporter.update("frontier", status="done", done=len(FRONTIER_TASKS), total=len(FRONTIER_TASKS),
                    tasks=results, score=score, gateway=gateway.metrics())
    return score


def terminal_status(state: dict[str, dict[str, Any]]) -> str:
    statuses = [item.get("status") for item in state.values() if item.get("status") != "skipped"]
    if not statuses:
        return "infra_failed"
    if all(status == "done" for status in statuses):
        return "done"
    if any(status == "done" for status in statuses):
        return "partial"
    if any(status == "infra_failed" for status in statuses):
        return "infra_failed"
    return "failed"


def process_claim(claim: dict[str, Any]) -> None:
    run_id = str(claim["id"])
    lease_token = str(claim["lease_token"])
    deadline = parse_deadline(str(claim["deadline_at"]))
    work = Path(tempfile.mkdtemp(prefix=f"harness-{run_id[:8]}-"))
    gateways: list[Gateway] = []
    scores: dict[str, float | None] = {"arc": None, "frontier": None, "ram1": None, "ram10": None}
    error_log: str | None = None
    with Lease(run_id, lease_token, deadline) as lease:
        reporter = Reporter(lease)
        try:
            repo, sha, venv = clone_submission(str(claim["repo_url"]), work, deadline)
            profile_fingerprint = hashlib.sha256(BEDROCK_MODEL_ID.encode()).hexdigest()
            api("/api/worker/harness/stage", {
                "id": run_id,
                "lease_token": lease_token,
                "status": "running",
                "commit_sha": sha,
                "bedrock_profile": BEDROCK_PROFILE_NAME,
                "bedrock_profile_fingerprint": profile_fingerprint,
            })
            ram_gateway = Gateway("ram", work)
            gateways.append(ram_gateway)
            arc_gateway = Gateway("arc", work)
            gateways.append(arc_gateway)
            frontier_gateway = Gateway("frontier", work)
            gateways.append(frontier_gateway)

            try:
                with ram_lane(deadline):
                    scores["ram1"], scores["ram10"] = run_ram(
                        run_id, repo, venv, ram_gateway, reporter, deadline
                    )
            except InfrastructureFailed as exc:
                reporter.update("ram", status="infra_failed", error=str(exc))
            except Exception as exc:
                reporter.update("ram", status="failed", error=str(exc)[:2000])

            outcomes: dict[str, tuple[str, Any]] = {}
            lock = threading.Lock()

            def benchmark(name: str, fn: Any) -> None:
                try:
                    value = fn()
                    with lock:
                        outcomes[name] = ("done", value)
                except InfrastructureFailed as exc:
                    with contextlib.suppress(Exception):
                        reporter.update(name, status="infra_failed", error=str(exc))
                    with lock:
                        outcomes[name] = ("infra_failed", exc)
                except Exception as exc:
                    with contextlib.suppress(Exception):
                        reporter.update(name, status="failed", error=str(exc)[:2000])
                    with lock:
                        outcomes[name] = ("failed", exc)

            arc_thread = threading.Thread(
                target=benchmark,
                args=("arc", lambda: run_arc(run_id, repo, venv, arc_gateway, reporter, deadline)),
                name=f"arc-{run_id[:8]}",
            )
            frontier_thread = threading.Thread(
                target=benchmark,
                args=("frontier", lambda: run_frontier(run_id, repo, work, frontier_gateway, reporter, deadline)),
                name=f"frontier-{run_id[:8]}",
            )
            arc_thread.start()
            frontier_thread.start()
            while arc_thread.is_alive() or frontier_thread.is_alive():
                if lease.lost.is_set():
                    raise LeaseLost("worker lost its run lease")
                if time.monotonic() >= deadline:
                    break
                time.sleep(0.5)
            arc_thread.join(timeout=10)
            frontier_thread.join(timeout=10)
            if arc_thread.is_alive() or frontier_thread.is_alive():
                raise InfrastructureFailed("benchmark threads did not stop at the global deadline")
            if outcomes.get("arc", (None,))[0] == "done":
                scores["arc"] = float(outcomes["arc"][1])
            if outcomes.get("frontier", (None,))[0] == "done":
                scores["frontier"] = float(outcomes["frontier"][1])
        except RunFailed as exc:
            error_log = str(exc)
            for name in ("arc", "frontier", "ram"):
                if reporter.state[name]["status"] == "pending":
                    reporter.state[name] = {"status": "failed", "error": error_log[:2000]}
        except Exception as exc:
            error_log = str(exc)
            for name in ("arc", "frontier", "ram"):
                if reporter.state[name]["status"] == "pending":
                    reporter.state[name] = {"status": "infra_failed", "error": error_log[:2000]}
        finally:
            cleanup_run_containers(run_id)
            for gateway in gateways:
                gateway.stop()
            state = reporter.snapshot()
            status = terminal_status(state)
            if error_log is None and status != "done":
                errors = [str(value.get("error")) for value in state.values() if value.get("error")]
                error_log = "\n".join(errors)[:8000] or None
            try:
                api("/api/worker/harness/result", {
                    "id": run_id,
                    "lease_token": lease_token,
                    "status": status,
                    "benchmark_state": state,
                    "score_arc": scores["arc"],
                    "score_frontier": scores["frontier"],
                    "ram_1session_mb": scores["ram1"],
                    "ram_10session_mb": scores["ram10"],
                    "error_log": error_log,
                })
                log(f"run {run_id} -> {status}")
            except LeaseLost:
                log(f"dropping stale result for {run_id}")
            shutil.rmtree(work, ignore_errors=True)


def process_executor_run(request: dict[str, Any]) -> None:
    """Run benchmark SDK adapters under Rust ownership; emit only bounded NDJSON."""
    global BEDROCK_PROFILE_NAME

    run_id = str(request.get("run_id") or "")
    try:
        uuid.UUID(run_id)
    except ValueError as exc:
        raise InfrastructureFailed("executor request contained an invalid run id") from exc
    deadline = parse_deadline(str(request.get("deadline_at") or ""))
    token = str(request.get("gateway_token") or "")
    socket_path = Path(str(request.get("gateway_socket") or ""))
    profile = str(request.get("model_profile") or "")
    benchmark_kind = str(request.get("benchmark_kind") or "")
    if len(token) != 64 or not socket_path.is_socket() \
            or not re.fullmatch(r"[A-Za-z0-9._-]{1,120}", profile) \
            or benchmark_kind not in {"arc", "frontier", "bundled"}:
        raise InfrastructureFailed("executor request contained an invalid model capability")
    BEDROCK_PROFILE_NAME = profile
    for path in (
        ARC_PYTHON, ARC_STARTER / "environment_files", FRONTIER_SOURCE, HARBOR_CLI,
    ):
        if not path.exists():
            raise InfrastructureFailed(f"required benchmark cache is missing: {path}")
    ensure_image()

    work = Path.cwd()
    reporter = NdjsonReporter(run_id, deadline, benchmark_kind)
    gateway = ExternalGateway(socket_path, token)
    scores: dict[str, float | None] = {"arc": None, "frontier": None, "ram1": None, "ram10": None}
    error_log: str | None = None
    try:
        repo, sha, venv = clone_submission(str(request.get("repo_url") or ""), work, deadline)
        emit({"type": "ready", "commit_sha": sha})

        try:
            with ram_lane(deadline):
                scores["ram1"], scores["ram10"] = run_ram(
                    run_id, repo, venv, gateway, reporter, deadline
                )
        except InfrastructureFailed as exc:
            reporter.update("ram", status="infra_failed", error=str(exc)[:2000])
        except Exception as exc:
            reporter.update("ram", status="failed", error=str(exc)[:2000])

        outcomes: dict[str, tuple[str, Any]] = {}
        lock = threading.Lock()

        def benchmark(name: str, fn: Any) -> None:
            try:
                value = fn()
                with lock:
                    outcomes[name] = ("done", value)
            except InfrastructureFailed as exc:
                with contextlib.suppress(Exception):
                    reporter.update(name, status="infra_failed", error=str(exc)[:2000])
                with lock:
                    outcomes[name] = ("infra_failed", exc)
            except Exception as exc:
                with contextlib.suppress(Exception):
                    reporter.update(name, status="failed", error=str(exc)[:2000])
                with lock:
                    outcomes[name] = ("failed", exc)

        threads: list[threading.Thread] = []
        if benchmark_kind in {"arc", "bundled"}:
            threads.append(threading.Thread(
                target=benchmark,
                args=("arc", lambda: run_arc(run_id, repo, venv, gateway, reporter, deadline)),
                name=f"arc-{run_id[:8]}",
            ))
        if benchmark_kind in {"frontier", "bundled"}:
            threads.append(threading.Thread(
                target=benchmark,
                args=("frontier", lambda: run_frontier(
                    run_id, repo, work, gateway, reporter, deadline
                )),
                name=f"frontier-{run_id[:8]}",
            ))
        for thread in threads:
            thread.start()
        while any(thread.is_alive() for thread in threads):
            if time.monotonic() >= deadline:
                break
            time.sleep(0.5)
        for thread in threads:
            thread.join(timeout=10)
        if any(thread.is_alive() for thread in threads):
            raise InfrastructureFailed("benchmark adapters did not stop at the global deadline")
        if outcomes.get("arc", (None,))[0] == "done":
            scores["arc"] = float(outcomes["arc"][1])
        if outcomes.get("frontier", (None,))[0] == "done":
            scores["frontier"] = float(outcomes["frontier"][1])
    except RunFailed as exc:
        error_log = str(exc)
        for name in ("arc", "frontier", "ram"):
            if reporter.state[name]["status"] == "pending":
                reporter.state[name] = {"status": "failed", "error": error_log[:2000]}
    except Exception as exc:
        error_log = str(exc)
        for name in ("arc", "frontier", "ram"):
            if reporter.state[name]["status"] == "pending":
                reporter.state[name] = {"status": "infra_failed", "error": error_log[:2000]}
    finally:
        cleanup_run_containers(run_id)
        state = reporter.snapshot()
        status = terminal_status(state)
        if error_log is None and status != "done":
            errors = [str(value.get("error")) for value in state.values() if value.get("error")]
            error_log = "\n".join(errors)[:8000] or None
        emit({
            "type": "result",
            "status": status,
            "benchmark_state": state,
            "score_arc": scores["arc"],
            "score_frontier": scores["frontier"],
            "ram_1session_mb": scores["ram1"],
            "ram_10session_mb": scores["ram10"],
            "error_log": error_log,
        })


def kaggle_environment(token: str, home: Path) -> dict[str, str]:
    allowed = (
        "PATH", "LANG", "LC_ALL", "SSL_CERT_FILE", "REQUESTS_CA_BUNDLE",
        "HTTP_PROXY", "HTTPS_PROXY", "NO_PROXY",
    )
    env = {name: os.environ[name] for name in allowed if name in os.environ}
    env.update({"HOME": str(home), "KAGGLE_API_TOKEN": token})
    return env


def kaggle_result(claim: dict[str, Any], status: str, **values: Any) -> None:
    payload = {
        "id": str(claim["id"]),
        "lease_token": str(claim["lease_token"]),
        "status": status,
        "kernel_slug": values.get("kernel_slug"),
        "kernel_version": values.get("kernel_version"),
        "submission_ref": values.get("submission_ref"),
        "public_score": values.get("public_score"),
        "private_score": values.get("private_score"),
        "status_message": str(values.get("status_message") or "")[:2000] or None,
    }
    if KAGGLE_NDJSON:
        emit({"type": "kaggle_result", "result": payload})
    else:
        api("/api/worker/harness/kaggle/result", payload)


def optional_score(value: Any) -> float | None:
    if value in (None, "", "null", "None"):
        return None
    try:
        score = float(value)
    except (TypeError, ValueError):
        return None
    return score if math.isfinite(score) and 0 <= score <= 100 else None


def kaggle_submission_row(
    competition: str, description: str, env: dict[str, str], timeout: float,
) -> dict[str, Any] | None:
    result = run_checked([
        str(KAGGLE_CLI), "competitions", "submissions", competition,
        "--format", "json", "--quiet",
    ], timeout=timeout, env=env)
    try:
        rows = json.loads(result.stdout)
    except json.JSONDecodeError as exc:
        raise RunFailed("Kaggle returned invalid submission-list JSON") from exc
    if not isinstance(rows, list):
        raise RunFailed("Kaggle returned an invalid submission list")
    return next(
        (row for row in rows if isinstance(row, dict) and row.get("description") == description),
        None,
    )


def report_kaggle_poll(
    claim: dict[str, Any], row: dict[str, Any] | None, description: str,
) -> None:
    kernel_slug = str(claim.get("kernel_slug") or "")
    kernel_version = int(claim.get("kernel_version") or 0)
    reference = str(claim.get("submission_ref") or description)
    if row is None:
        kaggle_result(
            claim, "submitted", kernel_slug=kernel_slug, kernel_version=kernel_version,
            submission_ref=reference, status_message="Kaggle submission is waiting to appear.",
        )
        return
    reference = str(row.get("ref") or reference)
    public_score = optional_score(row.get("publicScore"))
    private_score = optional_score(row.get("privateScore"))
    kaggle_status = str(row.get("status") or "pending")
    lowered = kaggle_status.lower()
    if any(word in lowered for word in ("error", "fail", "cancel")):
        status = "failed"
    elif public_score is not None or private_score is not None \
            or lowered in ("complete", "completed", "scored", "finished"):
        status = "scored"
    else:
        status = "submitted"
    kaggle_result(
        claim, status, kernel_slug=kernel_slug, kernel_version=kernel_version,
        submission_ref=reference, public_score=public_score, private_score=private_score,
        status_message=f"Kaggle status: {kaggle_status}",
    )


def process_kaggle_claim(claim: dict[str, Any]) -> None:
    phase = str(claim.get("phase") or "")
    run_id = str(claim.get("run_id") or "")
    job_id = str(claim.get("id") or "")
    token = str(claim.get("token") or "")
    username = str(claim.get("username") or "")
    commit_sha = str(claim.get("commit_sha") or "").lower()
    competition = str(claim.get("competition") or "")
    if phase not in ("submit", "poll") or competition != "arc-prize-2026-arc-agi-3" \
            or str(claim.get("benchmark_version")) != HARNESS_VERSION:
        raise InfrastructureFailed("official submission claim did not match this worker version")
    for value in (run_id, job_id, str(claim.get("lease_token") or "")):
        try:
            uuid.UUID(value)
        except ValueError as exc:
            raise InfrastructureFailed("official submission claim contained an invalid identifier") from exc
    if not re.fullmatch(r"[A-Za-z0-9_-]{2,64}", username) or not 20 <= len(token) <= 500:
        raise InfrastructureFailed("official submission claim contained invalid credentials")
    kernel_slug = str(claim.get("kernel_slug") or f"exposure-{run_id.replace('-', '')[:20]}")
    description = f"Exposure Academy {run_id} {commit_sha[:12]}"
    work = Path(tempfile.mkdtemp(
        prefix=f"kaggle-{job_id[:8]}-",
        dir=Path.cwd() if KAGGLE_NDJSON else None,
    ))
    home = work / "home"
    home.mkdir(mode=0o700)
    env = kaggle_environment(token, home)
    try:
        if phase == "poll":
            try:
                row = kaggle_submission_row(competition, description, env, timeout=30)
                report_kaggle_poll(claim, row, description)
            except RunFailed as exc:
                safe_error = str(exc).replace(token, "[redacted]")
                kaggle_result(
                    claim, "submitted", kernel_slug=kernel_slug,
                    kernel_version=int(claim.get("kernel_version") or 0),
                    submission_ref=str(claim.get("submission_ref") or description),
                    status_message=f"Kaggle poll will retry: {safe_error}",
                )
            return

        repo = clone_exact_submission(str(claim.get("repo_url") or ""), commit_sha, work)
        bundle = work / "bundle"
        run_checked([
            str(ARC_PYTHON), str(ROOT / "kaggle_bundle.py"), "--repo", str(repo),
            "--output", str(bundle), "--username", username, "--slug", kernel_slug,
            "--starter", str(ARC_STARTER),
        ], timeout=30)
        pushed = run_checked(
            [str(KAGGLE_CLI), "kernels", "push", "-p", str(bundle)],
            timeout=120, env=env,
        )
        match = re.search(r"Kernel version (\d+) successfully pushed", pushed.stdout)
        if match is None:
            raise RunFailed("Kaggle did not return the pushed kernel version")
        kernel_version = int(match.group(1))
        kernel_ref = f"{username}/{kernel_slug}"
        run_checked([
            str(KAGGLE_CLI), "competitions", "submit", competition,
            "-k", kernel_ref, "-v", str(kernel_version), "-m", description,
        ], timeout=60, env=env)
        kaggle_result(
            claim, "submitted", kernel_slug=kernel_ref, kernel_version=kernel_version,
            submission_ref=description, status_message="Kaggle accepted the notebook submission.",
        )
        log(f"official submission {job_id} -> submitted")
    except RunFailed as exc:
        safe_error = str(exc).replace(token, "[redacted]")
        kaggle_result(claim, "failed", status_message=safe_error)
        log(f"official submission {job_id} -> failed")
    finally:
        shutil.rmtree(work, ignore_errors=True)


def selfcheck() -> None:
    ensure_host()
    ensure_image()
    print(json.dumps({
        "ok": True,
        "version": HARNESS_VERSION,
        "image": HARNESS_IMAGE,
        "arc_games": list(ARC_GAMES),
        "frontier_tasks": list(FRONTIER_TASKS),
        "bedrock_profile": BEDROCK_PROFILE_NAME,
    }, indent=2))


def executor_main(kaggle: bool) -> None:
    global NDJSON_MODE, KAGGLE_NDJSON

    NDJSON_MODE = True
    KAGGLE_NDJSON = kaggle
    line = sys.stdin.readline()
    request = json.loads(line)
    expected = "kaggle" if kaggle else "run"
    if not isinstance(request, dict) or request.get("type") != expected:
        raise InfrastructureFailed(f"expected an executor {expected} request")
    if kaggle:
        claim = request.get("claim")
        if not isinstance(claim, dict):
            raise InfrastructureFailed("executor Kaggle request is missing its claim")
        try:
            process_kaggle_claim(claim)
        except Exception as exc:
            kaggle_result(claim, "failed", status_message=str(exc)[:2000])
    else:
        process_executor_run(request)


def main() -> None:
    ensure_host()
    ensure_image()
    once = "--once" in sys.argv
    log(f"runner ready: site={SITE} version={HARNESS_VERSION} profile={BEDROCK_PROFILE_NAME}")
    while True:
        try:
            did_work = False
            claim = api("/api/worker/harness/claim", {})
            if claim:
                log(f"claimed {claim['id']} ({claim['repo_url']})")
                process_claim(claim)
                did_work = True
            official = api("/api/worker/harness/kaggle/claim", {})
            if official:
                log(f"claimed official submission {official['id']} ({official['phase']})")
                process_kaggle_claim(official)
                did_work = True
            if once:
                if not did_work:
                    log("no queued run or official submission")
                return
            if not did_work:
                time.sleep(POLL_SECONDS)
        except KeyboardInterrupt:
            return
        except Exception as exc:
            log(f"worker loop error: {exc}")
            if once:
                raise
            time.sleep(POLL_SECONDS)


if __name__ == "__main__":
    if "--executor-ndjson" in sys.argv:
        executor_main(False)
    elif "--executor-kaggle-ndjson" in sys.argv:
        executor_main(True)
    elif "--selfcheck" in sys.argv:
        selfcheck()
    else:
        main()
