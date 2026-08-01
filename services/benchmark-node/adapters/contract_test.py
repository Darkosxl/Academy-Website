#!/usr/bin/env python3
"""Small no-framework check for the Python-to-Rust NDJSON boundary."""
from __future__ import annotations

import contextlib
import io
import json
import os
import time

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
        reporter.update("arc", status="running", done=1, total=13)
        reporter.frames([{
            "game": "ls20", "seq": 0, "grids": "0" * 4096,
            "state": "NOT_PLAYED", "levels_completed": 0, "baseline": None,
            "action": None, "action_x": None, "action_y": None,
        }])
    events = lines(output)
    assert events[0] == {
        "type": "progress",
        "benchmark": "arc",
        "state": {"status": "running", "done": 1, "total": 13},
    }
    assert events[1]["type"] == "frames"
    assert len(events[1]["frames"][0]["grids"]) == 4096
    assert runner.terminal_status({
        "arc": {"status": "done"},
        "frontier": {"status": "failed"},
        "ram": {"status": "failed"},
    }) == "partial"
    print("python adapter contract ok")


if __name__ == "__main__":
    main()
