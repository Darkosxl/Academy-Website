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
    SITE,
    WORKER_TOKEN,
    cleanup_run_containers,
    ensure_arc_host,
    ensure_image,
    parse_last_json,
    podman_base,
    run_checked,
)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--game", choices=ARC_GAMES, default="ls20")
    parser.add_argument("--seconds", type=int, default=45)
    parser.add_argument(
        "--repo",
        type=Path,
        default=ROOT / "live_submission",
        help="local submission directory (default: adapters/live_submission)",
    )
    # The frame feed keys on a real harness_runs row, so the default local id deliberately
    # is not a uuid and the feed stays off. Pass a queued run's uuid to watch this game
    # land in the viewer — the only way to exercise frame capture without a full cohort.
    parser.add_argument("--run-id", dest="run_id", default=None,
                        help="uuid of an existing non-terminal harness run; enables frame posting")
    args = parser.parse_args()
    if not 10 <= args.seconds <= 180:
        parser.error("--seconds must be between 10 and 180")
    if args.run_id:
        try:
            uuid.UUID(args.run_id)
        except ValueError:
            parser.error("--run-id must be a uuid naming an existing harness run")

    ensure_arc_host(require_worker_token=bool(args.run_id))
    ensure_image()
    run_id = args.run_id or f"live-{uuid.uuid4().hex[:12]}"
    submission = args.repo.resolve()
    if not (submission / "agent" / "my_agent.py").is_file():
        parser.error(f"{submission} must contain agent/my_agent.py")

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
                    # Both live in .env, not the shell, so arc_game.py cannot read them
                    # itself. Without these the frame feed silently disables — which is
                    # correct for a standalone smoke test, but defeats --run-id.
                    "HARNESS_SITE": SITE,
                    "WORKER_TOKEN": WORKER_TOKEN,
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
