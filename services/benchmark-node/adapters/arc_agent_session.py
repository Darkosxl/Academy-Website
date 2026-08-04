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
import types

sys.path.insert(0, "/submission")
sys.path.insert(0, "/opt/arc-agents")

# `agents/__init__.py` eagerly imports all nine bundled templates, so `from agents.agent
# import Agent` drags in langgraph, langchain and smolagents for one base class. Those are
# absent from the sandbox image and cannot be added at runtime (`--network=none`), and
# installing them would also pull the vendor's `openai==1.72.0` over the 2.44.0 the gateway
# client needs. Claim the package name first: submodules still resolve through __path__,
# but the template imports never run. agent.py itself only needs stdlib, pydantic and arc.
_agents = types.ModuleType("agents")
_agents.__path__ = ["/opt/arc-agents/agents"]
sys.modules.setdefault("agents", _agents)

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
            max_actions = int(getattr(agent, "MAX_ACTIONS", 80))
            if not 1 <= max_actions <= 10_000:
                raise ValueError("MyAgent.MAX_ACTIONS must be between 1 and 10000")
        emit({"ready": True, "max_actions": max_actions})
    except Exception:
        emit({"error": "agent import failed", "detail": traceback.format_exc()[-4000:]})
        return

    for line in sys.stdin:
        try:
            request = json.loads(line)
            latest = FrameData.model_validate(request["frame"])
            if request.get("append"):
                agent.append_frame(latest)
            if agent.action_counter >= max_actions or agent.is_done(agent.frames, latest):
                emit({"done": True})
                return
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
