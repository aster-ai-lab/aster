use super::*;
use serde_json::json;

fn document(text: &str) -> Map<String, Value> {
    json!({"messages":[{"role":"user","content":text}]})
        .as_object()
        .unwrap()
        .clone()
}

fn candidate(id: u128, load: u64) -> PrefixTarget {
    PrefixTarget {
        target_id: Uuid::from_u128(id),
        signature: Sha256::digest(id.to_le_bytes()).into(),
        load,
    }
}

#[test]
fn appended_history_reuses_the_longest_prefix_without_retaining_prompts() {
    let affinity = PrefixAffinity::default();
    let doc = document(&"reference ".repeat(4000));
    let hint = affinity.hint("scope", &doc).unwrap();
    let owner = candidate(1, 0);
    let now = Instant::now();
    affinity.accepted(&hint, owner.signature, now);
    let mut continuation = doc.clone();
    continuation["messages"].as_array_mut().unwrap().extend([
        json!({"role":"assistant","content":null,"tool_calls":[{"id":"call1","type":"function","function":{"name":"sum","arguments":"{\"a\":1}"}}]}),
        json!({"role":"tool","tool_call_id":"call1","content":"2"}),
        json!({"role":"user","content":"continue"}),
    ]);
    let next = affinity.hint("scope", &continuation).unwrap();
    let picked = affinity.select(&next, &[candidate(2, 0), owner], now);
    assert_eq!(picked.preferred, Some(Uuid::from_u128(1)));
    assert!(picked.matched_bytes > 30_000);
    assert!(std::mem::size_of::<Entry>() < 64);
}

#[test]
fn credentials_models_endpoints_and_process_salts_never_share_hints() {
    let affinity = PrefixAffinity::default();
    let doc = document(&"private ".repeat(2000));
    let now = Instant::now();
    let hint = affinity
        .hint(("credential-a", 1, "model-a", "chat"), &doc)
        .unwrap();
    affinity.accepted(&hint, candidate(1, 0).signature, now);
    for scope in [
        ("credential-b", 1, "model-a", "chat"),
        ("credential-a", 2, "model-a", "chat"),
        ("credential-a", 1, "model-b", "chat"),
        ("credential-a", 1, "model-a", "messages"),
    ] {
        let other = affinity.hint(scope, &doc).unwrap();
        assert_eq!(
            affinity.select(&other, &[candidate(1, 0)], now).preferred,
            None
        );
    }
    let unrelated = PrefixAffinity::default();
    let other = unrelated
        .hint(("credential-a", 1, "model-a", "chat"), &doc)
        .unwrap();
    assert_ne!(hint.checkpoints, other.checkpoints);
}

#[test]
fn replay_annotations_and_sampling_do_not_break_locality_but_tool_arguments_do() {
    let affinity = PrefixAffinity::default();
    let mut doc = document(&"same history ".repeat(1000));
    let before = affinity.hint("scope", &doc).unwrap();
    doc.insert("temperature".into(), json!(0.7));
    for name in [
        "agent",
        "conversationRequestId",
        "startsNewUserRequest",
        "messageId",
        "model",
        "requestModelId",
        "requestModelName",
        "stepSeq",
        "traceId",
        "argumentsDisplayText",
        "rawUsage",
        "usage",
    ] {
        doc["messages"][0][name] = json!({"untrusted":"changing annotation"});
    }
    let annotated = affinity.hint("scope", &doc).unwrap();
    assert_eq!(before.checkpoints, annotated.checkpoints);
    doc["messages"][0]["tool_calls"] = json!([{"id":"call1","function":{"arguments":"1"}}]);
    let one = affinity.hint("scope", &doc).unwrap();
    doc["messages"][0]["tool_calls"][0]["function"]["arguments"] = json!("2");
    let two = affinity.hint("scope", &doc).unwrap();
    assert_ne!(one.checkpoints.last(), two.checkpoints.last());
}

#[test]
fn busy_owner_yields_and_cold_traffic_uses_the_least_loaded_set() {
    let affinity = PrefixAffinity::default();
    let doc = document(&"data ".repeat(4000));
    let hint = affinity.hint("scope", &doc).unwrap();
    let now = Instant::now();
    affinity.accepted(&hint, candidate(1, 0).signature, now);
    let within_band = affinity.select(&hint, &[candidate(1, 1000), candidate(2, 0)], now);
    assert_eq!(within_band.preferred, Some(Uuid::from_u128(1)));
    let overloaded = affinity.select(&hint, &[candidate(1, 2000), candidate(2, 0)], now);
    assert_eq!(overloaded.preferred, None);
    assert_eq!(overloaded.least_loaded, vec![Uuid::from_u128(2)]);
    let cold = affinity.hint("other-credential", &doc).unwrap();
    let cold = affinity.select(
        &cold,
        &[candidate(1, 1000), candidate(2, 0), candidate(3, 0)],
        now,
    );
    assert_eq!(cold.preferred, None);
    assert_eq!(
        cold.least_loaded,
        vec![Uuid::from_u128(2), Uuid::from_u128(3)]
    );
}

