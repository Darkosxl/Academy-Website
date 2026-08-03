from harbor.agents.terminus_2.terminus_2 import Terminus2


class HarborAgent(Terminus2):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self._prompt_template += (
            "\n\nOperating policy: inspect the environment before changing it, make the "
            "smallest useful change, then run the relevant test or direct verification. "
            "If a command fails, use its output to repair the issue and continue. Do not "
            "mark the task complete until you have direct evidence that it is complete. "
            "This is a terminal task: begin by inspecting the workspace and never declare "
            "completion without first using the terminal."
        )

    def _get_completion_confirmation_message(self, terminal_output: str) -> str:
        return (
            super()._get_completion_confirmation_message(terminal_output)
            + "\n\nVerification gate: only confirm completion when the terminal output above "
            "shows a successful relevant test or direct verification after your latest change. "
            "Otherwise set task_complete to false, continue investigating, and run that check. "
            "A task with no terminal actions is not complete."
        )
