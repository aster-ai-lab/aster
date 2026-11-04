use super::*;
use chrono::TimeZone as _;

fn now() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 11, 1, 0, 0, 0).single().unwrap()
}

fn item(id: &str, importance: f32, days: i64) -> MemoryItem {
    let mut item = MemoryItem::new(id).with_importance(importance);
    item.id = id.into();
    item.timestamp = now() - chrono::Duration::days(days);
    item
}

#[test]
fn admission_refuses_nonfinite_thresholds_and_overflow() {
    for threshold in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let error = ValidatedPrunePolicy::try_new(
            &PrunePolicy {
                min_importance_to_keep: threshold,
                ..Default::default()
            },
            now(),
        )
        .unwrap_err();
        assert_eq!(error.field(), "min_importance_to_keep");
    }
    let error = ValidatedPrunePolicy::try_new(
        &PrunePolicy {
            max_age_days: u32::MAX,
            ..Default::default()
        },
        now(),
    )
    .unwrap_err();
    assert_eq!(error.field(), "max_age_days");
}

#[test]
fn strict_age_and_importance_boundaries_keep_the_original_meaning() {
    let policy = ValidatedPrunePolicy::try_new(&PrunePolicy::default(), now()).unwrap();
    let items = vec![
        item("expired", 0.1, 91),
        item("age-boundary", 0.1, 90),
        item("importance-boundary", 0.5, 91),
        item("recent", 0.1, 1),
    ];
    assert_eq!(policy.select(&items).unwrap(), ["expired"]);
    let outside_unit = ValidatedPrunePolicy::try_new(
        &PrunePolicy {
            min_importance_to_keep: 1.5,
            ..Default::default()
        },
        now(),
    )
    .unwrap();
    assert_eq!(
        outside_unit
            .select(&[item("finite-sdk-threshold", 0.9, 91)])
            .unwrap(),
        ["finite-sdk-threshold"]
    );
}

#[test]
fn explicit_protection_is_exempt_from_both_age_and_quota() {
    let mut items = Vec::new();
    for tag in ["keep", "pinned", "protected", "consolidated", "conflict"] {
        items.push(item(tag, 0.1, 400).with_tag(tag));
    }
    for key in [
        "keep",
        "pinned",
        "protected",
        "supersedes",
        "conflicts_with",
    ] {
        let mut record = item(&format!("metadata-{key}"), 0.1, 400);
        record.metadata.insert(
            key.into(),
            if key == "supersedes" || key == "conflicts_with" {
                "source-id".into()
            } else {
                "true".into()
            },
        );
        items.push(record);
    }
    let mut accessed = item("accessed", 0.1, 400);
    accessed.access_count = 3;
    items.push(accessed);
    items.push(item("expired", 0.1, 400));
    items.push(item("over-quota", 0.9, 1));
    let policy = ValidatedPrunePolicy::try_new(
        &PrunePolicy {
            max_items: 1,
            ..Default::default()
        },
        now(),
    )
    .unwrap();
    assert_eq!(policy.select(&items).unwrap(), ["expired", "over-quota"]);
}

#[test]
fn quota_uses_existing_relevance_and_stable_input_ties() {
    let policy = ValidatedPrunePolicy::try_new(
        &PrunePolicy {
            max_items: 2,
            ..Default::default()
        },
        now(),
    )
    .unwrap();
    let items = vec![
        item("first-tie", 0.8, 1),
        item("lower", 0.1, 1),
        item("second-tie", 0.8, 1),
        item("last-tie", 0.8, 1),
    ];
    assert_eq!(policy.select(&items).unwrap(), ["last-tie", "lower"]);
}

#[test]
fn record_importance_is_validated_before_a_plan_can_be_published() {
    let policy = ValidatedPrunePolicy::try_new(&PrunePolicy::default(), now()).unwrap();
    for importance in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let mut invalid = item("invalid", 0.1, 400);
        invalid.importance = importance;
        let error = policy
            .select(&[item("expired", 0.1, 400), invalid])
            .unwrap_err();
        assert_eq!(error.field(), "importance");
    }
}
