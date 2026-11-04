#![cfg(test)]

use super::*;

#[test]
fn cost_bounds_cover_cache_splits_and_every_reachable_price_tier() {
    let mut price = pricing(Currency::Usd, 0.3);
    price.input_token_tiers = vec![
        InputPriceTier {
            min_input_tokens: 4,
            prices: TokenPrices {
                input_per_million_tokens: Some(2.0),
                output_per_million_tokens: Some(3.0),
                cache_read_per_million_tokens: Some(4.0),
                cache_write_per_million_tokens: Some(5.0),
            },
        },
        InputPriceTier {
            min_input_tokens: 8,
            prices: price.prices.clone(),
        },
    ];
    let bound = price.upper_bound(20, 5).unwrap().unwrap();
    assert_eq!(bound.currency, Currency::Usd);
    for input in 0..=20 {
        for read in 0..=input {
            for write in 0..=input - read {
                let mut observed = record("request", "attempt", AttemptStatus::Succeeded);
                observed.pricing = Some(price.clone());
                observed.usage = ObservedTokens::from_reported(
                    Some(input),
                    Some(5),
                    Some(read),
                    Some(write),
                    InputAccounting::IncludesCache,
                )
                .unwrap();
                let measured = crate::budget::local::BudgetResource::Cost {
                    currency: Currency::Usd,
                }
                .measured_units(&observed)
                .unwrap()
                .unwrap();
                assert!(
                    bound.micro_units >= measured,
                    "input={input} read={read} write={write}"
                );
            }
        }
    }
    assert!(bound.micro_units >= 235);
}

#[test]
fn unknown_prices_zero_lanes_and_bound_overflow_remain_distinct() {
    let mut price = pricing(Currency::Cny, 1.0);
    price.prices.cache_write_per_million_tokens = None;
    assert!(price.upper_bound(1, 1).unwrap().is_none());
    assert_eq!(price.upper_bound(0, 0).unwrap().unwrap().micro_units, 0);
    assert_eq!(
        price.upper_bound(0, 1).unwrap().unwrap().currency,
        Currency::Cny
    );
    price.prices.output_per_million_tokens = Some(f64::MAX);
    assert!(price.upper_bound(0, u64::MAX).is_err());
    price.prices.output_per_million_tokens = Some(-1.0);
    assert!(price.upper_bound(0, 1).is_err());
}

fn pricing(currency: Currency, input: f64) -> PricingSnapshot {
    PricingSnapshot {
        currency,
        prices: TokenPrices {
            input_per_million_tokens: Some(input),
            output_per_million_tokens: Some(10.0),
            cache_read_per_million_tokens: Some(0.5),
            cache_write_per_million_tokens: Some(4.0),
        },
        input_token_tiers: Vec::new(),
        captured_at_ms: 1,
    }
}

fn record(request: &str, attempt: &str, status: AttemptStatus) -> InvocationRecord {
    InvocationRecord {
        request_id: request.into(),
        attempt_id: attempt.into(),
        source_id: "aster-chat".into(),
        session_id: Some("session-1".into()),
        provider_id: "provider-a".into(),
        model_id: "model-a".into(),
        credential_id: Some("key-1".into()),
        response_model: None,
        status,
        started_at_ms: 1_000,
        ended_at_ms: 2_500,
        first_token_ms: Some(500),
        usage: ObservedTokens::from_reported(
            Some(1_000),
            Some(100),
            Some(800),
            Some(100),
            InputAccounting::IncludesCache,
        )
        .unwrap(),
        usage_complete: true,
        http_status: None,
        failure: None,
        pricing: Some(pricing(Currency::Usd, 2.0)),
        reference_price: false,
    }
}

#[test]
fn provider_cache_conventions_normalize_to_the_same_disjoint_usage() {
    let inclusive = ObservedTokens::from_reported(
        Some(1_000),
        Some(100),
        Some(800),
        Some(100),
        InputAccounting::IncludesCache,
    )
    .unwrap();
    let exclusive = ObservedTokens::from_reported(
        Some(100),
        Some(100),
        Some(800),
        Some(100),
        InputAccounting::ExcludesCache,
    )
    .unwrap();
    assert_eq!(inclusive, exclusive);
    assert_eq!(inclusive.no_cache_tokens, Some(100));
    assert_eq!(inclusive.total_tokens().unwrap(), Some(1_100));
    assert!(
        (pricing(Currency::Usd, 2.0)
            .estimate(&inclusive)
            .unwrap()
            .unwrap()
            - 0.002)
            .abs()
            < 1e-12
    );
}

