-- Run this in Supabase SQL Editor (or psql against the connection string).
create extension if not exists pgcrypto;

create table if not exists users_exposure_academy (
  id uuid primary key default gen_random_uuid(),
  email text unique not null,
  display_name text not null,
  is_admin boolean not null default false,
  created_at timestamptz not null default now()
);

-- migrating existing deployments off username/password to email-based magic-link auth
alter table users_exposure_academy add column if not exists email text;
delete from users_exposure_academy where email is null; -- pre-launch, no real student data to preserve
alter table users_exposure_academy drop column if exists username;
alter table users_exposure_academy drop column if exists password_hash;
do $$ begin
  if not exists (select 1 from pg_constraint where conname = 'users_exposure_academy_email_key') then
    alter table users_exposure_academy add constraint users_exposure_academy_email_key unique (email);
  end if;
end $$;
alter table users_exposure_academy alter column email set not null;

-- onboarding profile. `nickname` is the ONLY name shown publicly (leaderboard);
-- display_name is the real name and stays admin-side. Null nickname = onboarding
-- not finished yet, which is what require_onboarded in main.rs gates on — that also
-- catches accounts the admin created by hand before this existed.
alter table users_exposure_academy add column if not exists nickname text;
alter table users_exposure_academy add column if not exists school text;
alter table users_exposure_academy add column if not exists grade text;
-- case-insensitive uniqueness, but only over the rows that have one
create unique index if not exists users_exposure_academy_nickname_lower_key
  on users_exposure_academy (lower(nickname)) where nickname is not null;

-- optional public profiles collected at the end of onboarding. Null = not provided
-- yet: the student skipped the step. The görev panosu (board) gates on both being
-- set, so a null here is what re-shows the "set up your profiles" prompt in-app.
alter table users_exposure_academy add column if not exists github_url text;
alter table users_exposure_academy add column if not exists linkedin_url text;

-- staff/intern accounts: they use the portal exactly like a student (videos, projects,
-- points) but must not show up in anything the students see — the leaderboard standings
-- and the teammate chips on the board. Unlike is_admin this grants nothing, so an intern
-- still onboards, still gets gated by require_onboarded, and still sees her own points.
alter table users_exposure_academy add column if not exists hidden_from_leaderboard boolean not null default false;

create table if not exists magic_links_exposure_academy (
  token text primary key,
  email text not null,
  expires_at timestamptz not null,
  used_at timestamptz,
  created_at timestamptz not null default now()
);

create table if not exists app_settings_exposure_academy (
  key text primary key,
  value text not null,
  updated_at timestamptz not null default now()
);
insert into app_settings_exposure_academy (key, value)
  values ('invite_code', encode(gen_random_bytes(6), 'hex'))
  on conflict (key) do nothing;

create table if not exists sessions_exposure_academy (
  token text primary key,
  user_id uuid not null references users_exposure_academy(id) on delete cascade,
  created_at timestamptz not null default now()
);

-- rolling 30-day sessions (extended on each request, see rolling_session in main.rs);
-- default backfills pre-existing rows instead of logging everyone out
alter table sessions_exposure_academy
  add column if not exists expires_at timestamptz not null default now() + interval '30 days';

create table if not exists videos_exposure_academy (
  id uuid primary key default gen_random_uuid(),
  youtube_id text not null,
  title text not null,
  level text not null check (level in ('PRESEED','SEED','SERIES_A')),
  position int not null default 0,
  created_at timestamptz not null default now()
);

create table if not exists watch_progress_exposure_academy (
  user_id uuid not null references users_exposure_academy(id) on delete cascade,
  video_id uuid not null references videos_exposure_academy(id) on delete cascade,
  seconds_watched real not null default 0,   -- accumulated watch time (rewatch counts)
  max_position real not null default 0,      -- furthest point reached, seconds
  duration real not null default 0,          -- video length, seconds (from player)
  updated_at timestamptz not null default now(),
  primary key (user_id, video_id)
);

create table if not exists tasks_exposure_academy (
  id uuid primary key default gen_random_uuid(),
  title text not null,
  description text not null,
  level text not null check (level in ('PRESEED','SEED','SERIES_A')),
  created_at timestamptz not null default now()
);

-- explicit ordering within a level (easiest -> hardest), admin-controlled via arrows.
alter table tasks_exposure_academy add column if not exists position int not null default 0;
-- one-time backfill of legacy rows: stable per-level order from creation time.
-- new rows get an explicit position on insert (admin_task), so they never stay 0.
update tasks_exposure_academy t set position = sub.rn
  from (select id, row_number() over (partition by level order by created_at) as rn
        from tasks_exposure_academy) sub
  where t.id = sub.id and t.position = 0;

