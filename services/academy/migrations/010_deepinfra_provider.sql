-- Allow DeepInfra (Qwen3.6-27B) harness runs. Same drop-then-add pattern as 007;
-- without this every insert with provider = 'deepinfra' fails the check constraint.

alter table harness_runs_exposure_academy
  drop constraint if exists harness_runs_provider_check;
alter table harness_runs_exposure_academy
  add constraint harness_runs_provider_check check
    (provider in ('bedrock','cerebras','deepinfra'));
