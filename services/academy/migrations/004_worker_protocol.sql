-- Worker protocol hardening. Result lease tokens make terminal writes idempotent without
-- allowing an old worker to mutate a reclaimed run. This project replays migrations at boot,
-- so every operation remains idempotent.

alter table harness_runs_exposure_academy
  add column if not exists result_lease_token uuid;

alter table harness_kaggle_submissions_exposure_academy
  add column if not exists last_result_lease_token uuid;
