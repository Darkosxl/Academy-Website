-- Chatbot Challenge: a prompt-injection education game on the Beginner Track.
-- 10 fixed levels (CHATBOT_LEVELS in chatbot_challenge.rs), same shared secret
-- set for every student — a fair race, not a per-student puzzle. "Current
-- level" is never stored: it's count(chatbot_completions_exposure_academy
-- rows) + 1, derived at read time — same philosophy as leader_rows() in
-- portal.rs computing every other leaderboard score at read time.

create table if not exists chatbot_messages_exposure_academy (
  id uuid primary key default gen_random_uuid(),
  user_id uuid not null references users_exposure_academy(id) on delete cascade,
  level smallint not null check (level between 1 and 10),
  role text not null check (role in ('user','assistant')),
  content text not null,
  created_at timestamptz not null default now()
);
-- ordered per-(user,level) transcript retrieval, and what the reset DELETE filters on
create index if not exists chatbot_messages_user_level_idx
  on chatbot_messages_exposure_academy (user_id, level, created_at);

-- One row per level a student has cracked. PK doubles as the ON CONFLICT DO
-- NOTHING guard against double-insertion (e.g. two tabs racing a lucky reply).
create table if not exists chatbot_completions_exposure_academy (
  user_id uuid not null references users_exposure_academy(id) on delete cascade,
  level smallint not null check (level between 1 and 10),
  completed_at timestamptz not null default now(),
  primary key (user_id, level)
);
