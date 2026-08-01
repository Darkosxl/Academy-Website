import os

_PROFILE = {
    "ARC_AGENT_NAME": "forge_v46_gemma31b_public_single",
    "ARC_MODEL_PROFILE": "gemma31b_public_single",
    "LLM_ACTION_CANDIDATES": "1",
    "LLM_ACTION_CONTEXT_FRAMES": "4",
    "LLM_CANDIDATE_ARBITER": "0",
    "LLM_CLICK_FAILURE_RADIUS": "0",
    "LLM_CONFIDENCE_PROMPT": "0",
    "LLM_INCLUDE_FRAME_DESCRIPTOR": "0",
    "LLM_MAX_NEW_TOKENS": "1024",
    "LLM_MAX_PLAN_ACTIONS": "4",
    "LLM_REFLECTION_INTERVAL": "10",
    "LLM_REFLECTION_MAX_NEW_TOKENS": "10000",
    "LLM_TRACE_IMAGES": "0",
}

for key, value in _PROFILE.items():
    os.environ.setdefault(key, value)
