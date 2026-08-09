-- Agent Lab (Beginner Track) — a sandbox copy of the two portal surfaces a browser agent
-- is asked to drive: the student profile form and the project submission form.
--
-- Deliberately its own pair of tables rather than extra columns on
-- users_exposure_academy / beginner_submissions_exposure_academy. An agent driving the
-- lab types whatever it likes, as often as it likes, and can wipe it back to empty —
-- none of which may touch a student's real profile or a graded submission. Nothing here
-- is read by leader_rows, by the Görev Puanlama queue, or by the admin submission views,
-- so the isolation is structural: there is no join to get wrong later.

create table if not exists agent_lab_profiles_exposure_academy (
  user_id uuid primary key references users_exposure_academy(id) on delete cascade,
  full_name text not null,
  school text not null,
  grade text not null,
  interest text not null,
  agent_goal text not null default '',
  updated_at timestamptz not null default now()
);

-- One row per student, not one per attempt: the challenge is "get it right", so a retry
-- replaces the previous attempt instead of piling up a history nobody reads. `correct` is
-- stored rather than recomputed at render time so the card reports the verdict of the
-- attempt as it was judged, even if the brief's target project is edited later.
create table if not exists agent_lab_submissions_exposure_academy (
  user_id uuid primary key references users_exposure_academy(id) on delete cascade,
  project_key text not null,
  repo_url text not null,
  demo_url text not null,
  correct boolean not null default false,
  updated_at timestamptz not null default now()
);
