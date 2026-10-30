//! Local budget rules; the host owns durable reservations and settlement.

use anyhow::{bail, ensure, Context, Result};
use chrono::{Datelike, FixedOffset, NaiveDate, TimeZone, Utc};
use serde::{Deserialize, Serialize};

use crate::llm::metering::{Currency, InvocationRecord, InvocationStart};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum BudgetScope {
    Global,
    Provider { provider_id: String },
    GatewayClient { client_id: String },
}

impl BudgetScope {
    pub fn parse(kind: &str, id: Option<&str>) -> Result<Self> {
        match (kind, id) {
            ("global", None) => Ok(Self::Global),
            ("provider", Some(id)) if valid_text(id) => Ok(Self::Provider {
                provider_id: id.to_owned(),
            }),
            ("gateway_client", Some(id)) if valid_text(id) => Ok(Self::GatewayClient {
                client_id: id.to_owned(),
            }),
            _ => bail!("invalid local budget scope: {kind}"),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Global => "global",
            Self::Provider { .. } => "provider",
            Self::GatewayClient { .. } => "gateway_client",
        }
    }

    pub fn matches(&self, provider_id: &str, client_id: Option<&str>) -> bool {
        match self {
            Self::Global => true,
            Self::Provider {
                provider_id: expected,
            } => expected == provider_id,
            Self::GatewayClient {
                client_id: expected,
            } => client_id == Some(expected.as_str()),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BudgetPeriod {
    Daily,
    Monthly,
}

impl BudgetPeriod {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "daily" => Ok(Self::Daily),
            "monthly" => Ok(Self::Monthly),
            _ => bail!("unknown local budget period: {value}"),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Daily => "daily",
            Self::Monthly => "monthly",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum BudgetResource {
    Tokens,
    Cost { currency: Currency },
}

impl BudgetResource {
    pub fn parse(kind: &str, currency: Option<Currency>) -> Result<Self> {
        match (kind, currency) {
            ("tokens", None) => Ok(Self::Tokens),
            ("cost", Some(currency)) => Ok(Self::Cost { currency }),
            _ => bail!("invalid local budget resource: {kind}"),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Tokens => "tokens",
            Self::Cost { .. } => "cost",
        }
    }

    pub fn upper_bound(self, start: &InvocationStart) -> Option<u64> {
        match self {
            Self::Tokens => start.bounds.tokens,
            Self::Cost { currency } => {
                if start.reference_price
                    || start
                        .pricing
                        .as_ref()
                        .is_some_and(|price| price.currency != currency)
                {
                    return Some(0);
                }
                start
                    .bounds
                    .cost
                    .as_ref()
                    .filter(|bound| bound.currency == currency)
                    .map(|bound| bound.micro_units)
            }
        }
    }

    /// Cost units are millionths of the declared currency, rounded up per call.
    pub fn measured_units(self, record: &InvocationRecord) -> Result<Option<u64>> {
        match self {
            Self::Tokens => Ok(record
                .usage
                .total_tokens()?
                .filter(|_| record.usage_complete)),
            Self::Cost { currency } => {
                if record.reference_price
                    || record
                        .pricing
                        .as_ref()
                        .is_some_and(|price| price.currency != currency)
                {
                    return Ok(Some(0));
                }
                record.estimated_cost()?.map(money_units).transpose()
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BudgetMode {
    Notify,
    Stop,
}

impl BudgetMode {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "notify" => Ok(Self::Notify),
            "stop" => Ok(Self::Stop),
            _ => bail!("unknown local budget mode: {value}"),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Notify => "notify",
            Self::Stop => "stop",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LocalBudget {
    pub id: String,
    pub name: String,
    pub scope: BudgetScope,
    pub period: BudgetPeriod,
    pub resource: BudgetResource,
    pub mode: BudgetMode,
    pub limit: u64,
    pub notify_percent: u8,
    /// Captured when configured so a system-zone change cannot move a cycle.
    pub utc_offset_minutes: i32,
}

impl LocalBudget {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            valid_text(&self.id) && valid_text(&self.name),
            "local budget identity must contain 1 to 1024 bytes"
        );
        ensure!(self.limit > 0, "local budget limit must be positive");
        ensure!(
            (1..=100).contains(&self.notify_percent),
            "local budget notification percentage must be 1 to 100"
        );
        ensure!(
            (-1439..=1439).contains(&self.utc_offset_minutes),
            "invalid local budget UTC offset"
        );
        match &self.scope {
            BudgetScope::Global => {}
            BudgetScope::Provider { provider_id } => {
                ensure!(valid_text(provider_id), "invalid budget provider identity")
            }
            BudgetScope::GatewayClient { client_id } => {
                ensure!(valid_text(client_id), "invalid budget client identity")
            }
        }
        Ok(())
    }

    pub fn window_at(&self, now_ms: u64) -> Result<BudgetWindow> {
        let seconds = self
            .utc_offset_minutes
            .checked_mul(60)
            .context("invalid local budget UTC offset")?;
        let zone = FixedOffset::east_opt(seconds).context("invalid local budget UTC offset")?;
        let now = chrono::DateTime::<Utc>::from_timestamp_millis(i64::try_from(now_ms)?)
            .context("local budget timestamp is out of range")?
            .with_timezone(&zone);
        let day = now.date_naive();
        let (first, next) = match self.period {
            BudgetPeriod::Daily => (
                day,
                day.succ_opt().context("local budget calendar overflow")?,
            ),
            BudgetPeriod::Monthly => {
                let first = day.with_day(1).context("invalid local budget month")?;
                let (year, month) = if day.month() == 12 {
                    (
                        day.year()
                            .checked_add(1)
                            .context("local budget calendar overflow")?,
                        1,
                    )
                } else {
                    (day.year(), day.month() + 1)
                };
                (
                    first,
                    NaiveDate::from_ymd_opt(year, month, 1)
                        .context("local budget calendar overflow")?,
                )
            }
        };
        let midnight = |day: NaiveDate| -> Result<i64> {
            Ok(zone
                .from_local_datetime(
                    &day.and_hms_opt(0, 0, 0)
                        .context("invalid local budget day")?,
                )
                .single()
                .context("invalid local budget calendar boundary")?
                .timestamp_millis())
        };
        Ok(BudgetWindow {
            start_ms: midnight(first)?,
            end_ms: midnight(next)?,
        })
    }

    /// Called inside the host's reservation transaction with current counters.
    pub fn admit(
        &self,
        state: BudgetState,
        upper_bound: Option<u64>,
    ) -> std::result::Result<BudgetAdmission, BudgetRefusal> {
        let reserved_units = upper_bound.unwrap_or(0);
        let projected = state
            .consumed
            .checked_add(state.reserved)
            .and_then(|value| value.checked_add(reserved_units))
            .ok_or(BudgetRefusal::CounterOverflow)?;
        if self.mode == BudgetMode::Stop {
            if state.unbounded_calls > 0 {
                return Err(BudgetRefusal::UnknownUsage);
            }
            if upper_bound.is_none() {
                return Err(BudgetRefusal::UnknownBound);
            }
            if projected > self.limit {
                return Err(BudgetRefusal::Exhausted);
            }
        }
        Ok(BudgetAdmission {
            reserved_units,
            notify: (projected as u128) * 100
                >= (self.limit as u128) * (self.notify_percent as u128)
                || state.unbounded_calls > 0
                || upper_bound.is_none(),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BudgetWindow {
    pub start_ms: i64,
    pub end_ms: i64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BudgetState {
    pub consumed: u64,
    pub reserved: u64,
    pub unbounded_calls: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BudgetAdmission {
    pub reserved_units: u64,
    pub notify: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BudgetRefusal {
    #[error("local budget is exhausted")]
    Exhausted,
    #[error("local budget requires a verified call bound")]
    UnknownBound,
    #[error("local budget contains calls with unknown unreserved usage")]
    UnknownUsage,
    #[error("local budget counter overflow")]
    CounterOverflow,
}

impl BudgetRefusal {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "exhausted" => Ok(Self::Exhausted),
            "unknown_bound" => Ok(Self::UnknownBound),
            "unknown_usage" => Ok(Self::UnknownUsage),
            "counter_overflow" => Ok(Self::CounterOverflow),
            _ => bail!("unknown local budget refusal: {value}"),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Exhausted => "exhausted",
            Self::UnknownBound => "unknown_bound",
            Self::UnknownUsage => "unknown_usage",
            Self::CounterOverflow => "counter_overflow",
        }
    }
}

fn money_units(amount: f64) -> Result<u64> {
    let units = (amount * 1_000_000.0).ceil();
    ensure!(
        units.is_finite() && units >= 0.0 && units < u64::MAX as f64,
        "local budget cost is out of range"
    );
    Ok(units as u64)
}

fn valid_text(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 1024
}

#[cfg(test)]
mod tests;
