#!/usr/bin/env python3
"""Build a self-contained ARC-AGI-3 Kaggle notebook from a validated repo."""
from __future__ import annotations

import argparse
import base64
import importlib.util
import io
import json
import os
import zipfile
from pathlib import Path
from textwrap import dedent

MAX_ARCHIVE_BYTES = 50 * 1024 * 1024


def starter_module(path: Path):
    spec = importlib.util.spec_from_file_location("arc_starter_notebook", path)
    if spec is None or spec.loader is None:
        raise RuntimeError("could not load the pinned ARC notebook builder")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def archive(repo: Path) -> bytes:
    output = io.BytesIO()
    with zipfile.ZipFile(output, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9) as bundle:
        for root, directories, files in os.walk(repo):
            directories[:] = [name for name in directories if name not in (".git", "__pycache__", ".kaggle")]
            for name in sorted(files):
                path = Path(root) / name
                relative = path.relative_to(repo)
                if path.is_symlink() or name.endswith((".pyc", ".pyo")) or name == ".env":
                    continue
                info = zipfile.ZipInfo(str(relative), date_time=(2026, 1, 1, 0, 0, 0))
                info.compress_type = zipfile.ZIP_DEFLATED
                info.external_attr = 0o600 << 16
                bundle.writestr(info, path.read_bytes())
    value = output.getvalue()
    if len(value) > MAX_ARCHIVE_BYTES:
        raise RuntimeError("compressed Kaggle source bundle exceeds 50 MiB")
    return value


def build(repo: Path, output: Path, username: str, slug: str, starter: Path) -> None:
    module = starter_module(starter / "scripts" / "build_notebook.py")
    notebook = module.build()
    encoded = base64.b64encode(archive(repo)).decode()
    notebook["cells"][0]["source"] = (
        "# Exposure Academy — ARC-AGI-3 official submission\n\n"
        "Generated from the exact commit scored by the local harness."
    )
    notebook["cells"][2] = module.code_cell(dedent(f"""\
        import base64, io, pathlib, zipfile
        bundle = base64.b64decode({encoded!r})
        root = pathlib.Path('/tmp/academy_submission')
        root.mkdir(parents=True, exist_ok=True)
        with zipfile.ZipFile(io.BytesIO(bundle)) as zf:
            zf.extractall(root)
    """))
    notebook["cells"][3] = module.code_cell(dedent('''\
        import os

        if os.getenv('KAGGLE_IS_COMPETITION_RERUN'):
            !curl --fail --retry 999 --retry-all-errors --retry-delay 5 \\
                  --retry-max-time 600 http://gateway:8001/api/games
            !cp -r /kaggle/input/competitions/arc-prize-2026-arc-agi-3/ARC-AGI-3-Agents \\
                   /kaggle/working/ARC-AGI-3-Agents

            import sys
            sys.path.insert(0, '/tmp/academy_submission')
            with open('/kaggle/working/ARC-AGI-3-Agents/agents/__init__.py', 'w') as f:
                f.write("""from typing import Type
        from dotenv import load_dotenv
        from .agent import Agent, Playback
        from .swarm import Swarm
        from .templates.random_agent import Random
        from agent.my_agent import MyAgent

        load_dotenv()
        AVAILABLE_AGENTS: dict[str, Type[Agent]] = {'random': Random, 'myagent': MyAgent}
        """)
            with open('/kaggle/working/ARC-AGI-3-Agents/.env', 'w') as f:
                f.write("""SCHEME=http
        HOST=gateway
        PORT=8001
        ARC_API_KEY=test-key-123
        ARC_BASE_URL=http://gateway:8001/
        OPERATION_MODE=online
        ENVIRONMENTS_DIR=
        RECORDINGS_DIR=/kaggle/working/server_recording
        """)
            !cd /kaggle/working/ARC-AGI-3-Agents && \\
                PYTHONPATH=/tmp/academy_submission:$PYTHONPATH \\
                MPLBACKEND=agg python main.py --agent myagent
    '''))
    output.mkdir(parents=True, exist_ok=True)
    (output / "submission.ipynb").write_text(json.dumps(notebook, indent=1))
    metadata = {
        "id": f"{username}/{slug}",
        "title": f"Exposure Academy ARC-AGI-3 {slug[-8:]}",
        "code_file": "submission.ipynb",
        "language": "python",
        "kernel_type": "notebook",
        "is_private": True,
        "enable_gpu": True,
        "enable_tpu": False,
        "enable_internet": False,
        "keywords": ["exposure-academy"],
        "dataset_sources": [],
        "kernel_sources": [],
        "competition_sources": ["arc-prize-2026-arc-agi-3"],
        "model_sources": [],
    }
    (output / "kernel-metadata.json").write_text(json.dumps(metadata, indent=2) + "\n")


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--repo", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--username", required=True)
    parser.add_argument("--slug", required=True)
    parser.add_argument("--starter", type=Path, required=True)
    args = parser.parse_args()
    build(args.repo, args.output, args.username, args.slug, args.starter)
