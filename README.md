# Exposure Academy

Video learning portal for high school students. Rust (Axum) + Supabase (Postgres).
Students log in, watch YouTube-embedded lessons split into three levels — shown as
**Beginner / Intermediate / Advanced**, stored as `PRESEED` / `SEED` / `SERIES_A` (see `LEVELS` in `html.rs`) —
(labels only — everyone can watch everything), and submit projects on the task board.
Watch time is tracked per student per video.

## Stack

- **Server**: Rust, Axum, server-rendered HTML (no JS framework)
- **DB**: Supabase Postgres via `sqlx` (direct connection string, no Supabase SDK)
- **Videos**: YouTube unlisted embeds; tracking via YouTube IFrame Player API
  (`static/tracker.js` heartbeats every 10s while playing → `POST /api/progress`)
- **Auth**: passwordless — session cookie + emailed magic link (Resend). Students self-register
  at `/join/:code` with the invite code baked into the link; admin can also add them by hand.

## Onboarding

`/admin` → **Davet bağlantısı** gives you the link to paste in the WhatsApp group. A student
fills in name / email / nickname / school / grade, gets a magic link, and clicking it opens the
account. The leaderboard shows **both** names — real name first, nickname in parentheses
(`Onur Çelik (onur_maker)`) — and the onboarding form says so. The board's teammate chips
still show the nickname alone. A null `nickname` means onboarding never finished, so
`require_onboarded` in `main.rs` redirects those students to `/profile` until they pick one;
that also catches accounts you created by hand from `/admin`. Admins are exempt from that gate.

### Hidden (intern / staff) accounts

`hidden_from_leaderboard` on the user row is for people who follow the program to learn —
an intern going through the videos and projects — without competing with the students.
The account is a completely normal student otherwise (it onboards, earns points, submits
projects, and sees its own total on Ana Sayfa); it is only left out of everything the
students see: the `/leaderboard` standings and the teammate chips on `/board`. Ranks are
computed after hidden rows are dropped, so a hidden account never pushes a student down a
place. Do **not** use `is_admin` for this — that hands over the admin panel and skips onboarding.

Two ways to set it, both in `/admin` → **Öğrenciler**: tick *Puan tablosunda gizle* when adding
the person by hand (do this **before** they open the invite link — `join_post` does
`on conflict (email) do nothing`, so the hidden row survives onboarding and they are never
visible for even one page load), or hit *Puan tablosunda gizle* on their row afterwards.

## Setup

1. Create a Supabase project → Settings → Database → copy the **connection string** (URI).
2. `cp .env.example .env`, fill it in:
   - `DATABASE_URL` — Supabase connection string
   - `ADMIN_USERNAME` / `ADMIN_PASSWORD` — seeded on first boot
   - `WORKER_TOKEN` — shared secret for the Phase 3 worker API
3. `cargo run` — schema (`migrations/001_init.sql`) is applied automatically, idempotent.
4. Log in as admin → **Yönetici paneli** → add students, videos (paste any YouTube URL or ID), tasks.

## What's where

| Route | What |
|---|---|
| `/` | public landing |
| `/join/:code` | onboarding — the link you paste in the WhatsApp group (code is in the URL) |
| `/profile` | student edits name / nickname / school / grade; reached from the sidebar chip |
| `/app` | video grid, level chips |
| `/watch/:id` | player + level playlist, resumes from last position |
| `/board` | task board: tasks per level, GitHub repo submission, status + feedback + demo video |
| `/admin` | add student/video/task, watch statistics, review submissions |
| `/api/progress` | watch-time heartbeat (student session) |
| `/api/worker/*` | Phase 3 pipeline API (see below) |

Watch data per (student, video): `seconds_watched` (accumulated, rewatches count),
`max_position` (furthest point), `duration`. Progress % = max_position/duration; ≥90% counts as completed.

## Phase 3 — auto-eval pipeline (NOT BUILT YET)

