-- Split new harness submissions into independent ARC and Frontier runs. Existing rows
-- remain bundled Bedrock runs, so queued work and historical results keep their meaning.

alter table harness_runs_exposure_academy
  add column if not exists provider text not null default 'bedrock',
  add column if not exists benchmark_kind text not null default 'bundled';

alter table harness_runs_exposure_academy
  drop constraint if exists harness_runs_provider_check;
alter table harness_runs_exposure_academy
  add constraint harness_runs_provider_check check
    (provider in ('bedrock','cerebras'));

alter table harness_runs_exposure_academy
  drop constraint if exists harness_runs_benchmark_kind_check;
alter table harness_runs_exposure_academy
  add constraint harness_runs_benchmark_kind_check check
    (benchmark_kind in ('arc','frontier','bundled'));

drop index if exists harness_runs_one_active_per_team;
create unique index if not exists harness_runs_one_active_kind_per_team
  on harness_runs_exposure_academy (team_id, benchmark_kind)
  where stage not in ('done','partial','failed','infra_failed','cancelled');
