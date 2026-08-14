#!/usr/bin/env python3
"""Academy-host controller: validates submissions and owns the CPU Colab fleet."""

from __future__ import annotations

import argparse
import gzip
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import signal
import subprocess
import sys
import tarfile
import tempfile
import threading
import time
import urllib.error
import urllib.request

from monopoly_game_engine import MonopolyEnv, action_to_description, build_state_vector
from rl_monopoly_runner import (
    agent_image_fingerprint,
    build_snapshot,
    docker_agent,
    effective_vcpus,
    machine_shape,
    system_ram_bytes,
)


ROOT = Path(__file__).resolve().parent
REPO_ROOT = ROOT.parents[1]


def local_env() -> dict[str, str]:
    values: dict[str, str] = {}
    try:
        for line in (REPO_ROOT / ".env").read_text().splitlines():
            line = line.strip()
            if line and not line.startswith("#") and "=" in line:
                key, value = line.split("=", 1)
                values[key] = value.strip().strip('"')
    except OSError:
        pass
    return values


LOCAL_ENV = local_env()
SITE = os.environ.get(
    "MONOPOLY_SITE", LOCAL_ENV.get("MONOPOLY_SITE", "http://127.0.0.1:3000")
).rstrip("/")
WORKER_TOKEN = os.environ.get("WORKER_TOKEN", LOCAL_ENV.get("WORKER_TOKEN", ""))
ARTIFACT_DIR = Path(
    os.environ.get(
        "MONOPOLY_ARTIFACT_DIR",
        LOCAL_ENV.get(
            "MONOPOLY_ARTIFACT_DIR", str(REPO_ROOT / "var/monopoly-artifacts")
        ),
    )
).resolve()
VALIDATION_WORKER_ID = os.environ.get(
    "MONOPOLY_VALIDATION_WORKER_ID", "academy-validation"
)
COLAB_SESSIONS = tuple(f"ai-monopoly-{index}" for index in range(1, 6))
REMOTE_BUNDLE = "/content/exposure-monopoly-worker.tar.gz"
REMOTE_CONFIG = "/content/exposure-monopoly-config.json"
ACTIVE_PROCESSES: set[subprocess.Popen] = set()
ACTIVE_PROCESSES_LOCK = threading.Lock()

# Colab sessions get their own remote hardware floor via colab_preflight.py.
# This is the floor for the *local* host worker(s) started by Fleet.start();
# it's low by default because Docker's own per-container caps (see
# docker_agent) and the systemd unit's cgroup limits are the real backstop,
# not a self-reported RAM/CPU heuristic.
MIN_HOST_RAM_BYTES = int(
    os.environ.get("MONOPOLY_MIN_RAM_BYTES", str(2 * 1024**3))
)
MIN_HOST_VCPUS = float(os.environ.get("MONOPOLY_MIN_VCPUS", "1"))

REPO_MAX_BYTES = 250 * 1024**2
ARTIFACT_MAX_BYTES = 2 * 1024**3
REPO_RE = re.compile(r"https://github\.com/[A-Za-z0-9_.-]{1,39}/[A-Za-z0-9_.-]{1,100}")
REQUIREMENT_RE = re.compile(
    r"[A-Za-z0-9][A-Za-z0-9._-]*(?:\[[A-Za-z0-9._,-]+\])?"
    r"(?:\s*(?:===|==|~=|!=|<=|>=|<|>)\s*[A-Za-z0-9][A-Za-z0-9.*+!_-]*)?"
    r"(?:\s*,\s*(?:===|==|~=|!=|<=|>=|<|>)\s*[A-Za-z0-9][A-Za-z0-9.*+!_-]*)*"
    r"(?:\s*;\s*[A-Za-z0-9_.-]+\s*(?:==|!=|<=|>=|<|>)\s*['\"][^'\"]+['\"])?"
)


def log(message: str) -> None:
    print(f"[{time.strftime('%H:%M:%S')}] {message}", flush=True)


class ApiError(RuntimeError):
    def __init__(self, status: int | None, message: str):
        super().__init__(message)
        self.status = status


class ControllerStopping(KeyboardInterrupt):
    pass


def tracked_run(command: list[str], **kwargs) -> subprocess.CompletedProcess:
    """Run a child so SIGTERM can kill it before remote-session cleanup begins."""
    timeout = kwargs.pop("timeout", None)
    check = kwargs.pop("check", False)
    process = subprocess.Popen(command, **kwargs)
    with ACTIVE_PROCESSES_LOCK:
        ACTIVE_PROCESSES.add(process)
    try:
        stdout, stderr = process.communicate(timeout=timeout)
    except subprocess.TimeoutExpired:
        terminate_process(process)
        raise
    finally:
        with ACTIVE_PROCESSES_LOCK:
            ACTIVE_PROCESSES.discard(process)
    result = subprocess.CompletedProcess(command, process.returncode, stdout, stderr)
    if check:
        result.check_returncode()
    return result


def interrupt_children() -> None:
    with ACTIVE_PROCESSES_LOCK:
        processes = list(ACTIVE_PROCESSES)
    for process in processes:
        terminate_process(process, timeout=2)


