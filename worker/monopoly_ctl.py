#!/usr/bin/env python3
"""Boot controller for the AI Monopoly GPU box.

Runs wherever worker/runner.py already runs — a small always-on machine. Polls the site
for queued Monopoly work and, if there is any and the GPU instance is stopped, starts it.
That is the whole job: the GPU box runs monopoly_runner.py from its own user-data and
stops *itself* after 15 idle minutes, so nothing here ever has to decide when to shut
down. Keeping the stop decision on the box means a crash of this controller costs money
for at most one idle window, not indefinitely.

Talks to EC2 through the `aws` CLI rather than boto3, so this stays stdlib-only like the
other workers and needs no virtualenv on the host.

Env:
  MONOPOLY_GPU_INSTANCE   i-0123...        (required)
  AWS_REGION              eu-central-1     (or configured in ~/.aws)
  MONOPOLY_SITE           https://...      (default: HARNESS_SITE, then localhost)
  WORKER_TOKEN            shared secret    (or .env beside the repo)

Run:  python3 worker/monopoly_ctl.py            poll forever
      python3 worker/monopoly_ctl.py --once     check once, then exit
      python3 worker/monopoly_ctl.py --dry-run  say what it would do, start nothing
"""
from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent
SITE = os.environ.get("MONOPOLY_SITE", os.environ.get("HARNESS_SITE", "http://127.0.0.1:3000"))
ENV_FILE = ROOT.parent / ".env"
POLL_SECONDS = 60


def env_from_file() -> dict:
    vals = {}
    if not ENV_FILE.exists():
        return vals
    with open(ENV_FILE) as f:
        for line in f:
            line = line.strip()
            if line and not line.startswith("#") and "=" in line:
                k, v = line.split("=", 1)
                vals[k] = v.strip().strip('"')
    return vals


ENV = env_from_file()
WORKER_TOKEN = os.environ.get("WORKER_TOKEN") or ENV.get("WORKER_TOKEN", "")
INSTANCE = os.environ.get("MONOPOLY_GPU_INSTANCE") or ENV.get("MONOPOLY_GPU_INSTANCE", "")
REGION = os.environ.get("AWS_REGION") or ENV.get("AWS_REGION", "")


def log(msg: str) -> None:
    print(f"[{time.strftime('%H:%M:%S')}] {msg}", flush=True)


def aws(*args) -> dict:
    cmd = ["aws", "ec2", *args, "--output", "json"]
    if REGION:
        cmd += ["--region", REGION]
    out = subprocess.run(cmd, capture_output=True, text=True, timeout=120)
    if out.returncode != 0:
        raise RuntimeError(f"aws {' '.join(args)} failed: {out.stderr.strip()}")
    return json.loads(out.stdout) if out.stdout.strip() else {}


def instance_state() -> str:
    d = aws("describe-instances", "--instance-ids", INSTANCE)
    return d["Reservations"][0]["Instances"][0]["State"]["Name"]


def work_waiting() -> bool:
    """True if the site has a tournament or a practice match nobody has finished.

    NOTE this is a plain GET of the same endpoint the runner claims from — but claiming is
    what the runner's POST-shaped calls do, and a GET here would race it. It doesn't: the
    tournament claim is idempotent for an already-claimed run (the handler falls back to
    "whichever is non-terminal"), and a practice claim flipped to running by this poll
    would be re-served on the next GET anyway. Worst case the box boots for work that is
    already in progress, which is the state we want it in regardless.
    """
    req = urllib.request.Request(
        SITE + "/api/worker/monopoly/pending",
        headers={"X-Worker-Token": WORKER_TOKEN})
    try:
        with urllib.request.urlopen(req, timeout=30) as r:
            return json.loads(r.read() or b"{}").get("kind", "none") != "none"
    except urllib.error.HTTPError as e:
        log(f"site returned HTTP {e.code} — not booting anything")
        return False
    except Exception as e:
        log(f"site unreachable ({e}) — not booting anything")
        return False


def tick(dry: bool) -> None:
    if not work_waiting():
        return
    state = instance_state()
    if state == "running":
        log("work waiting, GPU box already up")
        return
    if state != "stopped":
        # stopping / pending / shutting-down: it's mid-transition, next tick will see it
        log(f"work waiting, but instance is '{state}' — waiting for it to settle")
        return
    if dry:
        log(f"DRY RUN: would start {INSTANCE}")
        return
    log(f"work waiting, starting {INSTANCE}")
    aws("start-instances", "--instance-ids", INSTANCE)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--once", action="store_true", help="check once, then exit")
    ap.add_argument("--dry-run", action="store_true", help="report, start nothing")
    args = ap.parse_args()

    if not WORKER_TOKEN:
        sys.exit("WORKER_TOKEN missing (.env or environment)")
    if not INSTANCE:
        sys.exit("MONOPOLY_GPU_INSTANCE missing (.env or environment)")

    while True:
        try:
            tick(args.dry_run)
        except Exception as e:
            log(f"tick failed: {e}")     # a bad AWS call must not kill the controller
        if args.once:
            return
        time.sleep(POLL_SECONDS)


if __name__ == "__main__":
    main()
