-- Frozen AI Monopoly tournaments (ppo-plus-v2).
--
-- 018 has never shipped. It deliberately replaces the legacy negotiation-game tables
-- created by 001 and the short-lived singleton board-game draft. The Academy replays its
-- SQL files at boot, so the reused tournament table is dropped only when it still has the
-- legacy `rounds_total` shape. A frozen tournament is never dropped on a later boot.

drop table if exists monopoly_invites_exposure_academy cascade;
drop table if exists monopoly_notes_exposure_academy cascade;
drop table if exists monopoly_transactions_exposure_academy cascade;
drop table if exists monopoly_messages_exposure_academy cascade;
drop table if exists monopoly_matches_exposure_academy cascade;
drop table if exists monopoly_players_exposure_academy cascade;
drop table if exists monopoly_entries_exposure_academy cascade;

do $$
begin
  if exists (
    select 1
    from information_schema.columns
    where table_schema = current_schema()
      and table_name = 'monopoly_tournaments_exposure_academy'
      and column_name = 'rounds_total'
  ) then
    drop table monopoly_tournaments_exposure_academy cascade;
  end if;
end
$$;

create table if not exists monopoly_artifacts_exposure_academy (
  sha256 text primary key check (sha256 ~ '^[0-9a-f]{64}$'),
  size_bytes bigint not null check (size_bytes between 1 and 2147483648),
  created_at timestamptz not null default now(),
  last_used_at timestamptz not null default now()
);

-- One mutable submission per team. A resubmit increments generation and clears every
-- validation result. Tournament entries copy the accepted fields, so this row can change
-- without changing a frozen tournament.
create table if not exists monopoly_submissions_exposure_academy (
  id uuid primary key default gen_random_uuid(),
  team_id uuid not null unique references monopoly_teams_exposure_academy(id) on delete cascade,
  generation int not null default 1 check (generation > 0),
  submitted_by uuid references users_exposure_academy(id) on delete set null,
  repo_url text not null,
  agent_path text not null default 'agent.py',
  status text not null default 'pending'
    check (status in ('pending','validating','approved','failed','rejected','disabled')),
  commit_sha text check (commit_sha is null or commit_sha ~ '^[0-9a-f]{40}$'),
  repo_size_bytes bigint check (repo_size_bytes is null or repo_size_bytes between 0 and 262144000),
  artifact_sha256 text references monopoly_artifacts_exposure_academy(sha256),
  dependency_lock text,
  validation_log text,
  validation_lease_id uuid,
  validation_worker_id text,
  validation_lease_expires_at timestamptz,
  approved_at timestamptz,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now(),
  check ((status = 'approved') =
    (commit_sha is not null and artifact_sha256 is not null and dependency_lock is not null)),
  check ((status = 'validating') =
    (validation_lease_id is not null and validation_lease_expires_at is not null))
);
create index if not exists monopoly_submissions_status_idx
  on monopoly_submissions_exposure_academy (status, updated_at);

create table if not exists monopoly_tournaments_exposure_academy (
  id uuid primary key default gen_random_uuid(),
  status text not null default 'active'
    check (status in ('active','completed','partial','cancelled')),
  seed bigint not null,
  ruleset_version text not null default 'ppo-plus-v2' check (ruleset_version = 'ppo-plus-v2'),
  schema_version text not null default 'exposure-agent-v1',
  total_games int not null check (total_games > 0),
  completed_games int not null default 0 check (completed_games >= 0),
  partial_reason text,
  created_by uuid references users_exposure_academy(id) on delete set null,
  created_at timestamptz not null default now(),
  started_at timestamptz not null default now(),
  completed_at timestamptz
);
create unique index if not exists monopoly_one_active_tournament
  on monopoly_tournaments_exposure_academy ((true)) where status = 'active';

create table if not exists monopoly_tournament_entries_exposure_academy (
  id uuid primary key default gen_random_uuid(),
  tournament_id uuid not null references monopoly_tournaments_exposure_academy(id) on delete cascade,
  team_id uuid not null references monopoly_teams_exposure_academy(id) on delete restrict,
  submission_id uuid references monopoly_submissions_exposure_academy(id) on delete set null,
  submission_generation int not null check (submission_generation > 0),
  team_name text not null,
  repo_url text not null,
  commit_sha text not null check (commit_sha ~ '^[0-9a-f]{40}$'),
  agent_path text not null,
  artifact_sha256 text not null references monopoly_artifacts_exposure_academy(sha256),
  dependency_lock text not null,
  created_at timestamptz not null default now(),
  unique (tournament_id, team_id)
);
create index if not exists monopoly_tournament_entries_artifact_idx
  on monopoly_tournament_entries_exposure_academy (artifact_sha256);

