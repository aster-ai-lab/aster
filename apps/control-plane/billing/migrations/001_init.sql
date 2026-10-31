create table if not exists billing_customers (
    external_id text primary key,
    name text not null
);

create table if not exists billing_metrics (
    code text primary key,
    field_name text not null
);

create table if not exists billing_plans (
    code text primary key,
    currency text not null,
    recurring_amount bigint not null,
    charges_json text not null
);

create table if not exists billing_subscriptions (
    external_id text primary key,
    external_customer_id text not null,
    plan_code text not null,
    started_at bigint not null
);

create table if not exists billing_events (
    transaction_id text primary key,
    external_subscription_id text not null,
    code text not null,
    timestamp bigint not null,
    quantity bigint not null
);

create table if not exists billing_invoices (
    external_subscription_id text not null,
    period_start bigint not null,
    id text not null,
    currency text not null,
    period_end bigint not null,
    lines_json text not null,
    primary key (external_subscription_id, period_start)
);