#[test]
fn expired_removed_or_replaced_targets_cannot_supply_a_hint() {
    let affinity = PrefixAffinity::default();
    let hint = affinity
        .hint("scope", &document(&"a".repeat(9000)))
        .unwrap();
    let now = Instant::now();
    affinity.accepted(&hint, candidate(1, 0).signature, now);
    assert_eq!(
        affinity
            .select(&hint, &[candidate(1, 0)], now + TTL)
            .preferred,
        None
    );
    assert_eq!(
        affinity.select(&hint, &[candidate(2, 0)], now).preferred,
        None
    );
    let changed = PrefixTarget {
        signature: [9; 32],
        ..candidate(1, 0)
    };
    assert_eq!(affinity.select(&hint, &[changed], now).preferred, None);
}

#[test]
fn bounded_hashing_lru_and_ttl_survive_adversarial_unique_prompts() {
    let affinity = PrefixAffinity::default();
    let huge = document(&"界".repeat(2_000_000));
    let hint = affinity.hint("scope", &huge).unwrap();
    assert_eq!(hint.bytes, MAX_HASH_BYTES);
    assert!(hint.checkpoints.len() <= MAX_CHECKPOINTS);
    let now = Instant::now();
    for number in 0u64..5000 {
        let hint = affinity.hint(number, &document(&"x".repeat(1200))).unwrap();
        affinity.accepted(&hint, candidate(1, 0).signature, now);
    }
    let index = affinity.index.lock().unwrap();
    assert_eq!(index.entries.len(), MAX_ENTRIES);
    assert_eq!(index.lru.len(), MAX_ENTRIES);
}

#[test]
fn small_common_headers_do_not_pin_unrelated_long_prompts() {
    let affinity = PrefixAffinity::default();
    let mut first = document(&"x".repeat(40_000));
    first["messages"]
        .as_array_mut()
        .unwrap()
        .insert(0, json!({"role":"system","content":"header ".repeat(700)}));
    let now = Instant::now();
    affinity.accepted(
        &affinity.hint("scope", &first).unwrap(),
        candidate(1, 0).signature,
        now,
    );
    first["messages"][1]["content"] = json!("y".repeat(40_000));
    let other = affinity.hint("scope", &first).unwrap();
    assert_eq!(
        affinity
            .select(&other, &[candidate(1, 0), candidate(2, 0)], now)
            .preferred,
        None
    );
}

#[test]
fn explicit_cache_namespaces_remain_separate_and_a_clone_retains_locality() {
    let affinity = PrefixAffinity::default();
    let mut doc = document(&"private reference ".repeat(1000));
    doc.insert("prompt_cache_key".into(), json!("session-a"));
    let hint = affinity.hint("scope", &doc).unwrap();
    let now = Instant::now();
    affinity.accepted(&hint, candidate(1, 0).signature, now);
    let refresh = affinity.clone();
    let same = refresh.hint("scope", &doc).unwrap();
    assert_eq!(
        refresh.select(&same, &[candidate(1, 0)], now).preferred,
        Some(Uuid::from_u128(1))
    );
    doc["prompt_cache_key"] = json!("session-b");
    let separate = refresh.hint("scope", &doc).unwrap();
    assert_eq!(
        refresh.select(&separate, &[candidate(1, 0)], now).preferred,
        None
    );
}

#[test]
fn reservations_release_on_drop_and_count_before_connections_open() {
    let affinity = PrefixAffinity::default();
    let hint = affinity
        .hint("scope", &document(&"x".repeat(4096)))
        .unwrap();
    let now = Instant::now();
    let lease = affinity
        .reserve(&hint, candidate(1, 0).signature, 0, now)
        .unwrap();
    assert_eq!(
        affinity
            .select(&hint, &[candidate(1, 0), candidate(2, 0)], now)
            .least_loaded,
        vec![Uuid::from_u128(2)]
    );
    drop(lease);
    assert!(affinity.index.lock().unwrap().reservations.is_empty());
    assert_eq!(
        affinity
            .select(&hint, &[candidate(1, 0), candidate(2, 0)], now)
            .least_loaded
            .len(),
        2
    );
}

