#!/usr/bin/env python3
"""Agentic Harness runner — the real thing.

Polls the site for queued runs and scores each one on actual ARC-AGI-3 (local
game engine via the Kaggle starter), actual Frontier-bench (Harbor + Docker),
and real PSS RAM measurement. Posts stage transitions, live progress, and
honest final scores. Stdlib only; the benchmarks bring their own venvs.

One-time host setup (already done on this box):
  - worker/cache/arc-starter/   : ARC-AGI-3-Kaggle-Starter checkout + `make setup`
  - worker/cache/frontier-bench/: `harbor datasets download frontier-bench/frontier-bench`
  - `uv tool install harbor`, Docker daemon running
  - .env with WORKER_TOKEN + CEREBRAS_API_KEY

Run:  python3 worker/runner.py            (production: full non-GPU set)
      SMOKE_MODE=1 python3 worker/runner.py   (2 frontier tasks, 2 short ARC games)
"""
from __future__ import annotations

import contextlib
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import time
import urllib.request
from pathlib import Path

# ── config ──────────────────────────────────────────────────────────────────
ROOT = Path(__file__).resolve().parent
SITE = os.environ.get("HARNESS_SITE", "http://127.0.0.1:3000")
ENV_FILE = ROOT.parent / ".env"
POLL_SECONDS = 60

ARC_STARTER = ROOT / "cache" / "arc-starter"
FRONTIER_DATASET = ROOT / "cache" / "frontier-bench" / "frontier-bench"
GPU_TASKS = ["fp8-rmsnorm-gemm", "exam-pdf-eval", "math-eval-grader", "jax-speedrun-gpu"]

SMOKE = os.environ.get("SMOKE_MODE") == "1"
ARC_GAMES = "ls20,vc33" if SMOKE else None      # None = all games (competition rerun)
ARC_MAX_STEPS = 30 if SMOKE else 200
ARC_TIMEOUT = 900 if SMOKE else 3 * 3600
FRONTIER_N_TASKS = 2 if SMOKE else None         # None = full non-GPU set (70)
FRONTIER_CONCURRENCY = 4
FRONTIER_TIMEOUT = 2400 if SMOKE else 12 * 3600
SMOKE_TIMEOUT_BUILD = 60
# RAM-bench is its own benchmark: the student's own main.py session, nothing to do
# with the ARC engine or Harbor. Sessions get a fixed window so a long-running agent
# and a quick one are compared over the same interval.
RAM_SESSION_SECONDS = 30
RAM_SAMPLE_SECONDS = 0.05
RAM_CONCURRENT = 10


def env_from_file() -> dict:
    vals = {}
    with open(ENV_FILE) as f:
        for line in f:
            line = line.strip()
            if line and not line.startswith("#") and "=" in line:
                k, v = line.split("=", 1)
                vals[k] = v.strip().strip('"')
    return vals


ENV = env_from_file()
WORKER_TOKEN = ENV["WORKER_TOKEN"]
LLM_ENV = {
    "HARNESS_LLM_BASE": ENV.get("HARNESS_LLM_BASE", "https://api.cerebras.ai/v1"),
    "HARNESS_LLM_KEY": ENV.get("HARNESS_LLM_KEY") or ENV["CEREBRAS_API_KEY"],
    "HARNESS_LLM_MODEL": ENV.get("HARNESS_LLM_MODEL", "gemma-4-31b"),
}
# Both reference harnesses reach the provider through their own conventions rather
# than our HARNESS_* names: the ARC framework builds an OpenAI SDK client (which
# reads OPENAI_BASE_URL / OPENAI_API_KEY from the environment), and Terminus 2
# calls LiteLLM, which wants a "<provider>/<model>" string plus that provider's
# own key variable. Same endpoint and key, three spellings.
OPENAI_ENV = {
    "OPENAI_BASE_URL": LLM_ENV["HARNESS_LLM_BASE"],
    "OPENAI_API_KEY": LLM_ENV["HARNESS_LLM_KEY"],
}
LITELLM_PROVIDER = ENV.get("HARNESS_LLM_PROVIDER", "cerebras")
FRONTIER_MODEL = f"{LITELLM_PROVIDER}/{LLM_ENV['HARNESS_LLM_MODEL']}"
LITELLM_ENV = {f"{LITELLM_PROVIDER.upper()}_API_KEY": LLM_ENV["HARNESS_LLM_KEY"]}


# ── site API ────────────────────────────────────────────────────────────────
def api(path: str, body=None):
    req = urllib.request.Request(
        SITE + path,
        data=json.dumps(body).encode() if body is not None else None,
        headers={"X-Worker-Token": WORKER_TOKEN, "Content-Type": "application/json"},
        method="POST" if body is not None else "GET",
    )
    with urllib.request.urlopen(req) as r:
        raw = r.read()
        return r.status, json.loads(raw) if raw else None


