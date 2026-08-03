#!/usr/bin/env python3
"""Small no-framework check for the Python-to-Rust NDJSON boundary."""
from __future__ import annotations

import contextlib
import io
import json
import os
import tempfile
import subprocess
import threading
import time
from types import SimpleNamespace
from pathlib import Path

import bedrock_gateway
import runner


def lines(output: io.StringIO) -> list[dict]:
    return [json.loads(line) for line in output.getvalue().splitlines()]


def main() -> None:
    assert runner.deployment_environment({}) == "PROD"
    assert runner.deployment_environment({"ENVIRONMENT": "DEV"}) == "DEV"
    assert runner.deployment_environment({"ENVIRONMENT": "prod"}) == "PROD"
    assert runner.deployment_environment({"HARNESS_ENV": "local"}) == "DEV"
    assert runner.deployment_environment({"HARNESS_ENV": "production"}) == "PROD"
    previous_harness_env = os.environ.get("HARNESS_ENV")
    try:
        os.environ["HARNESS_ENV"] = "executor"
        assert runner.env_file() == {}
    finally:
        if previous_harness_env is None:
            os.environ.pop("HARNESS_ENV", None)
        else:
            os.environ["HARNESS_ENV"] = previous_harness_env
    output = io.StringIO()
    runner.NDJSON_MODE = True
    reporter = runner.NdjsonReporter("018f0f65-9abc-7def-8123-456789abcdef", time.monotonic() + 30)
    with contextlib.redirect_stdout(output):
        reporter.update("arc", status="running", done=1, total=25)
        reporter.frames([{
            "game": "ls20", "seq": 0, "grids": "0" * 4096,
            "state": "NOT_PLAYED", "levels_completed": 0, "baseline": None,
            "action": None, "action_x": None, "action_y": None,
        }])
    events = lines(output)
    assert events[0] == {
        "type": "progress",
        "benchmark": "arc",
        "state": {"status": "running", "done": 1, "total": 25},
    }
    assert events[1]["type"] == "frames"
    assert len(events[1]["frames"][0]["grids"]) == 4096
    terminal_reporter = runner.NdjsonReporter(
        "018f0f65-9abc-7def-8123-456789abcdef", time.monotonic() + 30, "frontier",
    )
    assert terminal_reporter.state["arc"] == {"status": "skipped"}
    terminal_reporter.state["frontier"] = {"status": "done"}
    terminal_reporter.state["ram"] = {"status": "done"}
    assert runner.terminal_status(terminal_reporter.snapshot()) == "done"

    with tempfile.TemporaryDirectory() as raw:
        previous_ram_lock = runner.CONFIG.get("HARNESS_RAM_LOCK")
        runner.CONFIG["HARNESS_RAM_LOCK"] = str(Path(raw) / "ram.lock")
        active = 0
        maximum_active = 0
        guard = threading.Lock()
        start = threading.Barrier(2)

        def use_ram_lane() -> None:
            nonlocal active, maximum_active
            start.wait()
            with runner.ram_lane(time.monotonic() + 5):
                with guard:
                    active += 1
                    maximum_active = max(maximum_active, active)
                time.sleep(0.05)
                with guard:
                    active -= 1

        threads = [threading.Thread(target=use_ram_lane) for _ in range(2)]
        for thread in threads:
            thread.start()
        for thread in threads:
            thread.join(timeout=2)
            assert not thread.is_alive()
        assert maximum_active == 1
        if previous_ram_lock is None:
            runner.CONFIG.pop("HARNESS_RAM_LOCK", None)
        else:
            runner.CONFIG["HARNESS_RAM_LOCK"] = previous_ram_lock
    assert len(runner.ARC_GAMES) == 25
    assert len(set(runner.ARC_GAMES)) == 25
    assert runner.ARC_CONCURRENCY == 5
    assert runner.TERMINAL_CONCURRENCY == 2
    assert runner.TERMINAL_MAX_TURNS == 180
    assert runner.TERMINAL_DEADLINE_SECONDS == 60 * 60
    assert runner.terminal_cutoff(9000, now=100) == 100 + 60 * 60
    assert runner.terminal_cutoff(500, now=100) == 500

    # buildkit_nameservers went away with b251b09 (classic builds for Terminal-Bench);
    # its assertions went with it.

    with tempfile.TemporaryDirectory(dir="/tmp") as raw:
        work = Path(raw) / "work"
        repo = work / "repo"
        dataset = work / "terminal-sprint"
        jobs = work / "terminal-jobs"
        gateway = work / "terminal-gateway"
        socket_dir = Path(raw) / "socket"
        docker_config = socket_dir / "docker"
        for path in (repo, dataset, jobs, gateway, docker_config):
            path.mkdir(parents=True, exist_ok=True)
        socket_path = socket_dir / "podman.sock"
        command = runner.bubblewrap_harbor(
            repo, dataset, jobs, SimpleNamespace(directory=gateway, token="test-token"),
            socket_path, docker_config,
        )

        def has_sequence(sequence):
            return any(
                command[index:index + len(sequence)] == sequence
                for index in range(len(command) - len(sequence) + 1)
            )

        assert has_sequence(["--ro-bind", str(repo), str(repo)])
        assert has_sequence(["--ro-bind", str(dataset), str(dataset)])
        assert has_sequence(["--bind", str(jobs), str(jobs)])
        assert not has_sequence(["--bind", str(work), str(work)])
        assert has_sequence(["--setenv", "DOCKER_CONFIG", str(docker_config)])
        assert "--unshare-net" not in command
        shell = command[-1]
        assert str(runner.HARBOR_ROOT / "bin/python") in shell
        assert str(runner.HARBOR_CLI) in shell
        assert f"-n {runner.TERMINAL_CONCURRENCY}" in shell
        assert f"max_turns={runner.TERMINAL_MAX_TURNS}" in shell
        assert json.dumps(
            {"response_format": runner.TERMINUS_RESPONSE_FORMAT}, separators=(",", ":")
        ) in shell
        assert str(Path.home() / ".local/share/uv/python") not in command

    # cleanup_buildx_builder went away with b251b09 too, so the shim is the only thing
    # standing between Harbor and a buildkit container Podman cannot create.
    with tempfile.TemporaryDirectory() as raw:
        docker_config = Path(raw) / "docker"
        docker_config.mkdir()
        runner.install_harbor_docker_shim(docker_config)
        shim = docker_config.parent / "bin" / "docker"
        fake_docker = docker_config.parent / "echo-args"
        fake_docker.write_text('#!/usr/bin/env bash\necho "$@"\n')
        fake_docker.chmod(0o700)
        shim.write_text(shim.read_text().replace("/usr/bin/docker", str(fake_docker)))

        def shim_run(*argv):
            return subprocess.run([str(shim), *argv], capture_output=True, text=True)

        # bootstrapping a builder is exactly what must never reach the real CLI
        for subcommand in ("create", "inspect", "use", "rm"):
            done = shim_run("buildx", subcommand, "--bootstrap", "somebuilder")
            assert done.returncode == 0 and done.stdout == "", (subcommand, done)
        built = shim_run(
            "buildx", "build", "--builder", "somebuilder", "--platform=linux/amd64",
            "--load", "--output=type=docker,name=task:latest", "-f", "Dockerfile", ".",
        )
        assert built.returncode == 0, built
        assert built.stdout.split() == ["build", "-f", "Dockerfile", "--tag", "task:latest", "."], built
        passed = shim_run("compose", "up")
        assert passed.stdout.strip() == "compose up", passed

    with tempfile.TemporaryDirectory() as raw:
        jobs = Path(raw) / "jobs"
        (jobs / "job" / "Task.Name__ABC1234").mkdir(parents=True)
        projects, verifier_prefixes = runner.harbor_compose_scopes(jobs)
        assert projects == {"task-name__abc1234__env"}
        assert verifier_prefixes == {"task-name__abc1234__verifier__"}
        task = Path(raw) / "task.toml"
        task.write_text(
            "[agent]\ntimeout_sec = 7200\nnetwork_mode = \"public\"\n"
            "[verifier]\ntimeout_sec = 300\n"
            "[environment]\nbuild_timeout_sec = 600\n"
        )
        runner.rewrite_timeouts(task)
        rewritten = task.read_text()
        assert f"timeout_sec = {runner.TERMINAL_AGENT_SECONDS}" in rewritten
        assert "timeout_sec = 300" in rewritten
        # three waves of agent time must still fit inside the stage budget
        assert 3 * runner.TERMINAL_AGENT_SECONDS < runner.TERMINAL_DEADLINE_SECONDS
        assert 'network_mode = "public"' in rewritten
        assert 'network_mode = "no-network"' not in rewritten

    class FakeProcess:
        live = 0
        maximum_live = 0
        games: list[str] = []

        def __init__(self, command, **_kwargs):
            game = command[command.index("--game") + 1]
            self.game = game
            self.stdout = io.StringIO(json.dumps({
                "game": game, "status": "done", "score": 1.0,
            }) + "\n")
            self.stderr = io.StringIO("")
            self.returncode = None
            FakeProcess.games.append(game)
            FakeProcess.live += 1
            FakeProcess.maximum_live = max(FakeProcess.maximum_live, FakeProcess.live)

        def poll(self):
            if self.returncode is None:
                self.returncode = 0
                FakeProcess.live -= 1
            return self.returncode

        def terminate(self):
            self.poll()

        def kill(self):
            self.poll()

        def wait(self, timeout=None):
            del timeout
            return self.poll()

    class FakeReporter:
        def __init__(self):
            self.lease = SimpleNamespace(token="lease-token")
            self.updates: list[tuple[str, dict]] = []

        def update(self, benchmark, **state):
            self.updates.append((benchmark, state))

    class FakeGateway:
        token = "gateway-token"
        directory = Path("/tmp")

        @staticmethod
        def metrics():
            return {"completed_last_30s": 0}

    original_popen = runner.subprocess.Popen
    original_cleanup = runner.cleanup_run_containers
    try:
        runner.subprocess.Popen = FakeProcess
        runner.cleanup_run_containers = lambda _run_id: None
        fake_reporter = FakeReporter()
        score = runner.run_arc(
            "018f0f65-9abc-7def-8123-456789abcdef",
            Path("/submission"), Path("/venv"), FakeGateway(), fake_reporter,
            time.monotonic() + 30,
        )
    finally:
        runner.subprocess.Popen = original_popen
        runner.cleanup_run_containers = original_cleanup
    assert FakeProcess.games == list(runner.ARC_GAMES)
    assert FakeProcess.maximum_live == runner.ARC_CONCURRENCY
    assert score == 1.0
    assert fake_reporter.updates[-1][1]["done"] == 25
    assert runner.terminal_status({
        "arc": {"status": "done"},
        "frontier": {"status": "failed"},
        "ram": {"status": "failed"},
    }) == "partial"
    for source in ("builtin://forge", "builtin://reki"):
        path = runner.builtin_submission(source)
        assert path is not None and path.is_dir()
        assert all((path / required).is_file() for required in runner.REQUIRED_FILES)
        assert len(runner.submission_digest(path)) == 40
    assert runner.builtin_submission("builtin://unknown") is None
    with tempfile.TemporaryDirectory() as raw:
        repo = Path(raw)
        (repo / "main.py").write_text("pass\n")
        digest = runner.submission_digest(repo)
        cache = repo / "__pycache__"
        cache.mkdir()
        (cache / "main.cpython-312.pyc").write_bytes(b"generated")
        assert runner.submission_digest(repo) == digest
    multimodal = {
        "messages": [{
            "role": "user",
            "content": [
                {"type": "image_url", "image_url": {"url": "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII="}},
                {"type": "text", "text": "choose an action"},
            ],
        }],
    }
    _, bedrock_messages, _ = bedrock_gateway.openai_to_bedrock(multimodal)
    assert bedrock_messages[0]["content"][0]["image"]["source"]["bytes"].startswith(
        b"\x89PNG\r\n\x1a\n"
    )
    _, inputs, _, _ = bedrock_gateway.openai_to_responses(multimodal)
    assert inputs[0]["content"][0]["type"] == "input_image"
    assert bedrock_gateway.model_supports_reasoning_effort("google.gemma-4-31b")
    assert not bedrock_gateway.model_supports_reasoning_effort(
        "mistral.mistral-large-3-675b-instruct"
    )
    assert not bedrock_gateway.model_uses_native_runtime("xai.grok-4.3")
    assert not bedrock_gateway.model_uses_native_runtime("google.gemma-4-31b")
    assert bedrock_gateway.model_uses_native_runtime("anthropic.claude-opus-4-6-v1")
    assert bedrock_gateway.model_uses_native_runtime("us.anthropic.claude-opus-4-6-v1")
    assert bedrock_gateway.model_uses_native_runtime(
        "global.anthropic.claude-opus-4-6-v1"
    )
    assert bedrock_gateway.model_uses_native_runtime(
        "arn:aws:bedrock:us-east-1:123456789012:application-inference-profile/example"
    )

    forwarded: dict[str, object] = {}

    class FakeCompletions:
        def create(self, **kwargs):
            forwarded.update(kwargs)
            return SimpleNamespace(model_dump=lambda **_kwargs: {"usage": {}})

    body = json.dumps({
        "messages": [{"role": "user", "content": "Respond as JSON."}],
        "response_format": runner.TERMINUS_RESPONSE_FORMAT,
    }).encode()
    handler = SimpleNamespace(
        path="/v1/chat/completions",
        headers={"Content-Length": str(len(body))},
        rfile=io.BytesIO(body),
        server=SimpleNamespace(
            gateway_token="test-token",
            slots=threading.BoundedSemaphore(1),
            api_style="chat",
            model_id="gemma-4-31b",
            reasoning_effort="none",
            client=SimpleNamespace(chat=SimpleNamespace(completions=FakeCompletions())),
            stats=bedrock_gateway.Stats(),
            profile_name="gemma-4-31b",
        ),
        _authorized=lambda: True,
        _json=lambda *_args, **_kwargs: None,
    )
    bedrock_gateway.GatewayHandler.do_POST(handler)
    assert forwarded["response_format"] == runner.TERMINUS_RESPONSE_FORMAT

    # A single-lane run must not require the other lane's dataset.
    arc_only = runner.required_caches("arc")
    terminal_only = runner.required_caches("frontier")
    assert runner.TERMINAL_SOURCE not in arc_only and runner.HARBOR_CLI not in arc_only
    assert runner.ARC_PYTHON not in terminal_only
    assert set(runner.required_caches("bundled")) == set(arc_only) | set(terminal_only)

    # A progress post over 8000 bytes is rejected outright, and one shared failure mode
    # fails all 25 games with the same long error at once. That is the worst case.
    games = {
        game: {
            "game": game, "status": "failed", "score": 0.0,
            "error": ("x" * 4000)[-runner.ARC_GAME_ERROR_CHARS:],
        }
        for game in runner.ARC_GAMES
    }
    worst = json.dumps({
        "status": "running", "done": len(games), "total": len(games),
        "games": games, "active": 0, "queued": 0, "rate": 0,
    })
    assert len(worst) < 8000, len(worst)

    print("python adapter contract ok")


if __name__ == "__main__":
    main()
