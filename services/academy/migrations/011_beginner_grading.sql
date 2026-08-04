-- Beginner Track projects became gradeable. 009 deliberately left grading out ("No grading
-- yet, so there is no status/points column here"); Görev Puanlama now reviews them in the same
-- queue as board submissions, so the columns mirror submissions_exposure_academy one-for-one —
-- same status vocabulary, same nullable feedback, same non-negative points_override.
--
-- A passed Beginner Track project is worth PTS_PROJECT_L1 (100) unless points_override says
-- otherwise; leader_rows binds that default rather than storing it here. These rows have no
-- level of their own, and none is added: Beginner is the only thing they could ever be.
alter table beginner_submissions_exposure_academy
  add column if not exists status text not null default 'pending';
alter table beginner_submissions_exposure_academy
  drop constraint if exists beginner_submissions_status_check;
alter table beginner_submissions_exposure_academy
  add constraint beginner_submissions_status_check
  check (status in ('pending','reviewing','passed','failed'));

alter table beginner_submissions_exposure_academy
  add column if not exists feedback text;

alter table beginner_submissions_exposure_academy
  add column if not exists points_override int;
alter table beginner_submissions_exposure_academy
  drop constraint if exists beginner_submissions_points_override_nonneg;
alter table beginner_submissions_exposure_academy
  add constraint beginner_submissions_points_override_nonneg
  check (points_override is null or points_override >= 0);

-- The grading queue reads "everything still waiting on me" on every page load, across all
-- students, so status leads the index.
create index if not exists beginner_submissions_status_idx
  on beginner_submissions_exposure_academy (status, updated_at desc);
