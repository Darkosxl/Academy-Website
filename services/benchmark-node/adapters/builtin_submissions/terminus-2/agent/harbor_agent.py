from harbor.agents.terminus_2.terminus_2 import Terminus2


class HarborAgent(Terminus2):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self._prompt_template += (
            "\n\nOperating policy: inspect the environment before changing it, make the "
            "smallest useful change, then run the relevant test or direct verification. "
            "If a command fails, use its output to repair the issue and continue. Do not "
            "mark the task complete until you have direct evidence that it is complete."
        )