def api(path: str, body: dict | None = None, *, timeout: float = 60) -> dict | None:
    request = urllib.request.Request(
        SITE + path,
        data=None if body is None else json.dumps(body, separators=(",", ":")).encode(),
        headers={
            "X-Worker-Token": WORKER_TOKEN,
            "Content-Type": "application/json",
            "User-Agent": "exposure-monopoly-controller/1",
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


def report_fleet(
    name: str,
    status: str,
    *,
    ram: int | None = None,
    cpus: float | None = None,
    shape: str | None = None,
    reason: str | None = None,
) -> None:
    try:
        api(
            "/api/worker/rl-monopoly/resource",
            {
                "worker_id": f"colab:{name}",
                "kind": "colab",
                "session_name": name,
                "status": status,
                "ram_bytes": ram,
                "effective_vcpus": cpus,
                "machine_shape": shape,
                "preflight_reason": reason,
                "current_game_id": None,
            },
        )
    except ApiError as error:
        log(f"could not report {name} {status} preflight to Academy: {error}")


def report_host(
    name: str, status: str, ram: int, cpus: float, reason: str | None = None
) -> None:
    try:
        api(
            "/api/worker/rl-monopoly/resource",
            {
                "worker_id": name,
                "kind": "host",
                "session_name": name,
                "status": status,
                "ram_bytes": ram,
                "effective_vcpus": cpus,
                "machine_shape": machine_shape(),
                "preflight_reason": reason,
                "current_game_id": None,
            },
        )
    except ApiError as error:
        log(f"could not report host {status} preflight to Academy: {error}")


class ValidationFailed(RuntimeError):
    pass


class ValidationLease:
    def __init__(self, job: dict):
        self.payload = {
            "worker_id": VALIDATION_WORKER_ID,
            "id": job["id"],
            "generation": job["generation"],
            "lease_id": job["lease_id"],
        }
        self.stop = threading.Event()
        self.error: Exception | None = None
        self.thread = threading.Thread(
            target=self._run, name="validation-heartbeat", daemon=True
        )

    def _run(self) -> None:
        while not self.stop.wait(60):
            try:
                api(
                    "/api/worker/rl-monopoly/submissions/heartbeat",
                    self.payload,
                    timeout=30,
                )
            except Exception as error:
                self.error = error
                self.stop.set()

    def check(self) -> None:
        if self.error:
            raise self.error

    def __enter__(self):
        self.thread.start()
        return self

    def __exit__(self, *_):
        self.stop.set()
        self.thread.join(timeout=2)


def run_logged(
    command: list[str],
    log_file,
    *,
    cwd: Path | None = None,
    env: dict | None = None,
    timeout: float = 300,
    text: bool = False,
):
    log_file.write(("$ " + " ".join(command) + "\n").encode())
    log_file.flush()
    try:
        result = tracked_run(
            command,
            cwd=cwd,
            env=env,
            stdout=subprocess.PIPE if text else log_file,
            stderr=subprocess.STDOUT,
            timeout=timeout,
            text=text,
        )
    except FileNotFoundError as error:
        raise ValidationFailed(f"missing executable: {command[0]}") from error
    except subprocess.TimeoutExpired as error:
        raise ValidationFailed(
            f"timed out after {timeout:.0f}s: {command[0]}"
        ) from error
    if result.returncode:
        if text and result.stdout:
            log_file.write(result.stdout.encode(errors="replace"))
        raise ValidationFailed(
            f"command failed ({result.returncode}): {' '.join(command[:3])}"
        )
    return result.stdout if text else None


def log_tail(path: Path, limit: int = 12_000) -> str:
    try:
        with open(path, "rb") as source:
            source.seek(0, os.SEEK_END)
            source.seek(max(0, source.tell() - limit))
            return source.read().decode(errors="replace")
    except OSError:
        return ""


def parse_default_head(output: str) -> tuple[str, str]:
    branch = None
    sha = None
    for line in output.splitlines():
        if line.startswith("ref: ") and line.endswith("\tHEAD"):
            branch = line[5:].split("\t", 1)[0]
        elif line.endswith("\tHEAD"):
            candidate = line.split("\t", 1)[0].lower()
            if len(candidate) == 40 and all(
                character in "0123456789abcdef" for character in candidate
            ):
                sha = candidate
    if not branch or not sha:
        raise ValidationFailed("repository default branch could not be pinned")
    return branch, sha


def inspect_git_tree(checkout: Path, commit: str, log_file) -> None:
    output = subprocess.run(
        ["git", "-C", str(checkout), "ls-tree", "-rz", "--full-tree", commit],
        capture_output=True,
        timeout=30,
        check=True,
    ).stdout
    seen_casefold: set[str] = set()
    for record in output.split(b"\0"):
        if not record:
            continue
        try:
            metadata, raw_path = record.split(b"\t", 1)
            mode = metadata.split(b" ", 1)[0]
            path = raw_path.decode("utf-8", errors="strict")
        except (ValueError, UnicodeDecodeError) as error:
            raise ValidationFailed("repository contains an undecodable path") from error
        parts = Path(path).parts
        if (
            not parts
            or path.startswith("/")
            or ".." in parts
            or "\\" in path
            or any(ord(char) < 32 for char in path)
        ):
            raise ValidationFailed(f"unsafe repository path: {path!r}")
        if mode in {b"120000", b"160000"}:
            raise ValidationFailed(f"symlinks and submodules are not allowed: {path}")
        folded = path.casefold()
        if folded in seen_casefold:
            raise ValidationFailed(f"case-colliding repository path: {path}")
        seen_casefold.add(folded)
    if (checkout / ".lfsconfig").exists():
        raise ValidationFailed("custom .lfsconfig files are not allowed")


def lfs_pointer_size(path: Path) -> int | None:
    try:
        if path.stat().st_size > 1024:
            return None
        lines = path.read_text(errors="strict").splitlines()
    except (OSError, UnicodeError):
        return None
    if not lines or lines[0] != "version https://git-lfs.github.com/spec/v1":
        return None
    for line in lines:
        if line.startswith("size ") and line[5:].isdigit():
            return int(line[5:])
    raise ValidationFailed(f"invalid Git LFS pointer: {path.name}")


def resolved_tree_size(checkout: Path) -> tuple[int, bool]:
    total = 0
    has_lfs = False
    for path in sorted(checkout.rglob("*")):
        if ".git" in path.relative_to(checkout).parts:
            continue
        if path.is_symlink():
            raise ValidationFailed(
                f"symlink is not allowed: {path.relative_to(checkout)}"
            )
        if not path.is_file():
            continue
        pointer_size = lfs_pointer_size(path)
        if pointer_size is None:
            total += path.stat().st_size
        else:
            has_lfs = True
            total += pointer_size
        if total > REPO_MAX_BYTES:
            raise ValidationFailed("resolved checkout exceeds 250 MiB")
    return total, has_lfs


def validate_requirements(path: Path) -> list[str]:
    if not path.exists():
        return []
    try:
        lines = path.read_text(errors="strict").splitlines()
    except UnicodeError as error:
        raise ValidationFailed("requirements.txt must be UTF-8") from error
    requirements = [
        line.strip()
        for line in lines
        if line.strip() and not line.lstrip().startswith("#")
    ]
    if len(requirements) > 32:
        raise ValidationFailed(
            "requirements.txt may contain at most 32 direct requirements"
        )
    for line in requirements:
        if (
            len(line) > 300
            or "@" in line
            or "://" in line
            or line.startswith("-")
            or not REQUIREMENT_RE.fullmatch(line)
        ):
            raise ValidationFailed(
                f"wheel-only PyPI requirement expected: {line[:100]!r}"
            )
    return requirements


def copy_submission(checkout: Path, destination: Path) -> None:
    destination.mkdir()
    for source in sorted(checkout.rglob("*")):
        relative = source.relative_to(checkout)
        if relative.parts[0] == ".git":
            continue
        target = destination / relative
        if source.is_dir():
            target.mkdir(parents=True, exist_ok=True)
        elif source.is_file() and not source.is_symlink():
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source, target)
            target.chmod(source.stat().st_mode & 0o777)
        else:
            raise ValidationFailed(f"unsupported checkout entry: {relative}")


def normalized_archive(source: Path, destination: Path) -> None:
    with (
        open(destination, "wb") as raw,
        gzip.GzipFile(filename="", fileobj=raw, mode="wb", mtime=0) as compressed,
        tarfile.open(fileobj=compressed, mode="w") as archive,
    ):
        for path in [source, *sorted(source.rglob("*"))]:
            relative = Path(".") if path == source else path.relative_to(source)
            name = "." if relative == Path(".") else relative.as_posix()
            info = archive.gettarinfo(str(path), arcname=name)
            info.uid = info.gid = 0
            info.uname = info.gname = ""
            info.mtime = 0
            if info.isdir():
                info.mode = 0o755
                archive.addfile(info)
            elif info.isfile():
                info.mode = 0o755 if path.stat().st_mode & 0o111 else 0o644
                with open(path, "rb") as payload:
                    archive.addfile(info, payload)
            else:
                raise ValidationFailed(f"artifact refuses non-file member: {relative}")


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with open(path, "rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def prepare_dependencies(checkout: Path, staging: Path, log_file) -> str:
    requirements = checkout / "requirements.txt"
    direct = validate_requirements(requirements)
    wheelhouse = staging / "wheelhouse"
    wheelhouse.mkdir()
    lock = staging / "requirements.lock"
    if not direct:
        lock.write_text("")
        return ""
    run_logged(
        [
            sys.executable,
            "-m",
            "pip",
            "download",
            "--only-binary=:all:",
            "--dest",
            str(wheelhouse),
            "--requirement",
            str(requirements),
        ],
        log_file,
        timeout=600,
    )
    downloads = list(wheelhouse.iterdir())
    if not downloads or any(path.suffix != ".whl" for path in downloads):
        raise ValidationFailed(
            "every direct and transitive dependency must publish a compatible wheel"
        )
    venv = staging / "smoke-venv"
    run_logged([sys.executable, "-m", "venv", str(venv)], log_file, timeout=120)
    python = venv / "bin" / "python"
    run_logged(
        [
            str(python),
            "-m",
            "pip",
            "install",
            "--no-index",
            "--only-binary=:all:",
            f"--find-links={wheelhouse}",
            "--requirement",
            str(requirements),
        ],
        log_file,
        timeout=600,
    )
    frozen = run_logged(
        [str(python), "-m", "pip", "freeze"], log_file, timeout=60, text=True
    )
    lock.write_text(frozen)
    shutil.rmtree(venv)
    return frozen


def validate_submission(job: dict) -> dict:
    repo_url = str(job.get("repo_url", ""))
    agent_path = str(job.get("agent_path", ""))
    if not REPO_RE.fullmatch(repo_url):
        raise ValidationFailed("claim contains an invalid public GitHub repository URL")
    if (
        not agent_path
        or Path(agent_path).is_absolute()
        or ".." in Path(agent_path).parts
        or "\\" in agent_path
    ):
        raise ValidationFailed("claim contains an unsafe agent path")
    ARTIFACT_DIR.mkdir(parents=True, exist_ok=True)
    temporary_archive: Path | None = None
    built_image: str | None = None
    keep_image = False
    with tempfile.NamedTemporaryFile(
        prefix="monopoly-validation-", suffix=".log", delete=False
    ) as log_file:
        log_path = Path(log_file.name)
    try:
        with (
            tempfile.TemporaryDirectory(
                prefix="monopoly-validation-"
            ) as temporary_name,
            open(log_path, "ab") as output,
        ):
            temporary = Path(temporary_name)
            checkout = temporary / "checkout"
            staging = temporary / "artifact"
            staging.mkdir()
            git_home = temporary / "git-home"
            git_home.mkdir()
            git_env = {
                key: value
                for key, value in os.environ.items()
                if not key.startswith("GIT_") and key not in {"GH_TOKEN", "GITHUB_TOKEN"}
            }
            git_env.update(
                {
                    "HOME": str(git_home),
                    "XDG_CONFIG_HOME": str(git_home / ".config"),
                    "GIT_TERMINAL_PROMPT": "0",
                    "GIT_ASKPASS": "/bin/false",
                    "SSH_ASKPASS": "/bin/false",
                    "GIT_LFS_SKIP_SMUDGE": "1",
                    "GIT_CONFIG_NOSYSTEM": "1",
                    "GIT_CONFIG_GLOBAL": "/dev/null",
                    "GIT_ALLOW_PROTOCOL": "https",
                }
            )
            head = run_logged(
                ["git", "ls-remote", "--symref", repo_url, "HEAD"],
                output,
                env=git_env,
                timeout=60,
                text=True,
            )
            _branch, commit = parse_default_head(head)
            run_logged(["git", "init", "--quiet", str(checkout)], output, env=git_env)
            run_logged(
                ["git", "-C", str(checkout), "remote", "add", "origin", repo_url],
                output,
                env=git_env,
            )
            run_logged(
                [
                    "git",
                    "-C",
                    str(checkout),
                    "fetch",
                    "--quiet",
                    "--depth=1",
                    "--no-tags",
                    "origin",
                    commit,
                ],
                output,
                env=git_env,
                timeout=180,
            )
            run_logged(
                [
                    "git",
                    "-C",
                    str(checkout),
                    "checkout",
                    "--quiet",
                    "--detach",
                    "FETCH_HEAD",
                ],
                output,
                env=git_env,
            )
            inspect_git_tree(checkout, commit, output)
            estimated_size, has_lfs = resolved_tree_size(checkout)
            if has_lfs:
                if shutil.which("git-lfs") is None and shutil.which("git"):
                    # Most installations expose LFS as `git lfs`, not a `git-lfs` path.
                    probe = subprocess.run(
                        ["git", "lfs", "version"], capture_output=True
                    )
                    if probe.returncode:
                        raise ValidationFailed(
                            "Git LFS is required for this submission"
                        )
                run_logged(
                    ["git", "-C", str(checkout), "lfs", "pull", "origin"],
                    output,
                    env={**git_env, "GIT_LFS_SKIP_SMUDGE": "0"},
                    timeout=600,
                )
            resolved_size, unresolved = resolved_tree_size(checkout)
            if unresolved:
                raise ValidationFailed("Git LFS pointers did not resolve")
            if resolved_size > REPO_MAX_BYTES or estimated_size > REPO_MAX_BYTES:
                raise ValidationFailed("resolved checkout exceeds 250 MiB")
            entry = (checkout / agent_path).resolve()
            if (
                checkout.resolve() not in entry.parents
                or not entry.is_file()
                or entry.is_symlink()
            ):
                raise ValidationFailed(
                    f"agent entrypoint is not a regular file: {agent_path}"
                )

            copy_submission(checkout, staging / "submission")
            shutil.copyfile(
                ROOT / "agent_image" / "runner.py", staging / "agent_runner.py"
            )
            dependency_lock = prepare_dependencies(checkout, staging, output)
            metadata = {
                "artifact_schema": 1,
                "ruleset_version": "ppo-plus-v2",
                "schema_version": "exposure-agent-v1",
                "repo_url": repo_url,
                "commit_sha": commit,
                "agent_path": agent_path,
            }
            (staging / "metadata.json").write_text(
                json.dumps(metadata, sort_keys=True, separators=(",", ":")) + "\n"
            )

            temporary_archive = (
                ARTIFACT_DIR / f".validation-{os.getpid()}-{job['id']}.tar.gz"
            )
            normalized_archive(staging, temporary_archive)
            artifact_sha = sha256_file(temporary_archive)
            artifact_size = temporary_archive.stat().st_size
            if artifact_size > ARTIFACT_MAX_BYTES:
                raise ValidationFailed("artifact exceeds 2 GiB")
            artifact = ARTIFACT_DIR / f"{artifact_sha}.tar.gz"

            context = temporary / "docker-context"
            shutil.copytree(staging, context)
            shutil.copyfile(ROOT / "agent_image" / "Dockerfile", context / "Dockerfile")
            tag = f"exposure-monopoly-agent:{artifact_sha}"
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
            image_exists = (
                inspected.returncode == 0 and inspected.stdout.strip() == runtime
            )
            if not image_exists:
                run_logged(
                    [
                        "docker",
                        "build",
                        "--pull=false",
                        "--network=none",
                        "--tag",
                        tag,
                        "--label",
                        f"exposure.artifact={artifact_sha}",
                        "--label",
                        f"exposure.runtime={runtime}",
                        str(context),
                    ],
                    output,
                    timeout=1200,
                )
                built_image = tag
            process = docker_agent(artifact_sha, agent_path)
            try:
                env = MonopolyEnv(max_rounds=1)
                player_id = env.whose_turn()
                allowed = [int(action) for action in env.get_allowed_actions(player_id)]
                state = {
                    "schema_version": "exposure-agent-v1",
                    "ruleset_version": "ppo-plus-v2",
                    "vector": build_state_vector(
                        env.players, env.properties, agent_id=player_id, env=env
                    ).tolist(),
                    "board": build_snapshot(env),
                    "actions": {
                        str(action): action_to_description(action) for action in allowed
                    },
                    "decision_seed": 0,
                }
                process.choose_action(state, player_id, allowed)
            finally:
                process.close()
            if artifact.exists():
                if (
                    artifact.stat().st_size != artifact_size
                    or sha256_file(artifact) != artifact_sha
                ):
                    raise ValidationFailed(
                        "artifact store contains a corrupt hash-addressed archive"
                    )
                temporary_archive.unlink()
            else:
                os.replace(temporary_archive, artifact)
            temporary_archive = None
            keep_image = True
        return {
            "commit_sha": commit,
            "repo_size_bytes": resolved_size,
            "artifact_sha256": artifact_sha,
            "artifact_size_bytes": artifact_size,
            "dependency_lock": dependency_lock,
            "validation_log": log_tail(log_path),
        }
    finally:
        if temporary_archive is not None:
            temporary_archive.unlink(missing_ok=True)
        if built_image is not None and not keep_image:
            subprocess.run(
                ["docker", "image", "rm", "--force", built_image],
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
                check=False,
            )
        log_path.unlink(missing_ok=True)


def validation_once() -> bool:
    job = (
        api(
            "/api/worker/rl-monopoly/submissions/claim",
            {"worker_id": VALIDATION_WORKER_ID},
        )
        or {}
    )
    if not job.get("id"):
        return False
    base = {
        "worker_id": VALIDATION_WORKER_ID,
        "id": job["id"],
        "generation": job["generation"],
        "lease_id": job["lease_id"],
    }
    log(f"validating submission {job['id']} generation {job['generation']}")
    try:
        with ValidationLease(job) as lease:
            result = validate_submission(job)
            lease.check()
        api(
            "/api/worker/rl-monopoly/submissions/result",
            {**base, "status": "approved", **result},
        )
        log(f"approved submission {job['id']} at {result['commit_sha'][:12]}")
    except Exception as error:
        message = f"{type(error).__name__}: {error}"[-12_000:]
        try:
            api(
                "/api/worker/rl-monopoly/submissions/result",
                {
                    **base,
                    "status": "failed",
                    "commit_sha": None,
                    "repo_size_bytes": None,
                    "artifact_sha256": None,
                    "artifact_size_bytes": None,
                    "dependency_lock": None,
                    "validation_log": message,
                },
            )
        except Exception as report_error:
            log(f"stale/unreportable validation failure: {report_error}")
        log(f"validation failed: {message}")
    return True


def colab(
    command: list[str],
    *,
    capture: bool = False,
    check: bool = True,
    timeout: float = 300,
):
    full = ["colab", *command]
    log("+ " + " ".join(full))
    return tracked_run(
        full,
        cwd=ROOT,
        stdout=subprocess.PIPE if capture else None,
        stderr=subprocess.PIPE if capture else None,
        text=True,
        check=check,
        timeout=timeout,
    )


def active_named_sessions() -> list[str]:
    result = colab(["sessions"], capture=True, check=False, timeout=30)
    text = (result.stdout or "") + (result.stderr or "")
    if result.returncode:
        raise RuntimeError(f"could not verify named Colab sessions: {text[-1000:]}")
    return [name for name in COLAB_SESSIONS if f"[{name}]" in text]


def stop_session(name: str) -> None:
    try:
        colab(["stop", "-s", name], check=False, timeout=30)
    except Exception as error:
        log(f"could not stop {name} on this attempt: {error}")


def terminate_process(process: subprocess.Popen, timeout: float = 10) -> None:
    if process.poll() is not None:
        return
    try:
        process.terminate()
        try:
            process.wait(timeout=timeout)
            return
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=timeout)
    except (OSError, subprocess.TimeoutExpired):
        # Remote named-session cleanup still runs even if a local CLI process is
        # momentarily unkillable.
        pass


def stop_all_named_sessions() -> list[str]:
    for name in COLAB_SESSIONS:
        stop_session(name)
    remaining = active_named_sessions()
    for name in remaining:
        stop_session(name)
    return active_named_sessions()


def parse_preflight(text: str) -> dict:
    marker = "EXPOSURE_PREFLIGHT="
    for line in reversed(text.splitlines()):
        if marker in line:
            return json.loads(line.split(marker, 1)[1])
    raise RuntimeError("Colab preflight did not report hardware")


def allocate_qualified(name: str) -> dict | None:
    try:
        result = colab(["new", "-s", name], capture=True, check=False, timeout=300)
    except Exception as error:
        reason = f"Colab allocation failed: {error}"[-1000:]
        log(f"{name} allocation rejected: {reason}")
        report_fleet(name, "error", reason=reason)
        stop_session(name)
        return None
    if result.returncode:
        reason = ((result.stdout or "") + (result.stderr or "")).strip()[
            -1000:
        ] or "Colab allocation failed"
        log(f"{name} allocation rejected: {reason}")
        report_fleet(name, "rejected", reason=reason)
        stop_session(name)
        return None
    try:
        probe = colab(
            ["exec", "-s", name, "-f", str(ROOT / "colab_preflight.py")],
            capture=True,
            timeout=180,
        )
        hardware = parse_preflight((probe.stdout or "") + (probe.stderr or ""))
        ram = int(hardware["ram_bytes"])
        cpus = float(hardware["effective_vcpus"])
        shape = str(hardware["machine_shape"])
        if ram < 32 * 1024**3 or cpus < 8:
            reason = f"requires 32 GiB/8 vCPU; allocated {ram / 1024**3:.1f} GiB/{cpus:.1f} vCPU"
            report_fleet(
                name, "rejected", ram=ram, cpus=cpus, shape=shape, reason=reason
            )
            stop_session(name)
            return None
        report_fleet(name, "ready", ram=ram, cpus=cpus, shape=shape)
        return hardware
    except Exception as error:
        report_fleet(name, "error", reason=f"preflight failed: {error}")
        stop_session(name)
        return None


def build_worker_bundle(destination: Path) -> None:
    paths = [
        ROOT / "rl_monopoly_runner.py",
        ROOT / "shared_game.py",
        ROOT / "agent_image" / "runner.py",
        ROOT / "monopoly_game_engine" / "__init__.py",
        ROOT / "monopoly_game_engine" / "actions.py",
        ROOT / "monopoly_game_engine" / "constants.py",
        ROOT / "monopoly_game_engine" / "env.py",
        ROOT / "monopoly_game_engine" / "state.py",
        ROOT / "monopoly_game_engine" / "networks.py",
        ROOT / "monopoly_game_engine" / "agents_fixed.py",
    ]
    with tempfile.TemporaryDirectory(
        prefix="monopoly-worker-bundle-"
    ) as temporary_name:
        staging = Path(temporary_name)
        for source in paths:
            relative = source.relative_to(ROOT)
            target = staging / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source, target)
        normalized_archive(staging, destination)


