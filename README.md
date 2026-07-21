# Exposure Academy

Video learning portal for high school students. Rust (Axum) + Supabase (Postgres).
Students log in, watch YouTube-embedded lessons split into three levels — shown as
**Seviye 1 / 2 / 3**, stored as `PRESEED` / `SEED` / `SERIES_A` (see `LEVELS` in `html.rs`) —
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
account. `nickname` is the *only* name shown on the leaderboard — `display_name` (the real name)
stays admin-side, and the form says so. A null `nickname` means onboarding never finished, so
`require_onboarded` in `main.rs` redirects those students to `/profile` until they pick one;
that also catches accounts you created by hand from `/admin`. Admins are exempt from that gate.

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

## Notes

- Turkish UI strings were produced via the Google Translate API, not hand-written.
- Some public YouTube videos disallow embedding ("Video kullanılamıyor") — your own
  unlisted uploads embed fine.
- `demos/` contains the original static style demos that the design came from.