create table if not exists monopoly_games_exposure_academy (
  id uuid primary key default gen_random_uuid(),
  tournament_id uuid not null references monopoly_tournaments_exposure_academy(id) on delete cascade,
  game_no int not null check (game_no > 0),
  seed bigint not null,
  status text not null default 'queued'
    check (status in ('queued','leased','done','failed','cancelled')),
  attempt_count int not null default 0 check (attempt_count between 0 and 3),
  lease_id uuid,
  lease_worker_id text,
  lease_expires_at timestamptz,
  round int not null default 0 check (round between 0 and 200),
  action_count int not null default 0 check (action_count between 0 and 50000),
  last_event_seq int not null default 0 check (last_event_seq between 0 and 50000),
  winner_seat smallint check (winner_seat between 0 and 3),
  winner_entry_id uuid references monopoly_tournament_entries_exposure_academy(id) on delete set null,
  end_reason text check (end_reason is null or end_reason in
    ('normal','ten_minute','action_ceiling','infra_exhausted')),
  duration_us bigint check (duration_us is null or duration_us >= 0),
  error_log text,
  created_at timestamptz not null default now(),
  started_at timestamptz,
  finished_at timestamptz,
  updated_at timestamptz not null default now(),
  unique (tournament_id, game_no),
  check ((status = 'leased') =
    (lease_id is not null and lease_worker_id is not null and lease_expires_at is not null))
);
create index if not exists monopoly_games_claim_idx
  on monopoly_games_exposure_academy (status, created_at);

create table if not exists monopoly_game_seats_exposure_academy (
  game_id uuid not null references monopoly_games_exposure_academy(id) on delete cascade,
  seat smallint not null check (seat between 0 and 3),
  entry_id uuid references monopoly_tournament_entries_exposure_academy(id) on delete cascade,
  bot_key text,
  label text not null,
  final_cash double precision,
  final_net_worth double precision,
  strikes int not null default 0 check (strikes between 0 and 3),
  decision_count int not null default 0 check (decision_count >= 0),
  decision_total_us bigint not null default 0 check (decision_total_us >= 0),
  decision_min_us bigint check (decision_min_us is null or decision_min_us between 0 and 2000000),
  decision_max_us bigint check (decision_max_us is null or decision_max_us between 0 and 2000000),
  disqualified boolean not null default false,
  replacement_bot text,
  runtime_log text,
  final_snapshot jsonb,
  primary key (game_id, seat),
  check ((entry_id is null) <> (bot_key is null))
);
create index if not exists monopoly_game_seats_entry_idx
  on monopoly_game_seats_exposure_academy (entry_id);

create table if not exists monopoly_game_attempts_exposure_academy (
  game_id uuid not null references monopoly_games_exposure_academy(id) on delete cascade,
  attempt_no int not null check (attempt_no between 1 and 3),
  lease_id uuid not null unique,
  worker_id text not null,
  status text not null default 'leased'
    check (status in ('leased','completed','infra_failed','expired','stale','cancelled')),
  claimed_at timestamptz not null default now(),
  last_heartbeat_at timestamptz not null default now(),
  finished_at timestamptz,
  error_log text,
  primary key (game_id, attempt_no)
);

-- seq is scoped to an attempt. Failed attempts remain auditable without being mixed into
-- the successful replay; readers select the game's final attempt_count.
create table if not exists monopoly_events_exposure_academy (
  game_id uuid not null references monopoly_games_exposure_academy(id) on delete cascade,
  attempt_no int not null,
  seq int not null check (seq > 0),
  acted_player smallint not null check (acted_player between 0 and 3),
  round int not null check (round between 0 and 200),
  phase text not null,
  action_idx int not null check (action_idx between 0 and 2957),
  action_desc text not null,
  decision_us bigint check (decision_us is null or decision_us between 0 and 2000000),
  strike boolean not null default false,
  info jsonb not null default '{}'::jsonb,
  snapshot jsonb not null,
  created_at timestamptz not null default now(),
  primary key (game_id, attempt_no, seq),
  foreign key (game_id, attempt_no)
    references monopoly_game_attempts_exposure_academy(game_id, attempt_no) on delete cascade
);

create table if not exists monopoly_workers_exposure_academy (
  worker_id text primary key,
  kind text not null check (kind in ('colab','host','validation','other')),
  session_name text,
  status text not null check (status in ('preflight','ready','busy','stopping','stopped','rejected','error')),
  ram_bytes bigint check (ram_bytes is null or ram_bytes >= 0),
  effective_vcpus double precision check (effective_vcpus is null or effective_vcpus >= 0),
  machine_shape text,
  preflight_reason text,
  current_game_id uuid references monopoly_games_exposure_academy(id) on delete set null,
  last_seen_at timestamptz not null default now(),
  updated_at timestamptz not null default now()
);
