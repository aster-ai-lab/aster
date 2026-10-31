create table if not exists auth_api_keys (
    token_hash text primary key,
    id uuid not null unique,
    plane text not null,
    subject_id uuid not null,
    org_id uuid,
    name text not null,
    hint text not null,
    created_at bigint not null,
    expires_at bigint,
    last_used_at bigint
);

create index if not exists auth_api_keys_subject_idx
    on auth_api_keys (plane, subject_id);
