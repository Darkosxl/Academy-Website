#!/usr/bin/env python3
"""Worker-owned JSON-lines adapter for a submitted root-level agent.py."""

import contextlib
import importlib.util
import json
import os
import sys
from pathlib import Path


SUBMISSION_DIR = Path(
    os.environ.get("EXPOSURE_SUBMISSION_DIR", "/app/submission")
).resolve()
AGENT_PATH = os.environ.get("EXPOSURE_AGENT_PATH", "agent.py")
ENTRYPOINT = (SUBMISSION_DIR / AGENT_PATH).resolve()


def load_choose_action():
    if (
        SUBMISSION_DIR not in ENTRYPOINT.parents
        or not ENTRYPOINT.is_file()
        or ENTRYPOINT.is_symlink()
    ):
        raise RuntimeError("configured agent path is not a regular submission file")
    sys.path.insert(0, str(SUBMISSION_DIR))
    spec = importlib.util.spec_from_file_location("exposure_submission", ENTRYPOINT)
    if spec is None or spec.loader is None:
        raise RuntimeError("agent.py could not be loaded")
    module = importlib.util.module_from_spec(spec)
    # Student prints belong on stderr; stdout is reserved for the protocol.
    with contextlib.redirect_stdout(sys.stderr):
        spec.loader.exec_module(module)
    choose = getattr(module, "choose_action", None)
    if not callable(choose):
        raise RuntimeError(
            "agent.py must define choose_action(state, player_id, allowed_actions)"
        )
    return choose


def main() -> None:
    choose_action = load_choose_action()
    print(json.dumps({"ready": True}, separators=(",", ":")), flush=True)
    for raw in sys.stdin:
        try:
            request = json.loads(raw)
            allowed = request["allowed_actions"]
            if not isinstance(allowed, list) or not allowed:
                raise ValueError("allowed_actions must be a non-empty list")
            with contextlib.redirect_stdout(sys.stderr):
                action = choose_action(request["state"], request["player_id"], allowed)
            if isinstance(action, bool) or not isinstance(action, int):
                raise TypeError("choose_action must return an integer")
            if action not in allowed:
                raise ValueError(f"illegal action {action}; allowed={allowed}")
            response = {"action": action}
        except Exception as exc:
            response = {"error": f"{type(exc).__name__}: {exc}"}
        print(json.dumps(response, separators=(",", ":")), flush=True)


if __name__ == "__main__":
    main()
