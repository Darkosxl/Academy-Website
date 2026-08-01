#!/usr/bin/env python3
"""Trusted controller for one pinned public ARC-AGI-3 game."""
from __future__ import annotations

import argparse
import json
import os
import queue
import selectors
import subprocess
import sys
import threading
import time
import urllib.error
import urllib.request
import uuid
from pathlib import Path

try:
    import arc_agi
    from arc_agi import OperationMode
    from arcengine import FrameData, GameAction, GameState
except ImportError:
    # --self-check exercises the pure-python encoder under the system interpreter,
    # which has no ARC venv. Every other invocation still fails loudly.
    if "--self-check" not in sys.argv:
        raise

HEX = "0123456789abcdef"
GRID_CHARS = 64 * 64
MAX_GRIDS = 16
BATCH_FRAMES = 32
BATCH_CHARS = 256 * 1024
STOP = {"grids": ""}  # queue sentinel, compared by identity, never posted
FRAME_PREFIX = "@@EXPOSURE_ARC_FRAMES@@"


def encode_grid(grid) -> str:
    """One 64x64 board as 4096 row-major lowercase hex nibbles; ARC cells are 0-15."""
    return "".join(HEX[cell & 15] for row in grid for cell in row)


def encode_grids(frames) -> str:
    """FrameData.frame is an intra-action animation buffer, not a history: keep the head
    and always the resulting board, so the viewer can play it back without storing 1000."""
    if len(frames) > MAX_GRIDS:
        frames = list(frames[:MAX_GRIDS - 1]) + list(frames[-1:])
    encoded = [encode_grid(grid) for grid in frames]
    return "\n".join(text for text in encoded if len(text) == GRID_CHARS)