def stage(run_id, name, sha=None):
    body = {"id": run_id, "stage": name}
    if sha:
        body["commit_sha"] = sha
    status, _ = api("/api/worker/harness/stage", body)
    log(f"stage -> {name}: HTTP {status}")


def progress(run_id, done=None, total=None, score=None, detail=None):
    p = {k: v for k, v in
         (("done", done), ("total", total), ("score", score), ("detail", detail))
         if v is not None}
    try:
        api("/api/worker/harness/progress", {"id": run_id, "progress": json.dumps(p)})
    except Exception as e:  # progress is cosmetic — never let it kill a run
        log(f"progress post failed: {e}")


class RunFailed(Exception):
    pass


def log(msg):
    print(f"[{time.strftime('%H:%M:%S')}] {msg}", flush=True)


# ── PSS RAM probe ───────────────────────────────────────────────────────────
def pss_kb(pid: int) -> int:
    try:
        with open(f"/proc/{pid}/smaps_rollup") as f:
            for line in f:
                if line.startswith("Pss:"):
                    return int(line.split()[1])
    except OSError:
        pass
    return 0


def arc_cmd(games, max_steps):
    cmd = [str(ARC_STARTER / ".venv" / "bin" / "python"), "scripts/play_local.py",
           "--max-steps", str(max_steps)]
    if games:
        cmd += ["--game", games]
    return cmd


