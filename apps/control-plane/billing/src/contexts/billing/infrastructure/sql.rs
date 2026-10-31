//! Billing rows through aster-orm. Postgres and SQLite share one schema and one query shape.

use aster_orm::{
    sql_query, Database, Dialect, Executor, Migration, Migrator, PostgresDialect, PostgresExecutor,
    Row, SqliteDialect, SqliteExecutor,
};
use async_trait::async_trait;

use crate::contexts::billing::domain::repository::BillingStore;
use crate::contexts::billing::domain::{
    BillableMetric, Charge, Customer, Invoice, InvoiceLine, LineKind, Plan, Subscription,
    UsageEvent,
};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct ChargeJson {
    metric_code: String,
    unit_amount: i64,
}

#[derive(Serialize, Deserialize)]
struct LineJson {
    kind: String,
    code: String,
    quantity: i64,
    unit_amount: i64,
    amount: i64,
}

fn encode_charges(charges: &[Charge]) -> Result<String, String> {
    let rows: Vec<ChargeJson> = charges
        .iter()
        .map(|charge| ChargeJson {
            metric_code: charge.metric_code.clone(),
            unit_amount: charge.unit_amount,
        })
        .collect();
    serde_json::to_string(&rows).map_err(|err| err.to_string())
}

fn decode_charges(raw: &str) -> Result<Vec<Charge>, String> {
    let rows: Vec<ChargeJson> = serde_json::from_str(raw).map_err(|err| err.to_string())?;
    Ok(rows
        .into_iter()
        .map(|row| Charge {
            metric_code: row.metric_code,
            unit_amount: row.unit_amount,
        })
        .collect())
}

fn encode_lines(lines: &[InvoiceLine]) -> Result<String, String> {
    let rows: Vec<LineJson> = lines
        .iter()
        .map(|line| LineJson {
            kind: match line.kind {
                LineKind::Recurring => "recurring".to_owned(),
                LineKind::Usage => "usage".to_owned(),
            },
            code: line.code.clone(),
            quantity: line.quantity,
            unit_amount: line.unit_amount,
            amount: line.amount,
        })
        .collect();
    serde_json::to_string(&rows).map_err(|err| err.to_string())
}

fn decode_lines(raw: &str) -> Result<Vec<InvoiceLine>, String> {
    let rows: Vec<LineJson> = serde_json::from_str(raw).map_err(|err| err.to_string())?;
    rows.into_iter()
        .map(|row| {
            let kind = match row.kind.as_str() {
                "recurring" => LineKind::Recurring,
                "usage" => LineKind::Usage,
                _ => return Err(format!("unknown invoice line kind {}", row.kind)),
            };
            Ok(InvoiceLine {
                kind,
                code: row.code,
                quantity: row.quantity,
                unit_amount: row.unit_amount,
                amount: row.amount,
            })
        })
        .collect()
}

const MIGRATION: &str = include_str!("../../../../migrations/001_init.sql");

pub struct SqlStore<D, E> {
    dialect: D,
    executor: E,
}

impl SqlStore<SqliteDialect, SqliteExecutor> {
    pub async fn open_in_memory() -> Result<Self, String> {
        let executor = SqliteExecutor::open_in_memory()
            .await
            .map_err(|err| err.to_string())?;
        let store = Self {
            dialect: SqliteDialect,
            executor,
        };
        store.migrate().await?;
        Ok(store)
    }

    pub async fn open_sqlite(path: impl AsRef<std::path::Path>) -> Result<Self, String> {
        let executor = SqliteExecutor::open(path)
            .await
            .map_err(|err| err.to_string())?;
        let store = Self {
            dialect: SqliteDialect,
            executor,
        };
        store.migrate().await?;
        Ok(store)
    }
}

impl SqlStore<PostgresDialect, PostgresExecutor> {
    pub async fn open_postgres(database_url: &str) -> Result<Self, String> {
        let executor =
            PostgresExecutor::connect_no_tls(database_url, 8).map_err(|err| err.to_string())?;
        let store = Self {
            dialect: PostgresDialect,
            executor,
        };
        store.migrate().await?;
        Ok(store)
    }
}

impl<D, E> SqlStore<D, E>
where
    D: Dialect + Clone + Send + Sync + 'static,
    E: Executor + Clone + Send + Sync + 'static,
    E::Row: Row,
{
    fn db(&self) -> Database<D, E> {
        Database::new(self.dialect.clone(), self.executor.clone())
    }

    async fn migrate(&self) -> Result<(), String>
    where
        E: aster_orm::MigrationBackend,
    {
        Migrator::new(self.executor.clone())
            .run([Migration::new("001", "billing", MIGRATION)])
            .await
            .map(|_| ())
            .map_err(|err| err.to_string())
    }
}

