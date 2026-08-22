-- Submitter photo (admin-uploaded, so kids can see a face next to the LinkedIn link) and
-- case-level reference documents (submitter/admin upload, students can only view/download).
-- Avatars stay in Postgres as bytea: one small image per person, bounded and low-volume,
-- the same shape as schedule_image_exposure_academy in the main Academy schema — nothing
-- like the "10 files, anything they have, per submission" pattern that sent submission
-- files to Supabase Storage instead (see 001_init.sql). Case documents CAN be that shape
-- (spec decks, zips), so they go to Storage the same way submission files do.
alter table verified_users add column if not exists avatar bytea;
alter table verified_users add column if not exists avatar_content_type text;

create table if not exists verified_case_documents (
  id uuid primary key default gen_random_uuid(),
  case_id uuid not null references verified_cases(id) on delete cascade,
  filename text not null,
  content_type text not null,
  storage_key text not null,
  size_bytes bigint not null,
  uploaded_by uuid not null references verified_users(id),
  position int not null default 0,
  created_at timestamptz not null default now()
);
create index if not exists verified_case_documents_case_idx on verified_case_documents (case_id, position);
