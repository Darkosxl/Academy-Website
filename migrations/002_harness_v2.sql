-- Agentic Harness v2: versioned, lease-owned runs with independent benchmark state.
-- Idempotent because this project applies migrations at startup rather than tracking
-- a separate migration ledger.

alter table harness_runs_exposure_academy
  drop constraint if exists harness_runs_exposure_academy_stage_check;

alter table harness_runs_exposure_academy
  add column if not exists benchmark_version text not null default 'harness-2026-sprint-v1',
  add column if not exists benchmark_state jsonb not null default
    '{"arc":{"status":"pending"},"frontier":{"status":"pending"},"ram":{"status":"pending"}}'::jsonb,
  add column if not exists bedrock_profile text,
  add column if not exists bedrock_profile_fingerprint text,
  add column if not exists deadline_at timestamptz,
  add column if not exists lease_token uuid,
  add column if not exists lease_expires_at timestamptz,
  add column if not exists worker_heartbeat_at timestamptz,
  add column if not exists claim_attempts integer not null default 0;

-- Preserve useful legacy scores while converting the old all-or-nothing state.
update harness_runs_exposure_academy
set benchmark_state = jsonb_build_object(
  'arc', jsonb_build_object(
    'status', case when score_arc is not null then 'done'
                   when stage = 'failed' then 'failed' else 'pending' end,
    'score', score_arc),
  'frontier', jsonb_build_object(
    'status', case when score_frontier is not null then 'done'
                   when stage = 'failed' then 'failed' else 'pending' end,
    'score', score_frontier),
  'ram', jsonb_build_object(
    'status', case when ram_10session_mb is not null then 'done'
                   when stage = 'failed' then 'failed' else 'pending' end,
    'one_session_mb', ram_1session_mb,
    'ten_session_mb', ram_10session_mb)
)
where benchmark_state =
  '{"arc":{"status":"pending"},"frontier":{"status":"pending"},"ram":{"status":"pending"}}'::jsonb
  and (score_arc is not null or score_frontier is not null
       or ram_10session_mb is not null or stage = 'failed');

update harness_runs_exposure_academy set stage = case
  when stage in ('queued') then 'queued'
  when stage in ('done') then 'done'
  when stage in ('failed') then 'failed'
  else 'queued'
end
where stage not in ('queued','preparing','running','done','partial','failed','infra_failed');

alter table harness_runs_exposure_academy
  add constraint harness_runs_exposure_academy_stage_check check
    (stage in ('queued','preparing','running','done','partial','failed','infra_failed'));

alter table harness_runs_exposure_academy
  drop constraint if exists harness_runs_claim_attempts_check;
alter table harness_runs_exposure_academy
  add constraint harness_runs_claim_attempts_check check
    (claim_attempts between 0 and 3);

drop index if exists harness_runs_one_active_per_team;
create unique index if not exists harness_runs_one_active_per_team
  on harness_runs_exposure_academy (team_id)
  where stage not in ('done','partial','failed','infra_failed');

create index if not exists harness_runs_claim_idx
  on harness_runs_exposure_academy (stage, lease_expires_at, created_at);

create table if not exists harness_kaggle_credentials_exposure_academy (
  team_id uuid primary key references harness_teams_exposure_academy(id) on delete cascade,
  username text not null,
  token_nonce bytea not null,
  token_ciphertext bytea not null,
  updated_by uuid references users_exposure_academy(id) on delete set null,
  updated_at timestamptz not null default now()
);

create table if not exists harness_kaggle_submissions_exposure_academy (
  id uuid primary key default gen_random_uuid(),
  run_id uuid not null unique references harness_runs_exposure_academy(id) on delete cascade,
  requested_by uuid references users_exposure_academy(id) on delete set null,
  status text not null default 'queued' check
    (status in ('queued','kernel_running','submitted','scored','failed')),
  kernel_slug text,
  kernel_version integer,
  submission_ref text,
  public_score real,
  private_score real,
  status_message text,
  lease_token uuid,
  lease_expires_at timestamptz,
  next_poll_at timestamptz,
  claim_attempts integer not null default 0 check (claim_attempts between 0 and 3),
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now()
);

alter table harness_kaggle_submissions_exposure_academy
  add column if not exists lease_token uuid,
  add column if not exists lease_expires_at timestamptz,
  add column if not exists next_poll_at timestamptz,
  add column if not exists claim_attempts integer not null default 0;

create index if not exists harness_kaggle_status_idx
  on harness_kaggle_submissions_exposure_academy (status, created_at);

create index if not exists harness_kaggle_claim_idx_v2
  on harness_kaggle_submissions_exposure_academy
    (status, next_poll_at, lease_expires_at, created_at);
