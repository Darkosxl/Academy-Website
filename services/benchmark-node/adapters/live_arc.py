#!/usr/bin/env python3
"""Run one unscored public ARC game through the production sandbox and gateway."""
from __future__ import annotations

import argparse
import json
import os
import subprocess
import tempfile
import time
import uuid
from pathlib import Path

from runner import (
    ARC_GAMES,
    ARC_PYTHON,
    ARC_STARTER,
    BEDROCK_PROFILE_NAME,
    Gateway,
    HARNESS_IMAGE,
    ROOT,
    cleanup_run_containers,
    ensure_host,
    ensure_image,
    parse_last_json,
    podman_base,
    run_checked,
)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--game", choices=ARC_GAMES, default="ls20")
    parser.add_argument("--seconds", type=int, default=45)
    args = parser.parse_args()
    if not 10 <= args.seconds <= 180:
        parser.error("--seconds must be between 10 and 180")

    ensure_host()
    ensure_image()
    run_id = f"live-{uuid.uuid4().hex[:12]}"
    submission = ROOT / "live_submission"

    with tempfile.TemporaryDirectory(prefix="harness-live-arc-") as raw_work:
        work = Path(raw_work)
        run_checked(
            podman_base(work, network="none")
            + ["python", "-m", "venv", "--system-site-packages", "/work/venv"],
            timeout=30,
        )
        gateway = Gateway("arc-live", work)
        try:
            deadline = time.monotonic() + args.seconds
            env = os.environ.copy()
            env.update(
                {
                    "HARNESS_ARC_STARTER": str(ARC_STARTER),
                    "BEDROCK_GATEWAY_TOKEN": gateway.token,
                    "BEDROCK_PROFILE_NAME": BEDROCK_PROFILE_NAME,
                }
            )
            result = subprocess.run(
                [
                    str(ARC_PYTHON),
                    str(ROOT / "arc_game.py"),
                    "--game",
                    args.game,
                    "--deadline-monotonic",
                    str(deadline),
                    "--repo",
                    str(submission),
                    "--venv",
                    str(work / "venv"),
                    "--gateway-dir",
                    str(gateway.directory),
                    "--image",
                    HARNESS_IMAGE,
                    "--worker-dir",
                    str(ROOT),
                    "--run-id",
                    run_id,
                ],
                env=env,
                capture_output=True,
                text=True,
                timeout=args.seconds + 15,
            )
            payload = parse_last_json(result.stdout)
            metrics = gateway.metrics()
            gateway_ok = (
                result.returncode == 0
                and payload.get("status") in {"done", "timeout"}
                and metrics["requests"] >= 1
                and metrics["errors"] == 0
            )
            rate_target = 10
            rate_eligible = metrics["uptime_seconds"] >= 30 and payload.get("status") == "timeout"
            rate_met = not rate_eligible or metrics["completed_last_30s"] >= rate_target
            ok = gateway_ok and rate_met
            print(
                json.dumps(
                    {
                        "ok": ok,
                        "diagnostic": payload,
                        "gateway": metrics,
                        "rate_eligible": rate_eligible,
                        "rate_target_per_30s": rate_target,
                        "rate_target_met": rate_met,
                        "stderr_tail": result.stderr[-2000:] if not ok else "",
                    },
                    indent=2,
                    sort_keys=True,
                )
            )
            if not ok:
                raise SystemExit(1)
        finally:
            cleanup_run_containers(run_id)
            gateway.stop()


if __name__ == "__main__":
    main()