def ram_sessions(repo: Path, venv_py: Path, procs: int) -> float:
    """RAM-bench, standalone: `procs` concurrent `main.py` sessions of the student's
    own agent — no game engine, no Harbor. Samples summed PSS over a fixed window and
    returns the peak in MB. Sessions still running at the end of the window are killed;
    ones that exit early keep the peak they reached.
    """
    env = {**os.environ, **LLM_ENV, "HARNESS_RAM_PROBE": "1"}
    ps = [subprocess.Popen([str(venv_py), "main.py"], cwd=repo, env=env,
                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
          for _ in range(procs)]
    peak = 0
    deadline = time.time() + RAM_SESSION_SECONDS
    try:
        while time.time() < deadline:
            live = [p for p in ps if p.poll() is None]
            if not live:
                break
            peak = max(peak, sum(pss_kb(p.pid) for p in live))
            time.sleep(RAM_SAMPLE_SECONDS)
    finally:
        for p in ps:
            if p.poll() is None:
                p.kill()
            p.wait()
    if peak == 0:
        raise RunFailed("RAM ölçülemedi: main.py ölçüm penceresinde hiç çalışmadı.")
    return round(peak / 1024, 1)


# ── stages ──────────────────────────────────────────────────────────────────
def clone(repo_url: str, work: Path) -> tuple[Path, str]:
    repo = work / "repo"
    r = subprocess.run(["git", "clone", "--depth", "1", repo_url, str(repo)],
                       capture_output=True, text=True, timeout=300)
    if r.returncode != 0:
        raise RunFailed(f"git clone başarısız:\n{r.stderr[-2000:]}")
    sha = subprocess.run(["git", "-C", str(repo), "rev-parse", "HEAD"],
                         capture_output=True, text=True).stdout.strip()
    return repo, sha


def build(repo: Path, work: Path) -> Path:
    """Contract v2 validation + deps + smoke test. Returns the repo's own venv python.

    Two installs on purpose: the repo gets a clean venv of its own (used by the smoke
    test and RAM-bench, so student pins can't collide with arc-agi's), and the ARC
    starter venv gets the same deps because play_local.py imports the student's agent
    in-process.
    """
    missing = [p for p in ("agent/my_agent.py", "agent/harbor_agent.py",
                           "main.py", "requirements.txt")
               if not (repo / p).exists()]
    if missing:
        raise RunFailed("Depo yapısı kurallara uymuyor, eksik: " + ", ".join(missing))

    reqs = str(repo / "requirements.txt")
    venv = work / "venv"
    subprocess.run([sys.executable, "-m", "venv", str(venv)], check=True, timeout=180)
    venv_py = venv / "bin" / "python"
    for py in (venv_py, ARC_STARTER / ".venv" / "bin" / "python"):
        r = subprocess.run([str(py), "-m", "pip", "install", "-q", "-r", reqs],
                           capture_output=True, text=True, timeout=600)
        if r.returncode != 0:
            raise RunFailed(f"pip install başarısız:\n{r.stderr[-2000:]}")

    r = subprocess.run([str(venv_py), "main.py"], cwd=repo,
                       capture_output=True, text=True, timeout=SMOKE_TIMEOUT_BUILD)
    if r.returncode != 0:
        raise RunFailed(f"main.py duman testi başarısız:\n{(r.stderr or r.stdout)[-2000:]}")
    return venv_py


@contextlib.contextmanager
def overlaid_agent(repo: Path):
    """Swap the student's my_agent.py into the cached starter for the ARC stage only.

    play_local.py loads the agent from a fixed path in the starter tree, so the
    student's file has to sit there while the games run; the starter's own file is put
    back afterwards.
    """
    target = ARC_STARTER / "agent" / "my_agent.py"
    backup = target.read_bytes()
    shutil.copyfile(repo / "agent" / "my_agent.py", target)
    try:
        yield
    finally:
        target.write_bytes(backup)


def run_arc(run_id) -> float:
    """Play the real local game engine with the student's agent already overlaid."""
    # The framework's LLM agents build an OpenAI client with no explicit base_url,
    # so the SDK's own OPENAI_* env vars are what point them at our provider.
    env = {**os.environ, **LLM_ENV, **OPENAI_ENV}
    p = subprocess.Popen(arc_cmd(ARC_GAMES, ARC_MAX_STEPS), cwd=ARC_STARTER, env=env,
                         stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    score = None
    done = total = 0
    levels = 0
    tail = []
    deadline = time.time() + ARC_TIMEOUT
    for line in p.stdout:
        tail = (tail + [line])[-80:]
        if time.time() > deadline:
            p.kill()
            raise RunFailed("ARC-AGI-3 zaman aşımına uğradı.")
        m = re.match(r"=== \[(\d+)/(\d+)\] (\S+) ===", line)
        if m:
            done, total = int(m.group(1)) - 1, int(m.group(2))
            progress(run_id, done, total, levels, m.group(3))
            log(f"ARC game {m.group(1)}/{total}: {m.group(3)}")
        m = re.search(r"levels_completed=(\d+), actions=", line)
        if m:
            levels += int(m.group(1))
            done += 1
            progress(run_id, done, total, levels)
        m = re.search(r"Aggregate scorecard score: ([\d.]+)", line)
        if m:
            score = float(m.group(1))
    p.wait()
    if score is None:
        raise RunFailed("ARC-AGI-3 skoru üretilemedi:\n" + "".join(tail)[-2000:])
    log(f"score_arc = {score}")
    return score


def trial_verdict(res: dict) -> tuple[str, str | None]:
    """Classify one Harbor trial result.json: resolved / unresolved / crashed.

    A crash means the trial raised (agent bug, LLM unreachable) and scored nothing —
    distinct from an honest 0 reward, which means the agent ran and failed the task.
    """
    ei = res.get("exception_info")
    if ei:
        return "crashed", f"{ei.get('exception_type')}: {ei.get('exception_message')}"
    rewards = (res.get("verifier_result") or {}).get("rewards") or {}
    val = rewards.get("reward")
    if val is None and rewards:
        val = next(iter(rewards.values()))
    try:
        return ("resolved" if val is not None and float(val) >= 1 else "unresolved"), None
    except (ValueError, TypeError):
        return "unresolved", None


def _selftest():
    assert trial_verdict({"verifier_result": {"rewards": {"reward": 1.0}}}) == ("resolved", None)
    assert trial_verdict({"verifier_result": {"rewards": {"reward": 0.0}}})[0] == "unresolved"
    assert trial_verdict({"verifier_result": {"rewards": {}}})[0] == "unresolved"
    assert trial_verdict({})[0] == "unresolved"
    v, msg = trial_verdict({"exception_info": {"exception_type": "HTTPError",
                                              "exception_message": "403"}})
    assert v == "crashed" and "403" in msg
    # a partial-credit board would score 0.5 as unresolved — pass/fail is the rule
    assert trial_verdict({"verifier_result": {"rewards": {"reward": 0.5}}})[0] == "unresolved"
    print("selftest ok")


def run_frontier(run_id, repo: Path, work: Path) -> float:
    jobs_dir = work / "jobs"
    cmd = ["harbor", "run", "-p", str(FRONTIER_DATASET),
           "-a", "agent.harbor_agent:HarborAgent", "-m", FRONTIER_MODEL,
           "-n", str(FRONTIER_CONCURRENCY), "-o", str(jobs_dir)]
    for t in GPU_TASKS:
        cmd += ["-x", t]
    if FRONTIER_N_TASKS:
        cmd += ["-l", str(FRONTIER_N_TASKS)]
    for k, v in LLM_ENV.items():
        cmd += ["--ae", f"{k}={v}"]

    total = FRONTIER_N_TASKS or (len([d for d in FRONTIER_DATASET.iterdir() if d.is_dir()])
                                 - len(GPU_TASKS))
    env = {**os.environ, **LLM_ENV, **LITELLM_ENV, "PYTHONPATH": str(repo)}
    logf = open(work / "harbor.log", "w")
    p = subprocess.Popen(cmd, cwd=repo, env=env, stdout=logf, stderr=subprocess.STDOUT)

    def scan():
        """(finished trials, resolved count, still-running task name, crash message)

        A trial that raised (agent bug, LLM unreachable) records exception_info and
        scores nothing. If *every* trial crashes the agent is broken, not bad — the
        run fails with that message instead of posting a meaningless 0%.
        """
        job = next((d for d in jobs_dir.iterdir() if d.is_dir()), None) \
            if jobs_dir.exists() else None
        if not job:
            return 0, 0, None, None
        finished = resolved = crashed = 0
        running = crash = None
        for trial in job.iterdir():
            if not trial.is_dir():
                continue
            rj = trial / "result.json"
            if not rj.exists():
                running = trial.name.split("__")[0]
                continue
            finished += 1
            try:
                res = json.loads(rj.read_text())
            except json.JSONDecodeError:
                continue
            verdict, msg = trial_verdict(res)
            if verdict == "crashed":
                crashed += 1
                crash = msg
            elif verdict == "resolved":
                resolved += 1
        return finished, resolved, running, (crash if crashed == finished else None)

    deadline = time.time() + FRONTIER_TIMEOUT
    while p.poll() is None:
        if time.time() > deadline:
            p.kill()
            raise RunFailed("Frontier-bench zaman aşımına uğradı.")
        finished, resolved, running, _ = scan()
        pct = round(100.0 * resolved / total, 1)
        progress(run_id, finished, total, pct, running)
        time.sleep(10)
    logf.close()

    finished, resolved, _, crash = scan()
    if finished == 0:
        tail = (work / "harbor.log").read_text()[-2000:]
        raise RunFailed(f"Frontier-bench hiçbir görevi çalıştıramadı:\n{tail}")
    if crash:
        raise RunFailed(
            "Frontier-bench görevlerinin tamamı ajan hatasıyla sonlandı — "
            f"agent/harbor_agent.py çalışmıyor:\n{crash[:1500]}")
    score = round(100.0 * resolved / total, 1)
    log(f"score_frontier = {score} ({resolved}/{total} resolved, {finished} finished)")
    return score


def run_ram(run_id, repo: Path, venv_py: Path) -> tuple[float, float]:
    progress(run_id, 0, 2, None, "1 oturum")
    ram1 = ram_sessions(repo, venv_py, 1)
    log(f"ram 1 session peak = {ram1} MB")
    progress(run_id, 1, 2, ram1, f"{RAM_CONCURRENT} oturum")
    ram10 = ram_sessions(repo, venv_py, RAM_CONCURRENT)
    log(f"ram {RAM_CONCURRENT} sessions peak = {ram10} MB")
    return ram1, ram10


# ── main loop ───────────────────────────────────────────────────────────────
def process(run_id: str, repo_url: str):
    work = Path(tempfile.mkdtemp(prefix="harness-run-"))
    try:
        repo, sha = clone(repo_url, work)
        stage(run_id, "building", sha)
        venv_py = build(repo, work)

        stage(run_id, "arc_agi_3")
        with overlaid_agent(repo):
            score_arc = run_arc(run_id)

        stage(run_id, "frontier_bench")
        score_frontier = run_frontier(run_id, repo, work)

        stage(run_id, "ram_bench")
        ram1, ram10 = run_ram(run_id, repo, venv_py)

        status, _ = api("/api/worker/harness/result", {
            "id": run_id, "status": "done",
            "score_arc": score_arc, "score_frontier": score_frontier,
            "ram_1session_mb": ram1, "ram_10session_mb": ram10,
        })
        log(f"result -> done: HTTP {status}")
    except RunFailed as e:
        api("/api/worker/harness/result",
            {"id": run_id, "status": "failed", "error_log": str(e)})
        log(f"run failed: {e}")
    except Exception as e:
        api("/api/worker/harness/result",
            {"id": run_id, "status": "failed", "error_log": f"worker hatası: {e}"})
        log(f"worker error: {e}")
    finally:
        shutil.rmtree(work, ignore_errors=True)


def main():
    log(f"runner up — site={SITE} smoke={SMOKE}")
    once = "--once" in sys.argv
    while True:
        try:
            _, pending = api("/api/worker/harness/pending")
        except Exception as e:
            log(f"pending poll failed: {e}")
            pending = None
        if pending:
            run = pending[0]
            log(f"claimed {run['id']} ({run['repo_url']})")
            process(run["id"], run["repo_url"])
            if once:
                return
        elif once:
            log("no queued run")
            return
        else:
            time.sleep(POLL_SECONDS)


if __name__ == "__main__":
    if "--selftest" in sys.argv:
        _selftest()
    else:
        main()