Goal: submissions get automatically evaluated by Claude Code on the admin's machine,
and passing projects get a recorded demo video published on the site.

The server side is already done — two authenticated endpoints (`X-Worker-Token` header):

- `GET /api/worker/pending` — atomically claims up to 5 `pending` submissions
  (flips them to `reviewing`), returns `[{id, repo_url, task_title}]`.
- `POST /api/worker/result` — `{id, status: "passed"|"failed", feedback, demo_video_url}`.

To build (a script/daemon in `worker/`, runs on the admin's machine — never on the server):

1. Poll `GET /api/worker/pending` every minute.
2. For each submission: `git clone` the repo into a sandbox dir.
3. Run **Claude Code** (headless, e.g. `claude -p`) against the clone:
   install deps, start the project, judge whether it works and meets the task description.
   Output: verdict + student-facing feedback in Turkish (what is wrong, how to fix).
4. Verdict failed → `POST /api/worker/result` with `status: "failed"` and the feedback.
5. Verdict passed → drive the running project with **Playwright** and record a short demo video
   (`playwright` context `recordVideo`), upload it (YouTube unlisted, same as lessons),
   then `POST /api/worker/result` with `status: "passed"`, feedback, and `demo_video_url`.
6. Board shows status/feedback/demo automatically — no server changes needed.

Safety notes for the worker: run student code in a container (podman/docker) with no
network egress except package registries, memory/time limits, throwaway filesystem.

### Agentic Harness runner API

Server side is done; the real runner lives at `worker/runner.py` (see its header for
one-time host setup: ARC starter checkout + `make setup` and the frontier-bench dataset
under `worker/cache/`, `uv tool install harbor`, Docker). `SMOKE_MODE=1` caps the run
to 2 frontier tasks and 2 short ARC games for pipeline checks; `--once` processes a
single run and exits. Same auth (`X-Worker-Token`), same pull model. One team submission = one run = scores for all three boards
(ARC-AGI-3, Frontier-bench, RAM-bench). Stages are forward-only; every update is
guarded on the expected current stage — on a `409` drop the run and move on.

- `GET /api/worker/harness/pending` — atomically claims one `queued` run
  (flips it to `cloning`), returns `[{id, repo_url}]` (0 or 1 element).
- `POST /api/worker/harness/stage` — `{id, stage, commit_sha?}` reports a transition:
  `building` → `arc_agi_3` → `frontier_bench` → `ram_bench`. The `building` report
  **must** carry `commit_sha` (7–40 hex chars, the commit that was checked out) —
  it's what the student history tab links to. `204` ok, `400` bad stage/sha, `409` stale.
- `POST /api/worker/harness/result` — terminal:
  `{id, status: "done"|"failed", score_arc, score_frontier, ram_1session_mb,
  ram_10session_mb, error_log}`. `done` requires all four scores (RAM values are PSS MB
  measured during active sessions — 1 and 10 concurrent; lower is better). `failed`
  stores only `error_log` (shown to the team in the history tab, so keep it readable).
- `POST /api/worker/harness/progress` — `{id, progress}` repeatable mid-stage report;
  `progress` is a JSON string `{"done", "total", "score", "detail"}` rendered live
  under the student's stepper. Cleared automatically on the terminal result.
  `204` ok, `400` too long (>2000 bytes), `409` run already terminal.

The student page polls `/agentic-harness/status` every 5s and moves a stepper through
the stages, so report each transition as it happens rather than batching at the end.
A run stuck non-terminal (runner died after claiming) blocks the team's resubmits;
the admin panel's "Başarısız say" button is the escape hatch.

## Notes

- Turkish UI strings were produced via the Google Translate API, not hand-written.
- Some public YouTube videos disallow embedding ("Video kullanılamıyor") — your own
  unlisted uploads embed fine.
- `demos/` contains the original static style demos that the design came from.