create table if not exists submissions_exposure_academy (
  id uuid primary key default gen_random_uuid(),
  task_id uuid not null references tasks_exposure_academy(id) on delete cascade,
  user_id uuid not null references users_exposure_academy(id) on delete cascade,
  repo_url text not null,
  status text not null default 'pending' check (status in ('pending','reviewing','passed','failed')),
  feedback text,
  demo_video_url text,
  created_at timestamptz not null default now()
);

-- example project URL shown as a live preview on the task card
alter table tasks_exposure_academy add column if not exists example_url text;
-- true = site allows iframe embedding (live preview); false/null = show cached screenshot instead
alter table tasks_exposure_academy add column if not exists example_embeddable boolean;
-- plan.md content submitted with the repo; null on pre-feature submissions
alter table submissions_exposure_academy add column if not exists plan_md text;

-- admin-set point value for this submission, overriding the task's level default
-- (see PTS_PROJECT_L* in model.rs). Null = use the level default, which is the
-- normal case; the admin fills this in only to score a project by hand.
alter table submissions_exposure_academy add column if not exists points_override int;
alter table submissions_exposure_academy drop constraint if exists submissions_points_override_nonneg;
alter table submissions_exposure_academy
  add constraint submissions_points_override_nonneg check (points_override is null or points_override >= 0);

-- cached hero screenshots for example URLs that block iframe embedding, keyed by
-- URL so tasks sharing a URL share one image; fetched once from Microlink then served from here
create table if not exists screenshot_cache_exposure_academy (
  url text primary key,
  image bytea not null,
  content_type text not null default 'image/png',
  fetched_at timestamptz not null default now()
);

-- a student flipping "Bunu yapmak isterim" on a task; drives the teammate list on /board
create table if not exists task_interest_exposure_academy (
  task_id uuid not null references tasks_exposure_academy(id) on delete cascade,
  user_id uuid not null references users_exposure_academy(id) on delete cascade,
  created_at timestamptz not null default now(),
  primary key (task_id, user_id)
);

-- ---- Agentic Harness ----
-- Teams are admin-managed for now (interim until team onboarding exists).
create table if not exists harness_teams_exposure_academy (
  id uuid primary key default gen_random_uuid(),
  name text not null,
  created_at timestamptz not null default now()
);
create unique index if not exists harness_teams_name_lower_key
  on harness_teams_exposure_academy (lower(name));

-- user_id as PK = a student is on at most one team, which is what makes
-- "any member submits for the team" unambiguous.
create table if not exists harness_team_members_exposure_academy (
  user_id uuid primary key references users_exposure_academy(id) on delete cascade,
  team_id uuid not null references harness_teams_exposure_academy(id) on delete cascade,
  created_at timestamptz not null default now()
);
create index if not exists harness_team_members_team_idx
  on harness_team_members_exposure_academy (team_id);

-- One submission = one run = scores for all three benchmarks. The worker claims a
-- queued run, walks it through the stages, and posts the scores (see /api/worker/harness/*).
create table if not exists harness_runs_exposure_academy (
  id uuid primary key default gen_random_uuid(),
  team_id uuid not null references harness_teams_exposure_academy(id) on delete cascade,
  submitted_by uuid references users_exposure_academy(id) on delete set null,
  repo_url text not null,
  commit_sha text,                          -- resolved by the worker after clone
  stage text not null default 'queued' check (stage in
    ('queued','cloning','building','arc_agi_3','frontier_bench','ram_bench','done','failed')),
  score_arc real,                           -- higher = better
  score_frontier real,                      -- higher = better
  ram_1session_mb real,                     -- PSS MB at 1 active session, shown next to the ranking column
  ram_10session_mb real,                    -- PSS MB at 10 active sessions, ranking column, lower = better
  error_log text,                           -- failure output shown in the history tab
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now()
);
create index if not exists harness_runs_team_idx
  on harness_runs_exposure_academy (team_id, created_at desc);
-- double-submit guard: at most ONE in-flight run per team, enforced by the DB so a
-- submit race can't slip past the handler's friendly pre-check.
create unique index if not exists harness_runs_one_active_per_team
  on harness_runs_exposure_academy (team_id) where stage not in ('done','failed');
-- live progress blob written by the runner mid-stage (JSON: done/total/score/detail),
-- shown under the active stepper step; cleared when the run reaches a terminal state.
alter table harness_runs_exposure_academy add column if not exists progress text;
