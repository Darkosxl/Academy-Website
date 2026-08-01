"""Compact real-LLM ARC agent used only by worker/live_arc.py."""
from __future__ import annotations

import json
import os
from itertools import groupby
from typing import Any

from arcengine import FrameData, GameAction, GameState
from openai import OpenAI

from agents.agent import Agent


def encode_row(row: list[int]) -> str:
    parts: list[str] = []
    for value, repeated in groupby(row):
        count = len(list(repeated))
        parts.append(f"{value}x{count}" if count > 1 else str(value))
    return ",".join(parts)


def encode_grid(grid: list[list[int]]) -> str:
    rows = [encode_row(row) for row in grid]
    encoded: list[str] = []
    for row, repeated in groupby(rows):
        count = len(list(repeated))
        encoded.append(f"{count}*({row})" if count > 1 else row)
    return ";".join(encoded)


def action_tool(legal: list[GameAction]) -> dict[str, Any]:
    names = [action.name for action in legal]
    return {
        "type": "function",
        "function": {
            "name": "take_action",
            "description": "Choose and dispatch exactly one currently legal game action.",
            "parameters": {
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "enum": names,
                        "description": "The legal action to dispatch.",
                    },
                    "x": {"type": "integer", "minimum": 0, "maximum": 63},
                    "y": {"type": "integer", "minimum": 0, "maximum": 63},
                },
                "required": ["action"],
                "additionalProperties": False,
            },
            "strict": True,
        },
    }


class MyAgent(Agent):
    """Use one compact required Grok tool call for every action after reset."""

    MODEL = os.environ.get("HARNESS_LLM_MODEL", "bedrock-harness")

    def __init__(self, *args: Any, **kwargs: Any) -> None:
        super().__init__(*args, **kwargs)
        self.client = OpenAI()
        self.actions: list[str] = []

    def is_done(self, frames: list[FrameData], latest_frame: FrameData) -> bool:
        return latest_frame.state in (GameState.WIN, GameState.GAME_OVER)

    def choose_action(
        self, frames: list[FrameData], latest_frame: FrameData
    ) -> GameAction:
        if not self.actions:
            self.actions.append(GameAction.RESET.name)
            return GameAction.RESET

        legal = [GameAction.from_id(value) for value in latest_frame.available_actions]
        prompt = (
            f"game={self.game_id}; state={latest_frame.state.name}; "
            f"levels={latest_frame.levels_completed}/{latest_frame.win_levels}; "
            f"recent_actions={','.join(self.actions[-8:])}; "
            "grid_rle rows use valuexcount and repeated rows use count*(row):\n"
            f"{encode_grid(latest_frame.frame[-1])}"
        )
        response = self.client.chat.completions.create(
            model=self.MODEL,
            messages=[
                {
                    "role": "system",
                    "content": (
                        "Play ARC-AGI-3. Infer the controls and objective from the grid. "
                        "Call exactly one legal action tool; minimize moves and avoid death."
                    ),
                },
                {"role": "user", "content": prompt},
            ],
            tools=[action_tool(legal)],
            tool_choice={"type": "function", "function": {"name": "take_action"}},
            max_tokens=128,
        )
        calls = response.choices[0].message.tool_calls or []
        if not calls:
            usage = response.usage
            raise RuntimeError(
                "model returned no action tool call "
                f"(finish={response.choices[0].finish_reason}, "
                f"output_tokens={usage.completion_tokens if usage else 'unknown'})"
            )
        call = calls[0]
        data = json.loads(call.function.arguments or "{}")
        action = GameAction.from_name(data.pop("action"))
        if action.is_complex():
            action.set_data(data)
        self.actions.append(action.name)
        return action
