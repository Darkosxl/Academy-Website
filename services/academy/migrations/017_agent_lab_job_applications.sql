-- Agent Lab challenge 3 (Job Application Agent): ten sandbox internship forms an agent
-- fills from the student's local profile.md.
--
-- Third isolated lab table, same reasoning as 016's two: nothing a browser agent types
-- into these forms may reach a real profile, a graded submission or the standings. Only
-- the challenge-3 handlers read or write it; leader_rows, the Görev Puanlama queue and
-- every admin view are unaware it exists.
--
-- A row exists only once an application has been submitted successfully, so presence *is*
-- completion — there is deliberately no `submitted` boolean that could disagree with it,
-- and progress is a plain count(*) over this table. Re-submitting one job upserts on
-- (user_id, job_key), so a student can correct an application without the count moving.
--
-- `answers` holds the submitted values as a JSON object (field name -> string, or array
-- for checkbox groups). Stored as text rather than jsonb on purpose: sqlx is declared
-- without its `json` feature, and the column is only ever read back to re-fill the form,
-- never queried into.
create table if not exists agent_lab_job_applications_exposure_academy (
  user_id uuid not null references users_exposure_academy(id) on delete cascade,
  job_key text not null,
  answers text not null,
  updated_at timestamptz not null default now(),
  primary key (user_id, job_key)
);
