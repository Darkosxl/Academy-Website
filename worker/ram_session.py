#!/usr/bin/env python3
"""Run one RAM-bench scenario inside a single measured container cgroup."""
from __future__ import annotations

import argparse
import json
import os
import subprocess
import time

PROMPT = (
    "Reply with exactly three concise bullets explaining how you would inspect "
    "this repository for a failing test. Do not edit files.\n"
)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--sessions", type=int, choices=(1, 10), required=True)
    args = parser.parse_args()
    env = os.environ.copy()
    env.pop("HARNESS_RAM_PROBE", None)
    processes = [
        subprocess.Popen(
            ["/venv/bin/python", "/submission/main.py"],
            cwd="/submission",
            env=env,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )
        for _ in range(args.sessions)
    ]
    for process in processes:
        process.stdin.write(PROMPT)
        process.stdin.close()

    deadline = time.monotonic() + 9.5
    while any(process.poll() is None for process in processes) and time.monotonic() < deadline:
        time.sleep(0.02)
    results = []
    ok = True
    for process in processes:
        if process.poll() is None:
            process.kill()
            ok = False
        process.wait()
        stdout = process.stdout.read()
        stderr = process.stderr.read()
        if process.returncode != 0 or not stdout.strip():
            ok = False
        results.append({
            "returncode": process.returncode,
            "stdout_bytes": len(stdout.encode()),
            "stderr": stderr[-1000:],
        })
    print(json.dumps({"ok": ok, "sessions": args.sessions, "results": results}, separators=(",", ":")))
    raise SystemExit(0 if ok else 1)


if __name__ == "__main__":
    main()
