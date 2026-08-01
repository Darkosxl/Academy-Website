#!/usr/bin/env python3
"""Untrusted-side ARC policy adapter.

JSON lines enter on stdin and actions leave on stdout. The trusted controller owns
the game engine and scorecard; this process can only inspect supplied observations
and choose an action.
"""
from __future__ import annotations

import contextlib
import importlib
import json
import os
import sys
import time
import traceback

sys.path.insert(0, "/submission")
sys.path.insert(0, "/opt/arc-agents")

from arcengine import FrameData, GameAction  # noqa: E402

PROTOCOL_STDOUT = sys.stdout
sys.stdout = sys.stderr


def emit(payload: dict) -> None:
    PROTOCOL_STDOUT.write(json.dumps(payload, separators=(",", ":")) + "\n")
    PROTOCOL_STDOUT.flush()


def main() -> None:
    game_id = os.environ["HARNESS_ARC_GAME"]
    try:
        with contextlib.redirect_stdout(sys.stderr):
            module = importlib.import_module("agent.my_agent")
            cls = getattr(module, "MyAgent")
            agent = cls(
                card_id="academy-public",
                game_id=game_id,
                agent_name=f"academy.{game_id}",
                ROOT_URL="http://localhost",
                record=False,
                arc_env=None,
                tags=["academy", "public-sprint"],
            )
            agent.timer = time.time()
        emit({"ready": True})
    except Exception:
        emit({"error": "agent import failed", "detail": traceback.format_exc()[-4000:]})
        return

    for line in sys.stdin:
        try:
            request = json.loads(line)
            latest = FrameData.model_validate(request["frame"])
            if request.get("append"):
                agent.append_frame(latest)
            with contextlib.redirect_stdout(sys.stderr):
                action = agent.choose_action(agent.frames, latest)
            if not isinstance(action, GameAction):
                raise TypeError("choose_action must return arcengine.GameAction")
            agent.action_counter += 1
            data = action.action_data.model_dump(mode="json") if action.is_complex() else {}
            reasoning = getattr(action, "reasoning", None)
            emit({"action": action.name, "data": data, "reasoning": reasoning})
        except Exception:
            emit({"error": "choose_action failed", "detail": traceback.format_exc()[-4000:]})
            return


if __name__ == "__main__":
    main()