class FrameFeed:
    """Emits boards to Rust over NDJSON, with legacy Academy posting for rollback."""

    def __init__(self, run_id: str, game: str, baseline: list | None):
        site = os.environ.get("HARNESS_SITE", "").strip().rstrip("/")
        token = os.environ.get("WORKER_TOKEN", "").strip()
        lease_token = os.environ.get("HARNESS_LEASE_TOKEN", "").strip()
        self.ndjson = os.environ.get("HARNESS_FRAME_SINK") == "ndjson"
        self.game = game
        self.baseline = baseline
        self.run_id = run_id
        self.url = site + "/api/worker/harness/arc/frames"
        self.token = token
        self.lease_token = lease_token
        self.seq = 0
        self.stopped = True
        self.thread = None
        self.queue: queue.Queue = queue.Queue(maxsize=256)
        try:
            uuid.UUID(run_id)
        except ValueError:
            return  # a synthetic run id has no row on the site to attach frames to
        if not self.ndjson and (not site or not token or not lease_token):
            return  # diagnostics have no active Academy lease and deliberately stay local
        self.stopped = False
        self.thread = threading.Thread(target=self._drain, name="arc-frames", daemon=True)
        self.thread.start()

    def publish(self, latest, action=None) -> None:
        seq = self.seq
        self.seq += 1
        if self.stopped:
            return
        try:
            grids = encode_grids(latest.frame)
            if not grids:
                return  # the engine stops rendering once a game is WIN or GAME_OVER
            data = getattr(action, "action_data", None)
            self.queue.put_nowait({
                "game": self.game,
                "seq": seq,
                "grids": grids,
                "state": latest.state.value,
                "levels_completed": int(latest.levels_completed),
                "baseline": self.baseline,
                "action": action.name if action is not None else None,
                "action_x": getattr(data, "x", None),
                "action_y": getattr(data, "y", None),
            })
        except queue.Full:
            # ponytail: drop instead of applying backpressure. A seq gap is a frame the
            # viewer skips; a blocked producer is a slower scored run, and the ARC loop is
            # aborted as an infrastructure failure if it misses its turns-per-30s target.
            pass
        except Exception:
            pass  # a viewer glitch must never fail a scored game

    def close(self) -> None:
        if self.thread is None:
            return
        try:
            self.queue.put(STOP, timeout=1)
        except queue.Full:
            pass
        self.thread.join(timeout=3)

    def _drain(self) -> None:
        while True:
            batch = [self.queue.get()]
            total = len(batch[0]["grids"])
            while len(batch) < BATCH_FRAMES and total < BATCH_CHARS and batch[-1] is not STOP:
                try:
                    batch.append(self.queue.get_nowait())
                except queue.Empty:
                    break
                total += len(batch[-1]["grids"])
            done = batch[-1] is STOP
            if done:
                batch.pop()
            if batch and not self.stopped:
                self._post(batch)
            if done:
                return

    def _post(self, rows: list) -> None:
        if self.ndjson:
            print(FRAME_PREFIX + json.dumps({"frames": rows}, separators=(",", ":")),
                  file=sys.stderr, flush=True)
            return
        request = urllib.request.Request(
            self.url,
            data=json.dumps({
                "run_id": self.run_id,
                "lease_token": self.lease_token,
                "frames": rows,
            }).encode(),
            headers={"X-Worker-Token": self.token, "Content-Type": "application/json"},
            method="POST",
        )
        try:
            with urllib.request.urlopen(request, timeout=10) as response:
                response.read()
        except urllib.error.HTTPError as exc:
            if exc.code == 409:
                self.stopped = True  # the run is finished or gone; stop talking to the site
        except Exception:
            pass


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
        "-e", "VLLM_BASE_URL=http://127.0.0.1:8000/v1",
        "-e", f"VLLM_API_KEY={os.environ['BEDROCK_GATEWAY_TOKEN']}",
        "-e", "VLLM_START_SERVER=0",
        "-e", f"VLLM_SERVED_MODEL_NAME={os.environ['BEDROCK_PROFILE_NAME']}",
        "-e", "VLLM_REQUEST_TIMEOUT=18",
        "-e", "LLM_MEMORY_DIR=/tmp/agent-memory",
        "-e", "LLM_TRACE_PATH=/tmp/llm-trace.jsonl",
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
    info = getattr(env, "info", None)
    baseline = list(getattr(info, "baseline_actions", None) or []) or None
    feed = FrameFeed(args.run_id, args.game, baseline)
    try:
        feed.publish(latest)
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
            if response.get("done") is True:
                break
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
            feed.publish(latest, action)
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
        feed.close()
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


def self_check() -> None:
    """Guards the wire format the live viewer decodes: 4096 lowercase nibbles per board."""
    grid = [[(row + column) % 16 for column in range(64)] for row in range(64)]
    text = encode_grid(grid)
    assert len(text) == GRID_CHARS, len(text)
    assert set(text) <= set(HEX), sorted(set(text) - set(HEX))
    decoded = [[int(char, 16) for char in text[row * 64:(row + 1) * 64]] for row in range(64)]
    assert decoded == grid, "hex round trip changed the board"

    buffer = []
    for index in range(20):
        board = [[0] * 64 for _ in range(64)]
        board[0][0] = index % 16
        board[0][1] = index // 16  # keeps all 20 boards distinct through the nibble encoding
        buffer.append(board)
    capped = encode_grids(buffer).split("\n")
    assert len(capped) == MAX_GRIDS, len(capped)
    assert capped[0] == encode_grid(buffer[0])
    assert capped[-2] == encode_grid(buffer[MAX_GRIDS - 2])
    assert capped[-1] == encode_grid(buffer[-1]), "the resulting board must stay last"
    assert encode_grids(buffer[:MAX_GRIDS]).split("\n") == [encode_grid(g) for g in buffer[:MAX_GRIDS]]
    assert encode_grids([]) == ""
    assert encode_grids([[[0] * 64] * 63]) == "", "a board that is not 64x64 must be dropped"
    print("arc_game self-check ok")


if __name__ == "__main__":
    if "--self-check" in sys.argv:
        self_check()
    else:
        main()