#[test]
fn absent_measurements_are_not_zero_or_free() {
    let unknown = ObservedTokens::default();
    assert_eq!(unknown.total_tokens().unwrap(), None);
    assert_eq!(
        pricing(Currency::Usd, 0.0).estimate(&unknown).unwrap(),
        None
    );

    let partial = ObservedTokens::from_reported(
        Some(100),
        Some(10),
        None,
        Some(0),
        InputAccounting::ExcludesCache,
    )
    .unwrap();
    assert_eq!(partial.no_cache_tokens, Some(100));
    assert_eq!(partial.input_tokens, None);
    assert_eq!(partial.cache_read_tokens, None);
    assert_eq!(partial.total_tokens().unwrap(), None);
    assert_eq!(
        pricing(Currency::Usd, 2.0).estimate(&partial).unwrap(),
        None
    );

    let zero = ObservedTokens::from_reported(
        Some(0),
        Some(0),
        Some(0),
        Some(0),
        InputAccounting::IncludesCache,
    )
    .unwrap();
    assert_eq!(zero.total_tokens().unwrap(), Some(0));
    let no_rates = PricingSnapshot {
        prices: TokenPrices::default(),
        ..pricing(Currency::Usd, 2.0)
    };
    assert_eq!(no_rates.estimate(&zero).unwrap(), Some(0.0));
}

#[test]
fn invalid_cache_partitions_and_overflow_are_refused() {
    for (input, read, write) in [(100, 80, 30), (100, 101, 0), (u64::MAX, u64::MAX, 1)] {
        assert!(ObservedTokens::from_reported(
            Some(input),
            Some(0),
            Some(read),
            Some(write),
            InputAccounting::IncludesCache,
        )
        .is_err());
    }
    assert!(ObservedTokens::from_reported(
        Some(u64::MAX),
        Some(1),
        Some(0),
        Some(0),
        InputAccounting::IncludesCache,
    )
    .is_err());
}

#[test]
fn tier_thresholds_use_full_input_and_keep_the_historical_price() {
    let mut current = pricing(Currency::Usd, 2.0);
    current.input_token_tiers.push(InputPriceTier {
        min_input_tokens: 1_000,
        prices: TokenPrices {
            input_per_million_tokens: Some(20.0),
            ..current.prices.clone()
        },
    });
    current.validate().unwrap();
    let mut call = record("request-1", "attempt-1", AttemptStatus::Succeeded);
    call.pricing = Some(current.clone());
    current.input_token_tiers[0].prices.input_per_million_tokens = Some(200.0);
    assert!((call.estimated_cost().unwrap().unwrap() - 0.0038).abs() < 1e-12);
    let encoded = serde_json::to_string(&call).unwrap();
    let restored: InvocationRecord = serde_json::from_str(&encoded).unwrap();
    restored.validate().unwrap();
    assert_eq!(restored, call);
}

#[test]
fn pricing_rejects_nonfinite_negative_and_unordered_tiers() {
    for rate in [-1.0, f64::NAN, f64::INFINITY] {
        assert!(pricing(Currency::Usd, rate).validate().is_err());
    }
    let mut snapshot = pricing(Currency::Usd, 2.0);
    for threshold in [100, 100] {
        snapshot.input_token_tiers.push(InputPriceTier {
            min_input_tokens: threshold,
            prices: snapshot.prices.clone(),
        });
    }
    assert!(snapshot.validate().is_err());
}

#[test]
fn retries_count_one_request_and_keep_failed_attempt_spend() {
    let failed = record("request-1", "attempt-1", AttemptStatus::Failed);
    let succeeded = record("request-1", "attempt-2", AttemptStatus::Succeeded);
    let mut interrupted = record("request-2", "attempt-3", AttemptStatus::Interrupted);
    interrupted.usage = ObservedTokens::default();
    let totals =
        InvocationTotals::from_records([&failed, &succeeded, &interrupted, &failed]).unwrap();
    assert_eq!(totals.requests, 2);
    assert_eq!(totals.attempts, 3);
    assert_eq!(totals.known_total_tokens, 2_200);
    assert_eq!(totals.unknown_usage_attempts, 1);
    assert_eq!(totals.unpriced_attempts, 1);
    assert!((totals.costs[&Currency::Usd].estimated_cost - 0.004).abs() < 1e-12);
}

#[test]
fn conflicting_attempt_replay_is_refused() {
    let first = record("request-1", "attempt-1", AttemptStatus::Succeeded);
    let mut conflicting = first.clone();
    conflicting.request_id = "request-2".into();
    assert!(InvocationTotals::from_records([&first, &conflicting]).is_err());
}

