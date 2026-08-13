"""Trusted local script sent to a qualified Colab kernel by monopoly_controller.py."""

import json
import os
from pathlib import Path
import runpy
import shutil
import sys
import tarfile


root = Path("/content/exposure-monopoly-worker")
shutil.rmtree(root, ignore_errors=True)
root.mkdir()
with tarfile.open("/content/exposure-monopoly-worker.tar.gz", "r:gz") as archive:
    members = archive.getmembers()
    if any(
        Path(member.name).is_absolute()
        or ".." in Path(member.name).parts
        or member.issym()
        or member.islnk()
        for member in members
    ):
        raise RuntimeError("unsafe trusted worker bundle")
    archive.extractall(root, members=members, filter="data")
config = json.loads(Path("/content/exposure-monopoly-config.json").read_text())
os.environ.update(
    {
        "MONOPOLY_SITE": config["site"],
        "WORKER_TOKEN": config["worker_token"],
        "MONOPOLY_WORKER_ID": f"colab:{config['session_name']}",
        "MONOPOLY_WORKER_KIND": "colab",
        "MONOPOLY_SESSION_NAME": config["session_name"],
        "MONOPOLY_AGENT_BACKEND": "venv",
        "MONOPOLY_CACHE_DIR": "/content/exposure-monopoly-cache",
    }
)
sys.path.insert(0, str(root))
sys.argv = [
    str(root / "rl_monopoly_runner.py"),
    *(["--selftest"] if config.get("selftest") else []),
]
runpy.run_path(str(root / "rl_monopoly_runner.py"), run_name="__main__")