fn learn(affinity: &PrefixAffinity, hint: &PrefixHint, id: u128, prefill_ms: f64, now: Instant) {
    affinity
        .reserve(hint, candidate(id, 0).signature, 0, now)
        .unwrap()
        .complete(
            Some(PrefixTiming {
                prompt: 4000,
                cached: 0,
                output: 32,
                prefill_ms,
                decode_ms: 400.0,
            }),
            now,
        );
}

#[test]
fn measured_cost_prefers_faster_prefill_and_expires_without_rewriting_accounting() {
    let affinity = PrefixAffinity::default();
    let hint = affinity
        .hint("scope", &document(&"x".repeat(16000)))
        .unwrap();
    let cold = affinity
        .hint("other-scope", &document(&"y".repeat(16000)))
        .unwrap();
    let now = Instant::now();
    learn(&affinity, &hint, 1, 8000.0, now);
    learn(&affinity, &hint, 2, 1000.0, now);
    let picked = affinity.select(&cold, &[candidate(1, 0), candidate(2, 0)], now);
    assert_eq!(picked.preferred, Some(Uuid::from_u128(2)));
    assert!(picked.estimated_ms.is_some());
    let expired = affinity.select(&cold, &[candidate(1, 0), candidate(2, 0)], now + TTL);
    assert!(expired.preferred.is_none());
    assert!(expired.estimated_ms.is_none());
    assert_eq!(expired.least_loaded.len(), 2);
}

#[test]
fn token_work_budget_can_beat_request_count_but_never_the_load_band() {
    let affinity = PrefixAffinity::default();
    let hint = affinity
        .hint("scope", &document(&"x".repeat(16000)))
        .unwrap();
    let short = affinity
        .hint("short", &document(&"s".repeat(1200)))
        .unwrap();
    let now = Instant::now();
    learn(&affinity, &hint, 1, 8000.0, now);
    learn(&affinity, &hint, 2, 8000.0, now);
    let long_lease = affinity
        .reserve(&hint, candidate(1, 0).signature, 0, now)
        .unwrap();
    let short_lease = affinity
        .reserve(&short, candidate(2, 0).signature, 0, now)
        .unwrap();
    let picked = affinity.select(&short, &[candidate(1, 1000), candidate(2, 1000)], now);
    assert_eq!(picked.preferred, Some(Uuid::from_u128(2)));
    let guarded = affinity.select(&hint, &[candidate(1, 0), candidate(2, 3000)], now);
    assert_ne!(guarded.preferred, Some(Uuid::from_u128(2)));
    drop((long_lease, short_lease));
}

#[test]
fn native_cache_miss_invalidates_prior_hints_and_invalid_rates_do_not_train() {
    let affinity = PrefixAffinity::default();
    let old = affinity
        .hint("scope", &document(&"x".repeat(12000)))
        .unwrap();
    let new = affinity
        .hint("scope", &document(&"y".repeat(12000)))
        .unwrap();
    let now = Instant::now();
    affinity.accepted(&old, candidate(1, 0).signature, now);
    affinity
        .reserve(&new, candidate(1, 0).signature, 4096, now)
        .unwrap()
        .complete(
            Some(PrefixTiming {
                prompt: 3000,
                cached: 0,
                output: 4,
                prefill_ms: 2000.0,
                decode_ms: 50.0,
            }),
            now,
        );
    assert!(affinity.select(&old, &[candidate(1, 0)], now).matched_bytes == 0);
    assert!(affinity.select(&new, &[candidate(1, 0)], now).matched_bytes > 0);
    let invalid = PrefixAffinity::default();
    learn(&invalid, &new, 1, f64::NAN, now);
    assert!(invalid.index.lock().unwrap().profiles.is_empty());
    let enormous = invalid
        .hint("scope", &document(&"界".repeat(200000)))
        .unwrap();
    assert_eq!(enormous.bytes, MAX_HASH_BYTES);
    assert!(enormous.input_bytes > enormous.bytes);
}

#[test]
fn declared_parallel_slots_keep_a_busy_warm_owner_when_another_slot_is_free() {
    let affinity = PrefixAffinity::default();
    let mut hint = affinity
        .hint("scope", &document(&"x".repeat(16000)))
        .unwrap();
    hint.parallelism = 2;
    let now = Instant::now();
    learn(&affinity, &hint, 1, 16000.0, now);
    let lease = affinity
        .reserve(&hint, candidate(1, 0).signature, 0, now)
        .unwrap();
    let picked = affinity.select(&hint, &[candidate(1, 1000), candidate(2, 0)], now);
    assert_eq!(picked.preferred, Some(Uuid::from_u128(1)));
    drop(lease);
}
