-- distributed config center (PG source of truth + NATS invalidate)

create table if not exists config_meta (
    id int primary key default 1,
    revision bigint not null default 0,
    constraint config_meta_singleton_chk check (id = 1),
    constraint config_meta_revision_chk check (revision >= 0)
);

insert into config_meta (id, revision)
values (1, 0)
on conflict (id) do nothing;

create table if not exists config_entries (
    namespace text not null,
    key text not null,
    value text not null,
    revision bigint not null,
    updated_at_epoch bigint not null,
    updated_by text not null default '',
    primary key (namespace, key),
    constraint config_entries_namespace_chk check (namespace ~ '^[a-z][a-z0-9_-]{0,63}$'),
    constraint config_entries_key_chk check (key ~ '^[a-z][a-z0-9._-]{0,127}$'),
    constraint config_entries_revision_chk check (revision > 0)
);

create index if not exists config_entries_revision_idx on config_entries (revision desc);

-- Remember a removed config key so an older revision cannot recreate it.
create table if not exists config_removals (
    namespace text not null,
    key text not null,
    revision bigint not null,
    primary key (namespace, key),
    constraint config_removals_revision_chk check (revision > 0)
);