def write_remote_config(config: Path, name: str, *, selftest: bool = False) -> None:
    config.write_text(
        json.dumps(
            {
                "site": SITE,
                "worker_token": WORKER_TOKEN,
                "session_name": name,
                "selftest": selftest,
            }
        )
    )
    config.chmod(0o600)


def launch_colab_worker(name: str, bundle: Path, config: Path) -> subprocess.Popen:
    write_remote_config(config, name)
    colab(["upload", "-s", name, str(bundle), REMOTE_BUNDLE], timeout=300)
    colab(["upload", "-s", name, str(config), REMOTE_CONFIG], timeout=120)
    command = [
        "colab",
        "exec",
        "-s",
        name,
        "-f",
        str(ROOT / "colab_worker_bootstrap.py"),
        "--timeout",
        "86400",
    ]
    return subprocess.Popen(command, cwd=ROOT)


class Fleet:
    def __init__(self, max_colab: int, host_workers: int = 1):
        self.max_colab = max_colab
        self.host_workers = host_workers
        self.processes: dict[str, subprocess.Popen] = {}
        self.hosts: dict[str, subprocess.Popen] = {}
        self.allocated: set[str] = set()
        self.attempted = False
        self.closed = False
        self.temporary = tempfile.TemporaryDirectory(prefix="monopoly-fleet-")
        temp = Path(self.temporary.name)
        self.bundle = temp / "worker.tar.gz"
        self.config = temp / "config.json"
        build_worker_bundle(self.bundle)

    def start(self) -> None:
        if self.attempted:
            return
        self.attempted = True
        if self.max_colab and shutil.which("colab") is not None:
            leftovers = stop_all_named_sessions()
            if leftovers:
                raise RuntimeError(
                    f"could not clear stale named Colab sessions: {leftovers}"
                )
            for name in COLAB_SESSIONS[: self.max_colab]:
                hardware = allocate_qualified(
                    name
                )  # exactly one allocation attempt per slot
                if hardware is None:
                    continue
                self.allocated.add(name)
                self.processes[name] = launch_colab_worker(
                    name, self.bundle, self.config
                )
        ram, cpus = system_ram_bytes(), effective_vcpus()
        if ram >= MIN_HOST_RAM_BYTES and cpus >= MIN_HOST_VCPUS:
            for index in range(1, self.host_workers + 1):
                name = f"academy-host-{index}"
                if name in self.hosts:
                    continue
                environment = os.environ.copy()
                environment.update(
                    {
                        "MONOPOLY_SITE": SITE,
                        "WORKER_TOKEN": WORKER_TOKEN,
                        "MONOPOLY_WORKER_ID": name,
                        "MONOPOLY_WORKER_KIND": "host",
                        "MONOPOLY_SESSION_NAME": name,
                        "MONOPOLY_AGENT_BACKEND": "docker",
                    }
                )
                self.hosts[name] = subprocess.Popen(
                    [sys.executable, str(ROOT / "rl_monopoly_runner.py")],
                    env=environment,
                )
        else:
            reason = (
                f"requires {MIN_HOST_RAM_BYTES / 1024**3:.1f} GiB/"
                f"{MIN_HOST_VCPUS:.1f} vCPU; found {ram / 1024**3:.1f} GiB/{cpus:.1f} vCPU"
            )
            report_host("academy-host-1", "rejected", ram, cpus, reason)
            log(f"Academy host excluded: {ram / 1024**3:.1f} GiB/{cpus:.1f} vCPU")
        if not self.processes and not self.hosts:
            log(
                "no qualified game worker; games remain queued and preflight reasons are recorded"
            )

    def reap(self) -> None:
        lost_last_worker = False
        for name, process in list(self.processes.items()):
            if process.poll() is not None:
                log(f"Colab worker {name} exited with {process.returncode}")
                stop_session(name)
                report_fleet(
                    name,
                    "error",
                    reason=f"worker process exited with {process.returncode}",
                )
                del self.processes[name]
                self.allocated.discard(name)
                lost_last_worker = True
        for name, process in list(self.hosts.items()):
            if process.poll() is not None:
                log(f"host worker {name} exited with {process.returncode}")
                report_host(
                    name,
                    "error",
                    system_ram_bytes(),
                    effective_vcpus(),
                    f"worker process exited with {process.returncode}",
                )
                del self.hosts[name]
                lost_last_worker = True
        if lost_last_worker and not self.processes and not self.hosts:
            # Re-arm only after an actually running worker dies. An initial
            # hardware rejection remains one allocation attempt per slot.
            self.attempted = False

    def stop_workers(self) -> None:
        if not self.attempted and not self.processes and not self.hosts:
            return
        for process in self.processes.values():
            terminate_process(process)
        for process in self.hosts.values():
            terminate_process(process)
        leftovers: list[str] = []
        if shutil.which("colab") is not None:
            leftovers = stop_all_named_sessions()
        for name in self.allocated:
            report_fleet(name, "stopped")
        self.processes.clear()
        self.hosts.clear()
        self.allocated.clear()
        self.attempted = False
        if leftovers:
            raise RuntimeError(
                f"named Colab sessions remain after cleanup: {leftovers}"
            )

    def close(self) -> None:
        if self.closed:
            return
        try:
            self.stop_workers()
            if shutil.which("colab") is not None:
                leftovers = stop_all_named_sessions()
                if leftovers:
                    raise RuntimeError(
                        f"named Colab sessions remain after shutdown: {leftovers}"
                    )
        finally:
            self.temporary.cleanup()
            self.closed = True


