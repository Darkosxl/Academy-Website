alter table verified_users drop constraint verified_users_role_check;
alter table verified_users add constraint verified_users_role_check
  check (role in ('admin','owner','submitter','student'));
