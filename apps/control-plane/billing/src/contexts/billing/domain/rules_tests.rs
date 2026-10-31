#![cfg(test)]
use super::super::{Charge, LineKind, Plan, UsageEvent};
use super::*;

#[test]
fn usage_through_stops_at_the_bill_timestamp() {
    let period = Period {
        start: 1_700_000_000,
        end: 1_700_000_000 + PERIOD_SECS,
    };
    let events = vec![
        UsageEvent {
            transaction_id: "early".into(),
            external_subscription_id: "sub".into(),
            code: "tracked_pages".into(),
            timestamp: period.start + 10,
            quantity: 4,
        },
        UsageEvent {
            transaction_id: "later".into(),
            external_subscription_id: "sub".into(),
            code: "tracked_pages".into(),
            timestamp: period.start + 40,
            quantity: 6,
        },
    ];
    assert_eq!(
        usage_through(&events, "tracked_pages", period, period.start + 10),
        4
    );
    assert_eq!(
        usage_through(&events, "tracked_pages", period, period.start + 40),
        10
    );
}

fn plan() -> Plan {
    Plan {
        code: "hybrid".into(),
        currency: "USD".into(),
        recurring_amount: 500,
        charges: vec![Charge {
            metric_code: "tracked_pages".into(),
            unit_amount: 3,
        }],
    }
}

#[test]
fn sum_ignores_other_metrics_and_other_periods() {
    let period = Period {
        start: 100,
        end: 100 + PERIOD_SECS,
    };
    let events = vec![
        UsageEvent {
            transaction_id: "a".into(),
            external_subscription_id: "sub".into(),
            code: "tracked_pages".into(),
            timestamp: 150,
            quantity: 10,
        },
        UsageEvent {
            transaction_id: "b".into(),
            external_subscription_id: "sub".into(),
            code: "other".into(),
            timestamp: 160,
            quantity: 99,
        },
        UsageEvent {
            transaction_id: "c".into(),
            external_subscription_id: "sub".into(),
            code: "tracked_pages".into(),
            timestamp: period.end,
            quantity: 7,
        },
    ];
    assert_eq!(usage_quantity(&events, "tracked_pages", period), 10);
}

#[test]
fn hybrid_lines_use_integer_minor_units() {
    let mut quantities = BTreeMap::new();
    quantities.insert("tracked_pages".into(), 30);
    let lines = invoice_lines(&plan(), &quantities).unwrap();
    assert_eq!(lines[0].kind, LineKind::Recurring);
    assert_eq!(lines[0].amount, 500);
    assert_eq!(lines[1].kind, LineKind::Usage);
    assert_eq!(lines[1].quantity, 30);
    assert_eq!(lines[1].unit_amount, 3);
    assert_eq!(lines[1].amount, 90);
}

#[test]
fn invoice_lines_refuse_a_quantity_that_overflows() {
    let mut quantities = BTreeMap::new();
    quantities.insert("tracked_pages".into(), i64::MAX);
    assert!(invoice_lines(&plan(), &quantities).is_none());
}