def smoke_colab() -> None:
    name = COLAB_SESSIONS[0]
    temporary = tempfile.TemporaryDirectory(prefix="monopoly-colab-smoke-")
    qualified = False
    try:
        hardware = allocate_qualified(name)
        if hardware is None:
            raise SystemExit("smoke allocation did not satisfy 32 GiB/8 vCPU")
        qualified = True
        root = Path(temporary.name)
        bundle, config = root / "worker.tar.gz", root / "config.json"
        build_worker_bundle(bundle)
        write_remote_config(config, name, selftest=True)
        colab(["upload", "-s", name, str(bundle), REMOTE_BUNDLE], timeout=300)
        colab(["upload", "-s", name, str(config), REMOTE_CONFIG], timeout=120)
        result = colab(
            [
                "exec",
                "-s",
                name,
                "-f",
                str(ROOT / "colab_worker_bootstrap.py"),
                "--timeout",
                "600",
            ],
            capture=True,
            timeout=660,
        )
        if "selftest ok" not in ((result.stdout or "") + (result.stderr or "")):
            raise RuntimeError("remote worker selftest did not complete")
        log("one-session CPU Colab smoke passed")
    finally:
        stop_session(name)
        if qualified:
            report_fleet(name, "stopped")
        temporary.cleanup()
        leftovers = active_named_sessions()
        if leftovers:
            raise RuntimeError(f"smoke cleanup left named sessions: {leftovers}")


