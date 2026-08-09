-- Track level lives on the user now (PRESEED/SEED/SERIES_A, same values Video/Task
-- already use for content difficulty). Defaults everyone to PRESEED (beginner);
-- backfills anyone already on a harness or monopoly team to SERIES_A (advanced),
-- since team membership is the only signal that ever existed for this distinction.
alter table users_exposure_academy
  add column if not exists level text not null default 'PRESEED'
    check (level in ('PRESEED', 'SEED', 'SERIES_A'));

update users_exposure_academy set level = 'SERIES_A'
where level = 'PRESEED'
  and id in (
    select user_id from harness_team_members_exposure_academy
    union
    select user_id from monopoly_team_members_exposure_academy
  );
