//! Pure usage aggregation and invoice totals. No IO.

use std::collections::BTreeMap;

use super::{Charge, InvoiceLine, LineKind, Plan, UsageEvent};

pub const PERIOD_SECS: i64 = 30 * 24 * 60 * 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Period {
    pub start: i64,
    pub end: i64,
}

pub fn period_containing(started_at: i64, timestamp: i64) -> Option<Period> {
    if timestamp < started_at {
        return None;
    }
    let index = timestamp.saturating_sub(started_at) / PERIOD_SECS;
    let start = started_at.saturating_add(index.saturating_mul(PERIOD_SECS));
    Some(Period {
        start,
        end: start.saturating_add(PERIOD_SECS),
    })
}

/// Quantity for the whole period. Billing uses [`usage_through`] so an event
/// after the bill timestamp stays off that invoice.
pub fn usage_quantity(events: &[UsageEvent], metric_code: &str, period: Period) -> i64 {
    usage_through(events, metric_code, period, period.end.saturating_sub(1))
}

/// Usage in `period` whose timestamp is at or before `until`.
pub fn usage_through(events: &[UsageEvent], metric_code: &str, period: Period, until: i64) -> i64 {
    events
        .iter()
        .filter(|event| {
            event.code == metric_code
                && event.timestamp >= period.start
                && event.timestamp < period.end
                && event.timestamp <= until
        })
        .map(|event| event.quantity)
        .sum()
}

pub fn invoice_lines(plan: &Plan, quantities: &BTreeMap<String, i64>) -> Option<Vec<InvoiceLine>> {
    let mut lines = Vec::with_capacity(1 + plan.charges.len());
    lines.push(InvoiceLine {
        kind: LineKind::Recurring,
        code: plan.code.clone(),
        quantity: 1,
        unit_amount: plan.recurring_amount,
        amount: plan.recurring_amount,
    });
    for charge in &plan.charges {
        let quantity = *quantities.get(&charge.metric_code).unwrap_or(&0);
        let amount = quantity.checked_mul(charge.unit_amount)?;
        lines.push(usage_line(charge, quantity, amount));
    }
    Some(lines)
}

fn usage_line(charge: &Charge, quantity: i64, amount: i64) -> InvoiceLine {
    InvoiceLine {
        kind: LineKind::Usage,
        code: charge.metric_code.clone(),
        quantity,
        unit_amount: charge.unit_amount,
        amount,
    }
}

#[cfg(test)]
#[path = "rules_tests.rs"]
mod tests;
