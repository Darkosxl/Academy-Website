#!/usr/bin/env python3
"""Canary the EC2 -> Academy -> Supabase -> Academy -> browser-poll path.

Use only with a dedicated running harness row. The script appends synthetic ARC frames but
never posts a terminal result. Secrets are read from the environment so they do not appear
in the process list.
"""
from __future__ import annotations

import argparse
import json
import math
import os
import queue
import threading
import time
import urllib.error
import urllib.parse
import urllib.request
import uuid
from dataclasses import dataclass
from typing import Any

GAMES = (
    "ls20", "vc33", "ar25", "cn04", "s5i5", "sp80", "bp35",
    "ft09", "m0r0", "re86", "cd82", "sb26", "r11l",
)
GRID = "0" * 4096


class ApiFailure(RuntimeError):
    pass


class Client:
    def __init__(self, base: str, worker_token: str, session_cookie: str):
        self.base = base.rstrip("/")
        self.worker_token = worker_token
        self.session_cookie = session_cookie

    def request(self, path: str, payload: Any | None, *, worker: bool) -> Any:
        body = json.dumps(payload, separators=(",", ":")).encode() if payload is not None else None
        headers = {"Accept": "application/json"}
        if body is not None:
            headers["Content-Type"] = "application/json"
        if worker:
            headers["X-Worker-Token"] = self.worker_token
        else:
            headers["Cookie"] = self.session_cookie
        request = urllib.request.Request(
            self.base + path,
            data=body,
            headers=headers,
            method="POST" if body is not None else "GET",
        )
        try:
            with urllib.request.urlopen(request, timeout=10) as response:
                raw = response.read()
                return json.loads(raw) if raw else None
        except urllib.error.HTTPError as exc:
            detail = exc.read().decode(errors="replace")[:500]
            raise ApiFailure(f"{path} returned HTTP {exc.code}: {detail}") from exc
        except OSError as exc:
            raise ApiFailure(f"{path} failed: {exc}") from exc


@dataclass
class PhaseResult:
    name: str
    seconds: float
    generated: int
    accepted: int
    post_errors: int
    observed: int
    p95_seconds: float | None

    @property
    def accepted_per_30_seconds(self) -> float:
        return self.accepted * 30 / self.seconds

    def json(self) -> dict[str, Any]:
        return {
            "name": self.name,
            "seconds": round(self.seconds, 3),
            "generated": self.generated,
            "accepted": self.accepted,
            "accepted_per_30_seconds": round(self.accepted_per_30_seconds, 2),
            "post_errors": self.post_errors,
            "observed": self.observed,
            "p95_frame_to_browser_response_seconds": (
                round(self.p95_seconds, 3) if self.p95_seconds is not None else None
            ),
        }


def percentile_95(values: list[float]) -> float | None:
    if not values:
        return None
    ordered = sorted(values)
    return ordered[max(0, math.ceil(len(ordered) * 0.95) - 1)]


def frame(game: str, seq: int) -> dict[str, Any]:
    return {
        "game": game,
        "seq": seq,
        "grids": GRID,
        "state": "NOT_FINISHED",
        "levels_completed": 0,
        "baseline": None,
        "action": "ACTION1",
        "action_x": None,
        "action_y": None,
    }


