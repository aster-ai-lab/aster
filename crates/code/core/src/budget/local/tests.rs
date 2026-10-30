#![cfg(test)]

use super::*;
use crate::llm::metering::{AttemptStatus, ObservedTokens, PricingSnapshot, TokenPrices};

fn budget() -> LocalBudget {
    LocalBudget {
        id: "daily".into(),
        name: "Daily budget".into(),
        scope: BudgetScope::Global,
        period: BudgetPeriod::Daily,
        resource: BudgetResource::Tokens,
        mode: BudgetMode::Stop,
        limit: 100,
        notify_percent: 80,
        utc_offset_minutes: 480,
    }
}

fn timestamp(value: &str) -> u64 {
    chrono::DateTime::parse_from_rfc3339(value)
        .unwrap()
        .timestamp_millis() as u64
}

#[test]
fn reservations_are_included_before_admitting_another_call() {
    let policy = budget();
    let state = BudgetState {
        consumed: 30,
        reserved: 50,
        unbounded_calls: 0,
    };
    assert_eq!(policy.admit(state, Some(21)), Err(BudgetRefusal::Exhausted));
    let admitted = policy.admit(state, Some(20)).unwrap();
    assert_eq!(admitted.reserved_units, 20);
    assert!(admitted.notify);
    assert!(policy
        .admit(
            BudgetState {
                consumed: 100,
                ..Default::default()
            },
            Some(0)
        )
        .is_ok());
}

#[test]
fn stop_mode_refuses_unknown_bounds_and_unreserved_history() {
    let policy = budget();
    assert_eq!(
        policy.admit(BudgetState::default(), None),
        Err(BudgetRefusal::UnknownBound)
    );
    assert_eq!(
        policy.admit(
            BudgetState {
                unbounded_calls: 1,
                ..Default::default()
            },
            Some(1)
        ),
        Err(BudgetRefusal::UnknownUsage)
    );
    assert_eq!(
        policy.admit(
            BudgetState {
                consumed: u64::MAX,
                ..Default::default()
            },
            Some(1)
        ),
        Err(BudgetRefusal::CounterOverflow)
    );
}

#[test]
fn notify_mode_allows_calls_beyond_the_limit_and_exposes_incomplete_usage() {
    let mut policy = budget();
    policy.mode = BudgetMode::Notify;
    assert!(
        policy
            .admit(
                BudgetState {
                    consumed: 200,
                    ..Default::default()
                },
                Some(10)
            )
            .unwrap()
            .notify
    );
    let unknown = policy
        .admit(
            BudgetState {
                unbounded_calls: 1,
                ..Default::default()
            },
            None,
        )
        .unwrap();
    assert_eq!(unknown.reserved_units, 0);
    assert!(unknown.notify);
}

#[test]
fn daily_windows_use_the_saved_offset_and_roll_over_at_its_midnight() {
    let policy = budget();
    let before = policy.window_at(timestamp("2026-10-28T15:59:59Z")).unwrap();
    let after = policy.window_at(timestamp("2026-10-28T16:00:00Z")).unwrap();
    assert_eq!(before.start_ms, timestamp("2026-10-27T16:00:00Z") as i64);
    assert_eq!(before.end_ms, after.start_ms);
    assert_eq!(after.end_ms, timestamp("2026-10-29T16:00:00Z") as i64);
}

#[test]
fn monthly_windows_handle_leap_days_and_year_boundaries() {
    let mut policy = budget();
    policy.period = BudgetPeriod::Monthly;
    let leap = policy.window_at(timestamp("2028-02-29T12:00:00Z")).unwrap();
    assert_eq!(leap.start_ms, timestamp("2028-01-31T16:00:00Z") as i64);
    assert_eq!(leap.end_ms, timestamp("2028-02-29T16:00:00Z") as i64);
    let december = policy.window_at(timestamp("2026-12-31T15:59:00Z")).unwrap();
    assert_eq!(december.end_ms, timestamp("2026-12-31T16:00:00Z") as i64);
    assert_eq!(
        policy
            .window_at(timestamp("2026-12-31T16:00:00Z"))
            .unwrap()
            .start_ms,
        december.end_ms
    );
}

#[test]
fn money_budgets_separate_currencies_and_subscription_reference_value() {
    let mut record = InvocationRecord {
        request_id: "request".into(),
        attempt_id: "attempt".into(),
        source_id: "chat".into(),
        session_id: None,
        provider_id: "provider".into(),
        model_id: "model".into(),
        credential_id: None,
        response_model: None,
        status: AttemptStatus::Succeeded,
        started_at_ms: 0,
        ended_at_ms: 1,
        first_token_ms: None,
        usage: ObservedTokens {
            input_tokens: Some(1),
            output_tokens: Some(0),
            no_cache_tokens: Some(1),
            cache_read_tokens: Some(0),
            cache_write_tokens: Some(0),
        },
        usage_complete: true,
        http_status: None,
        failure: None,
        pricing: Some(PricingSnapshot {
            currency: Currency::Usd,
            prices: TokenPrices {
                input_per_million_tokens: Some(0.1),
                ..Default::default()
            },
            input_token_tiers: Vec::new(),
            captured_at_ms: 0,
        }),
        reference_price: false,
    };
    record.validate().unwrap();
    let usd = BudgetResource::Cost {
        currency: Currency::Usd,
    };
    assert_eq!(usd.measured_units(&record).unwrap(), Some(1));
    assert_eq!(
        BudgetResource::Cost {
            currency: Currency::Cny
        }
        .measured_units(&record)
        .unwrap(),
        Some(0)
    );
    record.reference_price = true;
    assert_eq!(usd.measured_units(&record).unwrap(), Some(0));
    record.reference_price = false;
    record.pricing = None;
    assert_eq!(usd.measured_units(&record).unwrap(), None);
}

#[test]
fn scopes_match_exact_ids_and_never_infer_a_client() {
    let provider = BudgetScope::parse("provider", Some("provider-a")).unwrap();
    assert!(provider.matches("provider-a", None));
    assert!(!provider.matches("provider-a-copy", None));
    let client = BudgetScope::parse("gateway_client", Some("client-a")).unwrap();
    assert!(!client.matches("client-a", None));
    assert!(client.matches("provider-b", Some("client-a")));
    assert!(BudgetScope::parse("global", Some("extra")).is_err());
    assert!(BudgetScope::parse("provider", Some(" ")).is_err());
}

#[test]
fn invalid_policy_and_unknown_closed_set_values_are_refused() {
    let mut policy = budget();
    policy.limit = 0;
    assert!(policy.validate().is_err());
    policy = budget();
    policy.notify_percent = 101;
    assert!(policy.validate().is_err());
    policy = budget();
    policy.utc_offset_minutes = 1440;
    assert!(policy.validate().is_err());
    assert!(BudgetPeriod::parse("weekly").is_err());
    assert!(BudgetMode::parse("ignore").is_err());
    assert!(BudgetResource::parse("cost", None).is_err());
    for value in ["daily", "monthly"] {
        assert_eq!(BudgetPeriod::parse(value).unwrap().as_str(), value);
    }
    for value in ["notify", "stop"] {
        assert_eq!(BudgetMode::parse(value).unwrap().as_str(), value);
    }
    for value in [
        "exhausted",
        "unknown_bound",
        "unknown_usage",
        "counter_overflow",
    ] {
        assert_eq!(BudgetRefusal::parse(value).unwrap().as_str(), value);
    }
}