def smoke_fleet(max_colab: int) -> None:
    """Qualify up to five CPU sessions, run their small selftests concurrently, clean all."""
    temporary = tempfile.TemporaryDirectory(prefix="monopoly-colab-fleet-smoke-")
    processes: dict[str, subprocess.Popen] = {}
    qualified_names: list[str] = []
    try:
        root = Path(temporary.name)
        bundle = root / "worker.tar.gz"
        build_worker_bundle(bundle)
        qualified: list[tuple[str, Path]] = []
        for name in COLAB_SESSIONS[:max_colab]:
            if allocate_qualified(name) is None:
                continue
            qualified_names.append(name)
            config = root / f"{name}.json"
            write_remote_config(config, name, selftest=True)
            colab(["upload", "-s", name, str(bundle), REMOTE_BUNDLE], timeout=300)
            colab(["upload", "-s", name, str(config), REMOTE_CONFIG], timeout=120)
            qualified.append((name, config))
        if not qualified:
            raise SystemExit(
                "fleet smoke found no session with 32 GiB RAM and 8 effective vCPUs"
            )
        for name, _config in qualified:
            command = [
                "colab",
                "exec",
                "-s",
                name,
                "-f",
                str(ROOT / "colab_worker_bootstrap.py"),
                "--timeout",
                "600",
            ]
            processes[name] = subprocess.Popen(
                command,
                cwd=ROOT,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
            )
        for name, process in processes.items():
            stdout, stderr = process.communicate(timeout=660)
            if process.returncode or "selftest ok" not in stdout + stderr:
                raise RuntimeError(
                    f"fleet selftest failed for {name}: {(stdout + stderr)[-1000:]}"
                )
        log(f"parallel CPU Colab smoke passed on {len(processes)} qualified session(s)")
    finally:
        for process in processes.values():
            terminate_process(process)
        leftovers = stop_all_named_sessions()
        for name in qualified_names:
            report_fleet(name, "stopped")
        temporary.cleanup()
        if leftovers:
            raise RuntimeError(f"fleet smoke cleanup left named sessions: {leftovers}")


