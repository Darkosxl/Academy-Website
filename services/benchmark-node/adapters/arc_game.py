#!/usr/bin/env python3
"""Trusted controller for one pinned public ARC-AGI-3 game."""
from __future__ import annotations

import argparse
import json
import os
import selectors
import subprocess
import sys
import time
from pathlib import Path

import arc_agi
from arc_agi import OperationMode
from arcengine import FrameData, GameAction, GameState


def read_json(proc: subprocess.Popen, deadline: float) -> dict:
    selector = selectors.DefaultSelector()
    selector.register(proc.stdout, selectors.EVENT_READ)
    try:
        remaining = max(0.0, deadline - time.monotonic())
        if not selector.select(remaining):
            raise TimeoutError("agent response deadline exceeded")
        line = proc.stdout.readline()
    finally:
        selector.close()
    if not line:
        error = proc.stderr.read()[-4000:] if proc.stderr else ""
        raise RuntimeError(f"agent exited before replying: {error}")
    return json.loads(line)


def frame(raw) -> dict:
    return FrameData(
        game_id=raw.game_id,
        frame=[array.tolist() for array in raw.frame],
        state=raw.state,
        levels_completed=raw.levels_completed,
        win_levels=raw.win_levels,
        guid=raw.guid,
        full_reset=raw.full_reset,
        available_actions=raw.available_actions,
    ).model_dump(mode="json")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--game", required=True)
    parser.add_argument("--deadline-monotonic", type=float, required=True)
    parser.add_argument("--repo", type=Path, required=True)
    parser.add_argument("--venv", type=Path, required=True)
    parser.add_argument("--gateway-dir", type=Path, required=True)
    parser.add_argument("--image", required=True)
    parser.add_argument("--worker-dir", type=Path, required=True)
    parser.add_argument("--run-id", required=True)
    args = parser.parse_args()

    starter = Path(os.environ["HARNESS_ARC_STARTER"])
    arcade = arc_agi.Arcade(
        operation_mode=OperationMode.OFFLINE,
        environments_dir=str(starter / "environment_files"),
    )
    env = arcade.make(args.game)
    if env is None:
        raise SystemExit(f"could not create public game {args.game}")

    container = [
        "podman", "run", "--rm", "-i", "--network=none", "--cap-drop=all",
        "--label", f"academy.harness.run={args.run_id}",
        "--label", "academy.harness.benchmark=arc",
        "--security-opt=no-new-privileges", "--pids-limit=128", "--memory=2g",
        "--cpus=1", "--read-only", "--tmpfs=/tmp:rw,noexec,nosuid,size=128m",
        "--mount", f"type=bind,src={args.repo},dst=/submission,ro=true",
        "--mount", f"type=bind,src={args.venv},dst=/venv,ro=true",
        "--mount", f"type=bind,src={args.gateway_dir},dst=/run/harness,ro=true",
        "--mount", f"type=bind,src={args.worker_dir},dst=/opt/harness,ro=true",
        "-e", f"HARNESS_ARC_GAME={args.game}",
        "-e", "OPENAI_BASE_URL=http://127.0.0.1:8000/v1",
        "-e", f"OPENAI_API_KEY={os.environ['BEDROCK_GATEWAY_TOKEN']}",
        "-e", f"HARNESS_LLM_BASE=http://127.0.0.1:8000/v1",
        "-e", f"HARNESS_LLM_KEY={os.environ['BEDROCK_GATEWAY_TOKEN']}",
        "-e", f"HARNESS_LLM_MODEL={os.environ['BEDROCK_PROFILE_NAME']}",
        "-e", "HOME=/tmp/home",
        args.image,
        "sh", "-lc",
        "mkdir -p /tmp/home && "
        "socat TCP-LISTEN:8000,bind=127.0.0.1,reuseaddr,fork UNIX-CONNECT:/run/harness/bedrock.sock & "
        "exec /venv/bin/python /opt/harness/arc_agent_session.py",
    ]
    proc = subprocess.Popen(
        container,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        bufsize=1,
    )
    actions = 0
    latest = env.observation_space
    append = False
    try:
        ready = read_json(proc, min(args.deadline_monotonic, time.monotonic() + 30))
        if not ready.get("ready"):
            raise RuntimeError(ready.get("detail") or ready.get("error") or "agent did not initialize")
        while latest.state not in (GameState.WIN, GameState.GAME_OVER):
            if time.monotonic() >= args.deadline_monotonic:
                raise TimeoutError("global ARC deadline exceeded")
            proc.stdin.write(json.dumps({"frame": frame(latest), "append": append}) + "\n")
            proc.stdin.flush()
            response = read_json(proc, min(args.deadline_monotonic, time.monotonic() + 20))
            if response.get("error"):
                raise RuntimeError(response.get("detail") or response["error"])
            try:
                action = GameAction[response["action"]]
            except (KeyError, TypeError) as exc:
                raise RuntimeError("agent returned an invalid action") from exc
            if action.is_complex():
                action.set_data(response.get("data") or {})
            reasoning = response.get("reasoning")
            if reasoning is not None:
                action.reasoning = reasoning
            latest = env.step(action, data=action.action_data.model_dump(), reasoning=reasoning)
            actions += 1
            append = True
        scorecard = arcade.get_scorecard()
        print(json.dumps({
            "game": args.game,
            "status": "done",
            "state": latest.state.value,
            "levels_completed": latest.levels_completed,
            "actions": actions,
            "score": float(scorecard.score),
        }, separators=(",", ":")))
    except TimeoutError as exc:
        print(json.dumps({"game": args.game, "status": "timeout", "score": 0.0, "error": str(exc)}))
    except Exception as exc:
        print(json.dumps({"game": args.game, "status": "failed", "score": 0.0, "error": str(exc)}))
    finally:
        if proc.poll() is None:
            proc.terminate()
            try:
                proc.wait(timeout=2)
            except subprocess.TimeoutExpired:
                proc.kill()
        if proc.stderr:
            tail = proc.stderr.read()[-4000:]
            if tail:
                print(tail, file=sys.stderr)


if __name__ == "__main__":
    main()
