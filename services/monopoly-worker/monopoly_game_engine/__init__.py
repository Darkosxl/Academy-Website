"""
monopoly_game_engine – Shared ppo-plus-v2 Monopoly simulator
=============================================================

Based on:
  "Decision Making in Monopoly Using a Hybrid Deep Reinforcement
   Learning Approach"
  Bonjour et al., IEEE TETCI, Vol. 6, No. 6, December 2022.

Vendored copy for runtime inference only (worker/rl_monopoly_runner.py).
train.py is intentionally not included here — training is not something
this deployment does — so this file drops the train_ppo/train_ddqn/
evaluate_agent helpers the original package exposed, since those need it.
"""

from .env import MonopolyEnv
from .agents_fixed import (
    FP_AGENT_CLASSES,
    TheHoarder,
    TheDealMaker,
    TheGambler,
    TheBuilder,
    TheBlocker,
    TheRailBaron,
    FPAgentA,
    FPAgentB,
    FPAgentC,
    FPAgentD,
    FPAgentE,
    FPAgentF,
)
from .state import build_state_vector
from .actions import ACTION_SPACE_SIZE, action_to_description


__all__ = [
    "MonopolyEnv",
    "FP_AGENT_CLASSES",
    "TheHoarder",
    "TheDealMaker",
    "TheGambler",
    "TheBuilder",
    "TheBlocker",
    "TheRailBaron",
    "FPAgentA",
    "FPAgentB",
    "FPAgentC",
    "FPAgentD",
    "FPAgentE",
    "FPAgentF",
    "build_state_vector",
    "ACTION_SPACE_SIZE",
    "action_to_description",
]