def dry_run(max_colab: int) -> None:
    with tempfile.TemporaryDirectory(prefix="monopoly-dry-run-") as temporary_name:
        bundle = Path(temporary_name) / "worker.tar.gz"
        build_worker_bundle(bundle)
        log(f"dry run: built {bundle.stat().st_size} byte runtime bundle")
    log("dry run: would claim validation jobs sequentially")
    for name in COLAB_SESSIONS[:max_colab]:
        log(
            f"dry run: colab new -s {name} (CPU; no GPU flag), verify 32 GiB/8 vCPU, then launch"
        )
    log("dry run: every named session would be stopped and verified absent in finally")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--once", action="store_true", help="one validation/fleet control tick"
    )
    parser.add_argument(
        "--dry-run", action="store_true", help="show commands without allocating"
    )
    parser.add_argument(
        "--smoke-colab",
        action="store_true",
        help="allocate and verify only ai-monopoly-1",
    )
    parser.add_argument(
        "--smoke-fleet",
        action="store_true",
        help="qualify and self-test up to five CPU sessions",
    )
    parser.add_argument("--max-colab", type=int, default=5, choices=range(0, 6))
    parser.add_argument(
        "--host-workers",
        type=int,
        default=1,
        help="concurrent rl_monopoly_runner.py processes to run on this host",
    )
    args = parser.parse_args()
    if args.dry_run:
        dry_run(args.max_colab)
        return
    if not WORKER_TOKEN:
        raise SystemExit("WORKER_TOKEN is required")
    if args.smoke_colab or args.smoke_fleet:

        def interrupt_smoke(_signum, _frame):
            raise KeyboardInterrupt

        signal.signal(signal.SIGTERM, interrupt_smoke)
        try:
            if args.smoke_colab:
                smoke_colab()
            else:
                smoke_fleet(args.max_colab)
        except KeyboardInterrupt as error:
            raise SystemExit(130) from error
        return
    if shutil.which("colab") is None and args.max_colab:
        raise SystemExit("Google Colab CLI is not installed")
    fleet = Fleet(args.max_colab, args.host_workers)
    stopping = False

    def request_stop(_signum=None, _frame=None):
        nonlocal stopping
        if stopping:
            return
        stopping = True
        interrupt_children()
        raise ControllerStopping

    signal.signal(signal.SIGINT, request_stop)
    signal.signal(signal.SIGTERM, request_stop)
    try:
        while not stopping:
            worked = (
                validation_once()
            )  # validation/build work is intentionally sequential
            demand = api("/api/worker/rl-monopoly/demand") or {}
            if demand.get("games"):
                fleet.start()
            else:
                fleet.stop_workers()
            fleet.reap()
            if args.once:
                break
            if not worked:
                time.sleep(10)
    except ControllerStopping:
        pass
    finally:
        fleet.close()


if __name__ == "__main__":
    main()
