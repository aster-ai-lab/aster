alter table auth_lockouts add column if not exists version bigint not null default 0;
