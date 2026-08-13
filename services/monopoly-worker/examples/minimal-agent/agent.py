"""Smallest valid AI Monopoly submission."""


def choose_action(state, player_id, allowed_actions):
    descriptions = state["actions"]
    buy = next(
        (
            action
            for action in allowed_actions
            if descriptions.get(str(action)) == "BUY_PROPERTY"
        ),
        None,
    )
    return buy if buy is not None else allowed_actions[0]
