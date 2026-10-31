create table if not exists auth_credentials (
    plane text not null,
    subject_id uuid not null,
    password_hash text not null,
    totp_secret bytea,
    primary key (plane, subject_id)
);

create table if not exists auth_sessions (
    token_hash text primary key,
    plane text not null,
    subject_id uuid not null,
    org_id uuid,
    auth_time bigint not null,
    amr text not null,
    expires_at bigint not null
);

create index if not exists auth_sessions_subject_idx
    on auth_sessions (plane, subject_id);

create table if not exists auth_lockouts (
    lock_key text primary key,
    failures integer not null,
    locked_until bigint not null
);

create table if not exists auth_challenges (
    id uuid primary key,
    plane text not null,
    subject_id uuid not null,
    org_id uuid,
    auth_time bigint not null,
    expires_at bigint not null
);

create table if not exists auth_events (
    id bigserial primary key,
    kind text not null,
    plane text not null,
    subject_id uuid,
    at_epoch bigint not null
);

create table if not exists auth_codes (
    code_hash text primary key,
    client_id text not null,
    redirect_uri text not null,
    code_challenge text not null,
    nonce text not null,
    session_json jsonb not null,
    expires_at bigint not null
);

create table if not exists auth_used_codes (
    code_hash text primary key
);

create table if not exists auth_refreshes (
    token_hash text primary key,
    client_id text not null,
    session_json jsonb not null,
    expires_at bigint not null
);

create table if not exists auth_used_refreshes (
    token_hash text primary key
);

create table if not exists auth_oidc_clients (
    client_id text primary key,
    secret_hash text not null,
    redirect_uris text[] not null
);
