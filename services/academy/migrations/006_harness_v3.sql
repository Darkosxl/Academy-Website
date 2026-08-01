-- Harness v3 scores all 25 public ARC games and lets a team cancel its active run.
-- Migrations are replayed at startup, so every statement is idempotent.

alter table harness_runs_exposure_academy
  alter column benchmark_version set default 'harness-2026-sprint-v3';

-- Never execute an unclaimed historical row under today's scoring rules. Leased
-- v1/v2 runs may finish under their original worker during a rolling deploy.
update harness_runs_exposure_academy
set stage = 'cancelled', error_log = 'Superseded by harness v3 before execution.',
    lease_token = null, lease_expires_at = null, updated_at = now()
where benchmark_version <> 'harness-2026-sprint-v3' and stage = 'queued';

alter table harness_runs_exposure_academy
  drop constraint if exists harness_runs_exposure_academy_stage_check;
alter table harness_runs_exposure_academy
  add constraint harness_runs_exposure_academy_stage_check check
    (stage in ('queued','preparing','running','done','partial','failed','infra_failed','cancelled'));

drop index if exists harness_runs_one_active_per_team;
create unique index if not exists harness_runs_one_active_per_team
  on harness_runs_exposure_academy (team_id)
  where stage not in ('done','partial','failed','infra_failed','cancelled');
