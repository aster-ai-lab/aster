alter table auth_sessions add column if not exists computer_id text;
alter table auth_challenges add column if not exists computer_id text;