#[test]
fn interrupted_cumulative_counters_are_visible_but_do_not_claim_final_consumption() {
    let mut partial = record("request-1", "attempt-1", AttemptStatus::Interrupted);
    partial.usage_complete = false;
    partial.validate().unwrap();
    assert_eq!(partial.usage.total_tokens().unwrap(), Some(1100));
    assert_eq!(partial.estimated_cost().unwrap(), None);
    assert_eq!(partial.output_tokens_per_second(), None);
    let totals = InvocationTotals::from_records([&partial]).unwrap();
    assert_eq!(totals.known_total_tokens, 0);
    assert_eq!(totals.unknown_usage_attempts, 1);
}

#[test]
fn currencies_and_subscription_reference_values_do_not_mix() {
    let usd = record("request-1", "attempt-1", AttemptStatus::Succeeded);
    let mut cny = record("request-2", "attempt-2", AttemptStatus::Succeeded);
    cny.pricing = Some(pricing(Currency::Cny, 2.0));
    let mut subscription = record("request-3", "attempt-3", AttemptStatus::Succeeded);
    subscription.reference_price = true;
    let totals = InvocationTotals::from_records([&usd, &cny, &subscription]).unwrap();
    assert_eq!(totals.costs.len(), 2);
    assert!((totals.costs[&Currency::Usd].estimated_cost - 0.002).abs() < 1e-12);
    assert!((totals.costs[&Currency::Usd].reference_value - 0.002).abs() < 1e-12);
    assert!((totals.costs[&Currency::Cny].estimated_cost - 0.002).abs() < 1e-12);
}

#[test]
fn speed_uses_only_observed_output_and_generation_time() {
    let mut call = record("request-1", "attempt-1", AttemptStatus::Succeeded);
    assert_eq!(call.output_tokens_per_second(), Some(100.0));
    call.first_token_ms = None;
    assert_eq!(call.output_tokens_per_second(), None);
    call.first_token_ms = Some(1_500);
    assert_eq!(call.output_tokens_per_second(), None);
    call.ended_at_ms = 900;
    assert!(call.validate().is_err());
}

#[test]
fn closed_sets_and_unknown_wire_fields_are_refused() {
    for value in ["includes_cache", "excludes_cache"] {
        assert_eq!(InputAccounting::parse(value).unwrap().as_str(), value);
    }
    for value in ["succeeded", "failed", "cancelled", "interrupted"] {
        assert_eq!(AttemptStatus::parse(value).unwrap().as_str(), value);
    }
    for value in ["USD", "CNY"] {
        assert_eq!(Currency::parse(value).unwrap().as_str(), value);
    }
    assert!(InputAccounting::parse("auto").is_err());
    assert!(AttemptStatus::parse("unknown").is_err());
    assert!(Currency::parse("usd").is_err());
    for value in [
        "transport",
        "upstream",
        "stream",
        "invalid_response",
        "cancelled",
        "interrupted",
    ] {
        assert_eq!(AttemptFailure::parse(value).unwrap().as_str(), value);
    }
    assert!(AttemptFailure::parse("guess").is_err());
    assert!(
        serde_json::from_str::<ObservedTokens>(r#"{"inputTokens":1,"apiKey":"secret"}"#).is_err()
    );
}

#[test]
fn invocation_start_owns_identity_and_price_through_cancelled_completion() {
    let original = record("request-1", "attempt-1", AttemptStatus::Cancelled);
    let mut start = InvocationStart {
        identity: InvocationIdentity {
            request_id: original.request_id.clone(),
            attempt_id: original.attempt_id.clone(),
            source_id: original.source_id.clone(),
            session_id: original.session_id.clone(),
            provider_id: original.provider_id.clone(),
            model_id: original.model_id.clone(),
            credential_id: original.credential_id.clone(),
        },
        gateway_client_id: Some("client-1".into()),
        started_at_ms: original.started_at_ms,
        pricing: original.pricing.clone(),
        reference_price: false,
        routing: None,
        bounds: InvocationBounds {
            tokens: Some(2000),
            cost: Some(CostBound {
                currency: Currency::Usd,
                micro_units: 10_000,
            }),
        },
    };
    start.validate().unwrap();
    let completed = start
        .complete(InvocationResult {
            status: original.status,
            ended_at_ms: original.ended_at_ms,
            first_token_ms: original.first_token_ms,
            usage: original.usage.clone(),
            usage_complete: true,
            http_status: None,
            failure: None,
            response_model: original.response_model.clone(),
        })
        .unwrap();
    assert_eq!(completed, original);
    start
        .pricing
        .as_mut()
        .unwrap()
        .prices
        .input_per_million_tokens = Some(200.0);
    assert_eq!(completed.pricing, original.pricing);
    start.bounds.cost.as_mut().unwrap().currency = Currency::Cny;
    assert!(start.validate().is_err());
    start.bounds.cost = None;
    start.identity.attempt_id.clear();
    assert!(start.validate().is_err());
}
