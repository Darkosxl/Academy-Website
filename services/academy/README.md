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
3. `cargo run -p academy` from the repository root — schema migrations are applied
   automatically and idempotently.
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
| `/agentic-harness/arc/live` | ARC board feed as JSON — the team's own run, live or replayed (see below) |
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

### AI Monopoly tournament API

The Monopoly controller is a separate Academy-host service. It validates public GitHub
submissions, writes hash-addressed archives to `MONOPOLY_ARTIFACT_DIR`, and manages five named
CPU-only Colab workers. The Academy process only persists tournament state and serves
authenticated APIs; it never provisions a worker from an HTTP handler.

Worker routes require `X-Worker-Token`. Claims use 90-second renewable leases, and stale lease
heartbeats, events, or results return `409`.

- `POST /api/worker/rl-monopoly/submissions/claim`, `/heartbeat`, and `/result` validate and
  auto-approve pinned submissions.
- `POST /api/worker/rl-monopoly/claim`, `/heartbeat`, `/events`, and `/result` execute frozen
  tournament games with bounded event batches.
- `GET /api/worker/rl-monopoly/artifact/{sha256}` serves validated archives only to workers.
- `POST /api/worker/rl-monopoly/resource` records worker hardware and preflight failures.
- `GET /api/worker/rl-monopoly/demand` exposes credentialed fleet demand without team code.

Logged-in Academy users can view live matches, replays, standings, and the complete authenticated
tournament JSON export. Runtime log tails are filtered to the submitting team and administrators.
See `services/monopoly-worker/README.md` for validation, isolation, and fleet rollout commands.

### Agentic Harness runner API

Production execution lives in `services/benchmark-node`: a credential-owning Rust
controller and a separate restricted Rust executor. Python is retained only for benchmark
SDK integration. The old Python polling mode remains in `adapters/runner.py` as the rollback
path. EC2 recreates pinned caches and virtual environments under
`/var/lib/exposure-benchmark`; repository-local `.venv` directories are never copied.

All worker routes are `POST`, require `X-Worker-Token`, and every mutation after a claim
also carries that claim's `lease_token`. A missing/incorrect worker token returns `401`; an
expired or reclaimed lease returns `409`; transient Supabase failures return `503`.

- `/api/worker/harness/claim` — atomically claims one run with `FOR UPDATE SKIP LOCKED`.
- `/api/worker/harness/capacity` — returns global queued/active slot demand for the
  private Auto Scaling controllers; it contains no team or submission details.
- `/api/worker/harness/heartbeat` — renews the 90-second lease every five seconds and
  makes a team cancellation visible to the controller within one polling interval.
- `/api/worker/harness/stage` — records the checked-out commit and enters `running`.
- `/api/worker/harness/progress` — updates one of `arc`, `frontier`, or `ram`.
- `/api/worker/harness/result` — stores `done`, `partial`, `failed`, or `infra_failed`;
  repeating the same terminal write is idempotent.
- `/api/worker/harness/arc/frames` — appends a batch of at most 64 validated ARC frames.
- `/api/worker/harness/kaggle/claim` and `/api/worker/harness/kaggle/result` — run the
  explicit official-submit/poll workflow without changing local scores.

ARC frame writes are best effort. `grids` is one to sixteen newline-separated 4096-character
hex grids; the Rust controller uses a bounded non-blocking queue and drops visualization
traffic rather than delaying scoring. Result writes have a 30-second post-deadline grace
while still requiring the active lease.

Students watch that feed at `GET /agentic-harness/arc/live` (session cookie, all params
optional: `?run=&game=&seq=`). Focused frames are cursor-paginated in pages of 200. A run
identifier is always resolved through the caller's team membership, so guessing another
team's UUID returns no run. The same rows serve live viewing and finished-run replay.

The student page polls `/agentic-harness/status` every 2s (`static/harness.js`) and moves
a stepper through the stages, so report each transition as it happens rather than batching
at the end.
A team member can `POST /agentic-harness/stop`; Academy marks the run `cancelled` and
revokes its lease, so the controller tears down the executor without exposing worker access.
A run stuck non-terminal (runner died after claiming) blocks the team's resubmits;
the admin panel's "Başarısız say" button is the escape hatch.

See `services/benchmark-node/README.md` for build, EC2 installation, monitoring, canary,
rollout, and rollback instructions.

## Notes

- Turkish UI strings were produced via the Google Translate API, not hand-written.
- Some public YouTube videos disallow embedding ("Video kullanılamıyor") — your own
  unlisted uploads embed fine.
- `demos/` contains the original static style demos that the design came from.
