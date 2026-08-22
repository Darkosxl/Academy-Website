-- Exposure Academy Verified. Standalone site (platform.exposureai.org), standalone tables,
-- same Postgres/Supabase project as the main Academy so a student's eligibility can be
-- checked with a plain read of users_exposure_academy — no sync job, no second source of
-- truth for "who's a student." Nothing here is written to or read from any
-- *_exposure_academy table; the coupling is strictly one read query at login (see main.rs).
create extension if not exists pgcrypto;

-- One identity table for all three roles (admin/submitter/student) rather than three
-- separate tables: they share the exact same auth mechanism (email magic link, session
-- cookie), and "what can this person do" is entirely a matter of `role` plus, for
-- submitters, which case(s) reference them — not a different login system per role.
create table if not exists verified_users (
  id uuid primary key default gen_random_uuid(),
  email text unique not null,
  display_name text not null,
  role text not null check (role in ('admin','submitter','student')),
  linkedin_url text,          -- submitters only; shown on their case card
  created_at timestamptz not null default now()
);

create table if not exists verified_magic_links (
  token text primary key,
  email text not null,
  expires_at timestamptz not null,
  used_at timestamptz,
  created_at timestamptz not null default now()
);

create table if not exists verified_sessions (
  token text primary key,
  user_id uuid not null references verified_users(id) on delete cascade,
  created_at timestamptz not null default now(),
  expires_at timestamptz not null default now() + interval '30 days'
);

create table if not exists verified_cases (
  id uuid primary key default gen_random_uuid(),
  title text not null,
  description text not null,
  submitter_id uuid not null references verified_users(id) on delete restrict,
  created_by uuid not null references verified_users(id) on delete restrict,
  hidden boolean not null default false,
  created_at timestamptz not null default now()
);
create index if not exists verified_cases_hidden_idx on verified_cases (hidden, created_at desc);
create index if not exists verified_cases_submitter_idx on verified_cases (submitter_id);

create table if not exists verified_submissions (
  id uuid primary key default gen_random_uuid(),
  case_id uuid not null references verified_cases(id) on delete cascade,
  student_id uuid not null references verified_users(id) on delete cascade,
  note text,
  created_at timestamptz not null default now()
);
create index if not exists verified_submissions_case_idx on verified_submissions (case_id, created_at desc);
create index if not exists verified_submissions_student_idx on verified_submissions (case_id, student_id, created_at desc);

-- up to 5 per submission, enforced in the handler (not a DB constraint — counting rows
-- across a multi-statement insert isn't worth a trigger for a cap this small).
create table if not exists verified_submission_links (
  id uuid primary key default gen_random_uuid(),
  submission_id uuid not null references verified_submissions(id) on delete cascade,
  url text not null,
  position int not null default 0
);
create index if not exists verified_submission_links_sub_idx on verified_submission_links (submission_id, position);

-- up to 10 per submission, same enforced-in-handler cap as links. The bytes themselves
-- live in Supabase Storage (storage_key), not here — see db-capacity-guard: this account
-- has already taken a Postgres disk offline once from uncapped writes, and "up to 10
-- files, anything they have" is exactly the shape that repeats that if it lands in bytea.
create table if not exists verified_submission_files (
  id uuid primary key default gen_random_uuid(),
  submission_id uuid not null references verified_submissions(id) on delete cascade,
  filename text not null,
  content_type text not null,
  storage_key text not null,
  size_bytes bigint not null,
  position int not null default 0
);
create index if not exists verified_submission_files_sub_idx on verified_submission_files (submission_id, position);

-- Public per-case Q&A (every student viewing a case sees the full thread). The answer is
-- embedded directly on the question row (null until answered) rather than a separate
-- answers table — mirrors submissions_exposure_academy.feedback in the main Academy schema.
-- answered_by may be the case's submitter OR an admin (fallback if the submitter is slow).
create table if not exists verified_questions (
  id uuid primary key default gen_random_uuid(),
  case_id uuid not null references verified_cases(id) on delete cascade,
  student_id uuid not null references verified_users(id) on delete cascade,
  body text not null,
  created_at timestamptz not null default now(),
  answer_body text,
  answered_by uuid references verified_users(id),
  answered_at timestamptz
);
create index if not exists verified_questions_case_idx on verified_questions (case_id, created_at);
