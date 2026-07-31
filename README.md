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
| `/documents` | veli onay formları — student uploads the signed consent forms (see below) |
| `/documents/file/{id}` | download one uploaded document (its owner, or any admin) |
| `/admin` | add student/video/task, watch statistics, review submissions, collect consent forms |
| `/admin/documents.zip` | every consent form on file, as one archive |
| `/api/progress` | watch-time heartbeat (student session) |
| `/api/worker/*` | Phase 3 pipeline API (see below) |

Watch data per (student, video): `seconds_watched` (accumulated, rewatches count),
`max_position` (furthest point), `duration`. Progress % = max_position/duration; ≥90% counts as completed.

## Veli onay formları (parental consent)

The students are under 18, so a parent/legal guardian has to sign for them. `/documents`
is where the signed forms are uploaded — one card per form in `CONSENT_DOCS` (`model.rs`),
which is also where the titles and the deadline string live.

- **The blank form is on the card**: a "Formu indir" button (the Drive share link rewritten
  to `uc?export=download`, so it downloads rather than opening a preview) plus a
  "tarayıcıda aç" link to the original — one of the two fails on somebody's phone every
  time. The URL comes from `CONSENT_DOCS`, overridable per form on `/admin`.
- **One row per FILE, not per form.** A form is usually photographed a page at a time, so
  a student uploads several files against the same `kind` and they accumulate (up to
  `CONSENT_MAX_FILES`). The file picker takes several at once.
- **Formats** are decided by the file's own bytes (`sniff_document`), not by what the
  browser claims: PDF, JPEG, PNG, GIF, WebP, HEIC/HEIF (iPhone's default) and Word
  DOC/DOCX/ODT. The stored name gets the extension the bytes actually are, so the admin
  can open the file by double-clicking it. Everything is served back as
  `Content-Disposition: attachment` with `nosniff` — student bytes never render in our origin.
- **Bytes live in Postgres** (`consent_docs_exposure_academy`), same reasoning as the
  schedule image: a redeploy must not lose a document, and this is the one thing you can't
  ask a family to re-do at short notice.
- **Who can see a document**: the student who uploaded it, and admins. Nobody else — a
  signed form carries a minor's details and a parent's signature.

### Collecting them

`/admin` → **Veli onay formları** is the whole collection view: a student × form grid where
every uploaded file is a download link, per-form counters, and **⬇ Tüm belgeler (.zip)**.
The ZIP is a folder per form, a folder per student inside it, files numbered in upload
order — so the QNBEYOND folder is exactly what gets handed to QNBEYOND — plus
`_EKSIKLER.txt` at the root, a `[X]`/`[ ]` checklist of who has and hasn't uploaded.

### Opening and closing a form

A form whose document isn't ready to hand out yet is **closed**: students see the card
blurred behind a "yakında" overlay (no `<input>` is rendered at all) and the server refuses
uploads and deletes for it. Paribu ships closed — `CONSENT_LOCKED_BY_DEFAULT` — because its
form didn't exist yet. Open it with **Yüklemeye aç** on `/admin` once it does, and paste its link in the field
beside it. Both live in `app_settings_exposure_academy` (`consent_lock_<kind>`,
`consent_url_<kind>`), so it's two form fields, not a deploy.

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
