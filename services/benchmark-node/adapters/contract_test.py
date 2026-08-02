#!/usr/bin/env python3
"""Small no-framework check for the Python-to-Rust NDJSON boundary."""
from __future__ import annotations

import contextlib
import io
import json
import os
import tempfile
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
    frontier_reporter = runner.NdjsonReporter(
        "018f0f65-9abc-7def-8123-456789abcdef", time.monotonic() + 30, "frontier",
    )
    assert frontier_reporter.state["arc"] == {"status": "skipped"}
    frontier_reporter.state["frontier"] = {"status": "done"}
    frontier_reporter.state["ram"] = {"status": "done"}
    assert runner.terminal_status(frontier_reporter.snapshot()) == "done"

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
    assert runner.FRONTIER_DEADLINE_SECONDS == 15 * 60
    assert runner.frontier_cutoff(2000, now=100) == 1000
    assert runner.frontier_cutoff(500, now=100) == 500

    with tempfile.TemporaryDirectory() as raw:
        stub = Path(raw) / "stub.conf"
        upstream = Path(raw) / "upstream.conf"
        stub.write_text("nameserver 127.0.0.53\n")
        upstream.write_text("nameserver 192.0.2.53\nnameserver 2001:db8::53\n")
        assert runner.buildkit_nameservers((stub, upstream)) == [
            "192.0.2.53", "2001:db8::53",
        ]
        stub.write_text("nameserver 127.0.0.11\n# ExtServers: [192.0.2.53 2001:db8::53]\n")
        assert runner.buildkit_nameservers((stub,)) == ["192.0.2.53", "2001:db8::53"]

    with tempfile.TemporaryDirectory(dir="/tmp") as raw:
        work = Path(raw) / "work"
        repo = work / "repo"
        dataset = work / "frontier-sprint"
        jobs = work / "frontier-jobs"
        gateway = work / "frontier-gateway"
        socket_dir = Path(raw) / "socket"
        docker_config = socket_dir / "docker"
        for path in (repo, dataset, jobs, gateway, docker_config):
            path.mkdir(parents=True, exist_ok=True)
        socket_path = socket_dir / "podman.sock"
        command = runner.bubblewrap_harbor(
            repo, dataset, jobs, SimpleNamespace(directory=gateway, token="test-token"),
            socket_path, docker_config, "test-builder",
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
        assert "--unshare-net" in command
        shell = command[-1]
        assert str(runner.HARBOR_ROOT / "bin/python") in shell
        assert str(runner.HARBOR_CLI) in shell
        assert str(Path.home() / ".local/share/uv/python") not in command

    cleanup_calls = []
    original_subprocess_run = runner.subprocess.run

    def failed_cleanup(command, **_kwargs):
        cleanup_calls.append(command)
        return SimpleNamespace(returncode=1)

    try:
        runner.subprocess.run = failed_cleanup
        runner.cleanup_buildx_builder("test-builder", {"DOCKER_HOST": "test"})
    finally:
        runner.subprocess.run = original_subprocess_run
    assert cleanup_calls == [
        ["docker", "buildx", "rm", "--force", "test-builder"],
        ["podman", "rm", "-f", "buildx_buildkit_test-builder0"],
        ["podman", "volume", "rm", "-f", "buildx_buildkit_test-builder0_state"],
    ]

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
        assert "timeout_sec = 120.0" in rewritten
        assert "timeout_sec = 60.0" in rewritten
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
    print("python adapter contract ok")


if __name__ == "__main__":
    main()
