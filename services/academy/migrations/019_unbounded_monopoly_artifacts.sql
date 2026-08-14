alter table monopoly_artifacts_exposure_academy
  drop constraint if exists monopoly_artifacts_exposure_academy_size_bytes_check;

do $$
begin
  if not exists (
    select 1 from pg_constraint
    where conrelid = 'monopoly_artifacts_exposure_academy'::regclass
      and conname = 'monopoly_artifacts_exposure_academy_size_bytes_positive'
  ) then
    alter table monopoly_artifacts_exposure_academy
      add constraint monopoly_artifacts_exposure_academy_size_bytes_positive
      check (size_bytes >= 1);
  end if;
end
$$;
