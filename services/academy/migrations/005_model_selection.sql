-- The selected Bedrock model travels with the run so retries and reclaimed leases use
-- the same inference target. Academy validates the shared allowlist before insertion.

alter table harness_runs_exposure_academy
  add column if not exists model_id text not null default 'xai.grok-4.3';

alter table harness_runs_exposure_academy
  drop constraint if exists harness_runs_model_id_check;
alter table harness_runs_exposure_academy
  add constraint harness_runs_model_id_check check
    (char_length(model_id) between 1 and 120);