// Every statement is `sql_query` because Postgres and SQLite share one text
// shape, including `on conflict` upserts. The typed table builder has no
// shared upsert for both dialects. Values stay bound.
#[async_trait]
impl<D, E> BillingStore for SqlStore<D, E>
where
    D: Dialect + Clone + Send + Sync + 'static,
    E: Executor + Clone + Send + Sync + 'static,
    E::Row: Row,
{
    async fn put_customer(&self, customer: Customer) -> Result<(), String> {
        self.db()
            .execute(
                sql_query::<()>("insert into billing_customers (external_id, name) values (")
                    .bind(customer.external_id)
                    .append(", ")
                    .bind(customer.name)
                    .append(") on conflict (external_id) do update set name = excluded.name"),
            )
            .await
            .map_err(|err| err.to_string())?;
        Ok(())
    }

    async fn customer(&self, external_id: &str) -> Result<Option<Customer>, String> {
        let name = self
            .db()
            .fetch_optional_as(
                sql_query::<String>("select name from billing_customers where external_id = ")
                    .bind(external_id),
            )
            .await
            .map_err(|err| err.to_string())?;
        Ok(name.map(|name| Customer {
            external_id: external_id.to_owned(),
            name,
        }))
    }

    async fn put_metric(&self, metric: BillableMetric) -> Result<(), String> {
        self.db()
            .execute(
                sql_query::<()>("insert into billing_metrics (code, field_name) values (")
                    .bind(metric.code)
                    .append(", ")
                    .bind(metric.field_name)
                    .append(") on conflict (code) do update set field_name = excluded.field_name"),
            )
            .await
            .map_err(|err| err.to_string())?;
        Ok(())
    }

    async fn metric(&self, code: &str) -> Result<Option<BillableMetric>, String> {
        let field_name = self
            .db()
            .fetch_optional_as(
                sql_query::<String>("select field_name from billing_metrics where code = ")
                    .bind(code),
            )
            .await
            .map_err(|err| err.to_string())?;
        Ok(field_name.map(|field_name| BillableMetric {
            code: code.to_owned(),
            field_name,
        }))
    }

    async fn put_plan(&self, plan: Plan) -> Result<(), String> {
        let charges = encode_charges(&plan.charges)?;
        self.db()
            .execute(
                sql_query::<()>(
                    "insert into billing_plans (code, currency, recurring_amount, charges_json) values (",
                )
                .bind(plan.code)
                .append(", ")
                .bind(plan.currency)
                .append(", ")
                .bind(plan.recurring_amount)
                .append(", ")
                .bind(charges)
                .append(
                    ") on conflict (code) do update set currency = excluded.currency, recurring_amount = excluded.recurring_amount, charges_json = excluded.charges_json",
                ),
            )
            .await
            .map_err(|err| err.to_string())?;
        Ok(())
    }

    async fn plan(&self, code: &str) -> Result<Option<Plan>, String> {
        let row = self
            .db()
            .fetch_optional_as(
                sql_query::<(String, i64, String)>(
                    "select currency, recurring_amount, charges_json from billing_plans where code = ",
                )
                .bind(code),
            )
            .await
            .map_err(|err| err.to_string())?;
        match row {
            None => Ok(None),
            Some((currency, recurring_amount, charges_json)) => {
                let charges = decode_charges(&charges_json)?;
                Ok(Some(Plan {
                    code: code.to_owned(),
                    currency,
                    recurring_amount,
                    charges,
                }))
            }
        }
    }

    async fn put_subscription(&self, subscription: Subscription) -> Result<(), String> {
        self.db()
            .execute(
                sql_query::<()>(
                    "insert into billing_subscriptions (external_id, external_customer_id, plan_code, started_at) values (",
                )
                .bind(subscription.external_id)
                .append(", ")
                .bind(subscription.external_customer_id)
                .append(", ")
                .bind(subscription.plan_code)
                .append(", ")
                .bind(subscription.started_at)
                .append(
                    ") on conflict (external_id) do update set external_customer_id = excluded.external_customer_id, plan_code = excluded.plan_code, started_at = excluded.started_at",
                ),
            )
            .await
            .map_err(|err| err.to_string())?;
        Ok(())
    }

    async fn subscription(&self, external_id: &str) -> Result<Option<Subscription>, String> {
        let row = self
            .db()
            .fetch_optional_as(
                sql_query::<(String, String, i64)>(
                    "select external_customer_id, plan_code, started_at from billing_subscriptions where external_id = ",
                )
                .bind(external_id),
            )
            .await
            .map_err(|err| err.to_string())?;
        Ok(row.map(
            |(external_customer_id, plan_code, started_at)| Subscription {
                external_id: external_id.to_owned(),
                external_customer_id,
                plan_code,
                started_at,
            },
        ))
    }

    async fn event(&self, transaction_id: &str) -> Result<Option<UsageEvent>, String> {
        let row = self
            .db()
            .fetch_optional_as(
                sql_query::<(String, String, i64, i64)>(
                    "select external_subscription_id, code, timestamp, quantity from billing_events where transaction_id = ",
                )
                .bind(transaction_id),
            )
            .await
            .map_err(|err| err.to_string())?;
        Ok(row.map(
            |(external_subscription_id, code, timestamp, quantity)| UsageEvent {
                transaction_id: transaction_id.to_owned(),
                external_subscription_id,
                code,
                timestamp,
                quantity,
            },
        ))
    }

    async fn insert_event(&self, event: UsageEvent) -> Result<bool, String> {
        let result = self
            .db()
            .execute(
                sql_query::<()>(
                    "insert into billing_events (transaction_id, external_subscription_id, code, timestamp, quantity) values (",
                )
                .bind(event.transaction_id)
                .append(", ")
                .bind(event.external_subscription_id)
                .append(", ")
                .bind(event.code)
                .append(", ")
                .bind(event.timestamp)
                .append(", ")
                .bind(event.quantity)
                .append(") on conflict (transaction_id) do nothing"),
            )
            .await
            .map_err(|err| err.to_string())?;
        Ok(result.rows_affected > 0)
    }

    async fn events_for_subscription(&self, external_id: &str) -> Result<Vec<UsageEvent>, String> {
        let rows = self
            .db()
            .fetch_all_as(
                sql_query::<(String, String, i64, i64)>(
                    "select transaction_id, code, timestamp, quantity from billing_events where external_subscription_id = ",
                )
                .bind(external_id),
            )
            .await
            .map_err(|err| err.to_string())?;
        Ok(rows
            .rows
            .into_iter()
            .map(|(transaction_id, code, timestamp, quantity)| UsageEvent {
                transaction_id,
                external_subscription_id: external_id.to_owned(),
                code,
                timestamp,
                quantity,
            })
            .collect())
    }

    async fn invoice_for_period(
        &self,
        external_subscription_id: &str,
        period_start: i64,
    ) -> Result<Option<Invoice>, String> {
        let row = self
            .db()
            .fetch_optional_as(
                sql_query::<(String, String, i64, String)>(
                    "select id, currency, period_end, lines_json from billing_invoices where external_subscription_id = ",
                )
                .bind(external_subscription_id)
                .append(" and period_start = ")
                .bind(period_start),
            )
            .await
            .map_err(|err| err.to_string())?;
        match row {
            None => Ok(None),
            Some((id, currency, period_end, lines_json)) => {
                let lines = decode_lines(&lines_json)?;
                Ok(Some(Invoice {
                    id,
                    external_subscription_id: external_subscription_id.to_owned(),
                    currency,
                    period_start,
                    period_end,
                    lines,
                }))
            }
        }
    }

    async fn insert_invoice(&self, invoice: Invoice) -> Result<bool, String> {
        let lines = encode_lines(&invoice.lines)?;
        let result = self
            .db()
            .execute(
                sql_query::<()>(
                    "insert into billing_invoices (external_subscription_id, period_start, id, currency, period_end, lines_json) values (",
                )
                .bind(invoice.external_subscription_id)
                .append(", ")
                .bind(invoice.period_start)
                .append(", ")
                .bind(invoice.id)
                .append(", ")
                .bind(invoice.currency)
                .append(", ")
                .bind(invoice.period_end)
                .append(", ")
                .bind(lines)
                .append(") on conflict (external_subscription_id, period_start) do nothing"),
            )
            .await
            .map_err(|err| err.to_string())?;
        Ok(result.rows_affected > 0)
    }

    async fn save_invoice(&self, invoice: Invoice) -> Result<(), String> {
        let lines = encode_lines(&invoice.lines)?;
        self.db()
            .execute(
                sql_query::<()>(
                    "insert into billing_invoices (external_subscription_id, period_start, id, currency, period_end, lines_json) values (",
                )
                .bind(invoice.external_subscription_id)
                .append(", ")
                .bind(invoice.period_start)
                .append(", ")
                .bind(invoice.id)
                .append(", ")
                .bind(invoice.currency)
                .append(", ")
                .bind(invoice.period_end)
                .append(", ")
                .bind(lines)
                .append(
                    ") on conflict (external_subscription_id, period_start) do update set currency = excluded.currency, period_end = excluded.period_end, lines_json = excluded.lines_json",
                ),
            )
            .await
            .map_err(|err| err.to_string())?;
        Ok(())
    }

    async fn invoice_count(&self) -> Result<usize, String> {
        let count = self
            .db()
            .fetch_one_as(sql_query::<i64>("select count(*) from billing_invoices"))
            .await
            .map_err(|err| err.to_string())?;
        Ok(count.max(0) as usize)
    }

    async fn ping(&self) -> Result<(), String> {
        self.db()
            .fetch_one_as(sql_query::<i64>("select 1"))
            .await
            .map_err(|err| err.to_string())?;
        Ok(())
    }
}
