"""Deterministic read/write wrapper for the canonical ppo-plus-v2 runtime engine."""

from __future__ import annotations

import copy
from dataclasses import dataclass
import random

from monopoly_game_engine.actions import ACTION_SPACE_SIZE
from monopoly_game_engine.constants import NUM_PLAYERS, RULESET_VERSION
from monopoly_game_engine.env import MonopolyEnv
from monopoly_game_engine.state import STATE_DIM


if (RULESET_VERSION, STATE_DIM, ACTION_SPACE_SIZE, NUM_PLAYERS) != (
    "ppo-plus-v2",
    300,
    2_958,
    4,
):
    raise RuntimeError("The installed Monopoly engine is not ppo-plus-v2")


@dataclass(slots=True)
class SharedGame:
    """A cloneable engine state whose stdlib RNG never leaks into agent code."""

    env: MonopolyEnv
    random_state: object

    @classmethod
    def new(cls, seed: int, max_rounds: int = 200) -> "SharedGame":
        outer = random.getstate()
        try:
            random.seed(seed)
            env = MonopolyEnv(agent_ids=[0], max_rounds=max_rounds)
            state = random.getstate()
        finally:
            random.setstate(outer)
        return cls(env, state)

    def clone(self) -> "SharedGame":
        return copy.deepcopy(self)

    def step(self, action: int):
        outer = random.getstate()
        try:
            random.setstate(self.random_state)
            result = self.env.step(action)
            self.random_state = random.getstate()
            return result
        finally:
            random.setstate(outer)
