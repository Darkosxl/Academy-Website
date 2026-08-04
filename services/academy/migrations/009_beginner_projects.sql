-- Beginner Track: five fixed projects (BEGINNER_PROJECTS in html.rs, not admin-editable —
-- same pattern as DEMOS). Each student can save one GitHub + Vercel link pair per project.
-- No grading yet, so there is no status/points column here — just the links, upserted per
-- (user, project_key) so resubmitting replaces rather than piles up.
create table if not exists beginner_submissions_exposure_academy (
  user_id uuid not null references users_exposure_academy(id) on delete cascade,
  project_key text not null,
  repo_url text not null,
  vercel_url text not null,
  updated_at timestamptz not null default now(),
  primary key (user_id, project_key)
);
