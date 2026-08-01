#!/usr/bin/env python3
"""Run one cached ARC game in-process without Academy, containers, or network."""
from __future__ import annotations

import argparse
import importlib.util
import sys
from pathlib import Path

import arc_agi
from arc_agi import OperationMode


def load_agent(submission: Path, vendor: Path):
    sys.path[:0] = [str(submission), str(vendor)]
    source = submission / "agent" / "my_agent.py"
    if not source.is_file():
        raise SystemExit(f"{submission} must contain agent/my_agent.py")
    spec = importlib.util.spec_from_file_location("local_submission_agent", source)
    if spec is None or spec.loader is None:
        raise SystemExit(f"could not load {source}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    if not hasattr(module, "MyAgent"):
        raise SystemExit(f"{source} must define MyAgent")
    return module.MyAgent


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--game", default="ls20")
    parser.add_argument("--max-steps", type=int, default=50)
    parser.add_argument("--cache", type=Path, required=True)
    parser.add_argument("--repo", type=Path, required=True)
    args = parser.parse_args()
    if args.max_steps < 1:
        parser.error("--max-steps must be positive")

    starter = args.cache.resolve()
    submission = args.repo.resolve()
    agent_class = load_agent(submission, starter / "vendor" / "ARC-AGI-3-Agents")
    arcade = arc_agi.Arcade(
        operation_mode=OperationMode.OFFLINE,
        environments_dir=str(starter / "environment_files"),
    )
    environment = arcade.make(args.game)
    if environment is None:
        raise SystemExit(f"cached game not found: {args.game}")

    if hasattr(agent_class, "MAX_ACTIONS"):
        agent_class.MAX_ACTIONS = min(agent_class.MAX_ACTIONS, args.max_steps)
    agent = agent_class(
        card_id="local-dev",
        game_id=args.game,
        agent_name=f"MyAgent.local.{args.game}",
        ROOT_URL="http://localhost",
        record=False,
        arc_env=environment,
        tags=["local-dev", "offline"],
    )
    agent.main()

    final = agent.frames[-1]
    scorecard = arcade.get_scorecard()
    score = scorecard.score if hasattr(scorecard, "score") else scorecard
    print(
        f"game={args.game} state={final.state} levels={final.levels_completed} "
        f"actions={agent.action_counter} score={score}"
    )


if __name__ == "__main__":
    main()