def run_phase(
    client: Client,
    run_id: str,
    lease_token: str,
    name: str,
    duration: float,
    turns_per_30_seconds: float,
    seqs: dict[str, int],
    poll_browser: bool,
) -> PhaseResult:
    pending: queue.Queue[tuple[dict[str, Any], float]] = queue.Queue(maxsize=512)
    generated_at: dict[tuple[str, int], float] = {}
    accepted_keys: set[tuple[str, int]] = set()
    observed: set[tuple[str, int]] = set()
    latencies: list[float] = []
    lock = threading.Lock()
    post_errors = 0
    accepted = 0
    started = time.monotonic()
    end = started + duration
    per_game_interval = 30 * len(GAMES) / turns_per_30_seconds

    def produce(game: str, offset: float) -> None:
        next_turn = started + offset
        while next_turn < end:
            delay = next_turn - time.monotonic()
            if delay > 0:
                time.sleep(delay)
            seq = seqs[game]
            seqs[game] += 1
            created = time.monotonic()
            item = frame(game, seq)
            with lock:
                generated_at[(game, seq)] = created
            try:
                pending.put_nowait((item, created))
            except queue.Full:
                pass
            next_turn += per_game_interval

    def send() -> None:
        nonlocal accepted, post_errors
        while time.monotonic() < end or not pending.empty():
            try:
                first, _ = pending.get(timeout=0.25)
            except queue.Empty:
                continue
            batch = [first]
            while len(batch) < 64:
                try:
                    item, _ = pending.get_nowait()
                    batch.append(item)
                except queue.Empty:
                    break
            try:
                client.request(
                    "/api/worker/harness/arc/frames",
                    {"run_id": run_id, "lease_token": lease_token, "frames": batch},
                    worker=True,
                )
                with lock:
                    accepted += len(batch)
                    accepted_keys.update((item["game"], item["seq"]) for item in batch)
            except ApiFailure:
                with lock:
                    post_errors += 1

    def poll() -> None:
        next_poll = started
        poll_end = end + 2.5
        encoded_run = urllib.parse.quote(run_id, safe="")
        while time.monotonic() < poll_end:
            delay = next_poll - time.monotonic()
            if delay > 0:
                time.sleep(delay)
            try:
                payload = client.request(
                    f"/agentic-harness/arc/live?run={encoded_run}", None, worker=False
                )
                received = time.monotonic()
                latest = {
                    row.get("game"): int(row.get("seq", -1))
                    for row in (payload or {}).get("games", [])
                    if isinstance(row, dict)
                }
                with lock:
                    for key in accepted_keys:
                        if key not in observed and latest.get(key[0], -1) >= key[1]:
                            observed.add(key)
                            latencies.append(received - generated_at[key])
            except (ApiFailure, TypeError, ValueError):
                pass
            next_poll += 2.0

    threads = [threading.Thread(target=send, name=f"{name}-sender")]
    stagger = per_game_interval / len(GAMES)
    threads.extend(
        threading.Thread(target=produce, args=(game, index * stagger), name=f"{name}-{game}")
        for index, game in enumerate(GAMES)
    )
    if poll_browser:
        threads.append(threading.Thread(target=poll, name=f"{name}-browser"))
    for thread in threads:
        thread.start()
    for thread in threads:
        thread.join()
    elapsed = max(duration, time.monotonic() - started - (2.5 if poll_browser else 0))
    return PhaseResult(
        name=name,
        seconds=elapsed,
        generated=len(generated_at),
        accepted=accepted,
        post_errors=post_errors,
        observed=len(observed),
        p95_seconds=percentile_95(latencies),
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--academy", help="Academy HTTPS origin")
    parser.add_argument("--run-id")
    parser.add_argument("--duration", type=float, default=30.0)
    parser.add_argument("--turns-per-30-seconds", type=float, default=100.0)
    parser.add_argument("--seq-start", type=int, default=int(time.time()))
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()

    if args.self_test:
        assert len(GAMES) == 13
        assert len(frame("ls20", 1)["grids"]) == 4096
        assert percentile_95(list(range(1, 101))) == 95
        print("frame load-test contract ok")
        return 0

    worker_token = os.environ.get("WORKER_TOKEN", "")
    lease_token = os.environ.get("HARNESS_LEASE_TOKEN", "")
    session_cookie = os.environ.get("ACADEMY_SESSION_COOKIE", "")
    if not args.academy or not args.run_id:
        parser.error("--academy and --run-id are required outside --self-test")
    try:
        uuid.UUID(args.run_id)
        uuid.UUID(lease_token)
    except ValueError:
        parser.error("run id or HARNESS_LEASE_TOKEN is not a UUID")
    if len(worker_token) < 32 or not session_cookie.startswith("session="):
        parser.error("WORKER_TOKEN and ACADEMY_SESSION_COOKIE=session=... are required")
    if args.duration < 10 or args.turns_per_30_seconds <= 0:
        parser.error("duration must be >= 10 and target rate must be positive")

    client = Client(args.academy, worker_token, session_cookie)
    client.request(
        "/api/worker/harness/heartbeat",
        {"id": args.run_id, "lease_token": lease_token},
        worker=True,
    )
    stop_heartbeat = threading.Event()

    def heartbeat() -> None:
        while not stop_heartbeat.wait(25):
            client.request(
                "/api/worker/harness/heartbeat",
                {"id": args.run_id, "lease_token": lease_token},
                worker=True,
            )

    heartbeat_thread = threading.Thread(target=heartbeat, name="lease-heartbeat", daemon=True)
    heartbeat_thread.start()
    seqs = {game: args.seq_start for game in GAMES}
    try:
        baseline = run_phase(
            client, args.run_id, lease_token, "frames_without_browser_polls",
            args.duration, args.turns_per_30_seconds, seqs, False,
        )
        live = run_phase(
            client, args.run_id, lease_token, "frames_with_browser_polls",
            args.duration, args.turns_per_30_seconds, seqs, True,
        )
    finally:
        stop_heartbeat.set()
        heartbeat_thread.join(timeout=2)

    minimum_rate = args.turns_per_30_seconds * 0.95
    rate_ratio = (
        live.accepted_per_30_seconds / baseline.accepted_per_30_seconds
        if baseline.accepted_per_30_seconds else 0.0
    )
    passed = (
        baseline.post_errors == 0
        and live.post_errors == 0
        and baseline.accepted_per_30_seconds >= minimum_rate
        and live.accepted_per_30_seconds >= minimum_rate
        and rate_ratio >= 0.95
        and live.observed >= max(1, int(live.accepted * 0.9))
        and live.p95_seconds is not None
        and live.p95_seconds < 2.5
    )
    print(json.dumps({
        "ok": passed,
        "games": len(GAMES),
        "target_turns_per_30_seconds": args.turns_per_30_seconds,
        "browser_poll_seconds": 2,
        "live_to_baseline_throughput_ratio": round(rate_ratio, 3),
        "phases": [baseline.json(), live.json()],
    }, indent=2, sort_keys=True))
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
