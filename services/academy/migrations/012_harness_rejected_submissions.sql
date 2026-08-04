-- Rejected harness submissions. Until now a bad repo link produced a 400 and vanished, so
-- "what did the student actually paste?" had no answer — the submit handler wrote nothing and
-- the service has no logging. That question is the only reason this table exists: nothing
-- downstream consumes it, no run is ever created from it, and /admin is its sole reader.
--
-- raw_input is unvalidated student text, stored truncated (HARNESS_RAW_INPUT_MAX in
-- harness.rs) and rendered only through esc(). It is never handed to the worker.
--
-- Both FKs are `on delete set null` rather than cascade: a deleted account is exactly when an
-- old rejection is still worth reading, and the row carries nothing about the student beyond
-- the id. main.rs sweeps rows older than 30 days on boot, the same opportunistic pattern as
-- magic links and sessions.
create table if not exists harness_rejected_submissions_exposure_academy (
  id uuid primary key default gen_random_uuid(),
  user_id uuid references users_exposure_academy(id) on delete set null,
  team_id uuid references harness_teams_exposure_academy(id) on delete set null,
  raw_input text not null,
  reason text not null,          -- source_error_slug: 'owner_only', 'non_ascii', 'not_github', ...
  benchmark_kind text,
  created_at timestamptz not null default now()
);
-- /admin reads "the most recent 50, newest first" and nothing else; the boot sweep walks the
-- same index from the other end.
create index if not exists harness_rejected_submissions_created_idx
  on harness_rejected_submissions_exposure_academy (created_at desc);
