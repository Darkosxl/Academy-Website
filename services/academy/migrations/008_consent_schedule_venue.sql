-- Ported from the pre-workspace tree, where these lived at the bottom of
-- migrations/001_init.sql. Every migration here replays on every boot, so all of it is
-- written to be re-runnable.

-- Haftalık program: the schedule lives in a spreadsheet the team keeps outside the
-- portal, and what students see is an image of it the admin uploads (one per track).
-- Stored as bytes here rather than on disk so a redeploy can't lose it, same as
-- screenshot_cache_exposure_academy. One row per track, replaced on re-upload.
create table if not exists schedule_image_exposure_academy (
  track text primary key check (track in ('beginner','advanced')),
  image bytea not null,
  content_type text not null,
  uploaded_at timestamptz not null default now()
);

-- Veli onay formları: the signed parental-consent forms a student uploads. One row per
-- FILE, not per form — a form is often photographed a page at a time, so a student can
-- have several rows for the same `kind` and re-uploading adds rather than replaces.
-- Bytes live here for the same reason as schedule_image_exposure_academy: a redeploy
-- must not lose a document, and these are the one thing we cannot ask a student to
-- re-do at short notice. `kind` values are baked into CONSENT_DOCS in model.rs.
create table if not exists consent_docs_exposure_academy (
  id uuid primary key default gen_random_uuid(),
  user_id uuid not null references users_exposure_academy(id) on delete cascade,
  kind text not null check (kind in ('exposure','qnbeyond','paribu')),
  filename text not null,          -- what the student's file was called, for the download
  content_type text not null,      -- sniffed from the bytes on upload, never the browser's claim
  file bytea not null,
  uploaded_at timestamptz not null default now()
);
create index if not exists consent_docs_exposure_academy_user_idx
  on consent_docs_exposure_academy (user_id, kind);

-- The venue used to be one address (`venue_name` etc). The two academy weeks run in
-- different places, so it is now per week (`venue1_*`, `venue2_*`); whatever was
-- already entered becomes week 1. Idempotent: after the first run there are no
-- `venue_%` rows left to move, and `do nothing` protects an already-edited week 1.
insert into app_settings_exposure_academy (key, value, updated_at)
  select replace(key, 'venue_', 'venue1_'), value, updated_at
  from app_settings_exposure_academy where key like 'venue\_%'
  on conflict (key) do nothing;
delete from app_settings_exposure_academy where key like 'venue\_%';
