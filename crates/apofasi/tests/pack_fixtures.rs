//! Packing golden fixtures (ByteTokenizer layout invariants).
//!
//! These fixtures lock the sequence contract without needing a neural
//! checkpoint. HF tokenizer parity lives in `pack_hf_parity` (opt-in).

use a3s_apofasi::{
    pack_question, pack_question_with_truncation, ByteTokenizer, Criteria, DecisionKind, Question,
    SequenceConfig, SpecialTokens, State, StateTruncation,
};
use indexmap::IndexMap;
use serde_json::json;

fn specials() -> SpecialTokens {
    SpecialTokens {
        cls: 0,
        sep: 1,
        mask: 2,
        pad: 3,
    }
}

#[test]
fn fixture_choice_markers_and_specials() {
    let mut opts = IndexMap::new();
    opts.insert("billing".into(), Some(json!("refunds")));
    opts.insert("tech".into(), Some(json!("bugs")));
    let q = Question::new(
        DecisionKind::Choice,
        json!("Which department?"),
        Some(Criteria::Choice(opts)),
    )
    .unwrap();
    let state = State::Text("Please refund my invoice.".into());
    let packed = pack_question(
        &ByteTokenizer,
        specials(),
        "[MASK]",
        &state,
        "department",
        &q,
        SequenceConfig::default(),
    )
    .unwrap();

    assert_eq!(packed.input_ids[0], 0, "must start with CLS");
    assert_eq!(packed.markers.len(), 2);
    assert_eq!(packed.option_labels, ["billing", "tech"]);
    for &m in &packed.markers {
        assert_eq!(packed.input_ids[m], 2, "marker must be MASK");
    }
    assert_eq!(*packed.input_ids.last().unwrap(), 1, "must end with SEP");
    // Head SEP sits between instructions and the first option marker.
    assert!(packed.markers[0] > 1);
    assert!(packed.input_ids[..packed.markers[0]].contains(&1));
}

#[test]
fn fixture_noul_false_true_order() {
    let q = Question::new(DecisionKind::Noul, json!("Refund requested?"), None).unwrap();
    let packed = pack_question(
        &ByteTokenizer,
        specials(),
        "[MASK]",
        &State::Text("Please refund.".into()),
        "refund",
        &q,
        SequenceConfig::default(),
    )
    .unwrap();
    assert_eq!(packed.option_labels, ["false", "true"]);
    assert_eq!(packed.qtype, 2);
    assert_eq!(packed.markers.len(), 2);
}

#[test]
fn fixture_score_levels_are_indexed() {
    let q = Question::new(
        DecisionKind::Score,
        json!("Urgency"),
        Some(Criteria::Score(vec![json!("low"), json!("high")])),
    )
    .unwrap();
    let packed = pack_question(
        &ByteTokenizer,
        specials(),
        "[MASK]",
        &State::Text("ASAP".into()),
        "urgency",
        &q,
        SequenceConfig {
            max_len: 256,
            head_max_len: 96,
            option_cap: 24,
        },
    )
    .unwrap();
    assert_eq!(packed.option_labels, ["0", "1"]);
    assert!(packed.input_ids.len() <= 256);
}

fn score_question() -> Question {
    Question::new(
        DecisionKind::Score,
        json!("Urgency"),
        Some(Criteria::Score(vec![json!("low"), json!("high")])),
    )
    .unwrap()
}

fn small_sequence() -> SequenceConfig {
    SequenceConfig {
        max_len: 96,
        head_max_len: 32,
        option_cap: 48,
    }
}

#[test]
fn state_budget_boundary_is_exact() {
    let base = pack_question(
        &ByteTokenizer,
        specials(),
        "[MASK]",
        &State::Text("x".into()),
        "urgency",
        &score_question(),
        small_sequence(),
    )
    .unwrap();
    let room = small_sequence().max_len - base.input_ids.len() + 1;
    for (len, accepted) in [(room - 1, true), (room, true), (room + 1, false)] {
        let result = pack_question_with_truncation(
            &ByteTokenizer,
            specials(),
            "[MASK]",
            &State::Text("x".repeat(len)),
            "urgency",
            &score_question(),
            small_sequence(),
            StateTruncation::Reject,
        );
        assert_eq!(result.is_ok(), accepted, "state length {len}, room {room}");
    }
}

#[test]
fn reject_reports_state_overflow() {
    let error = pack_question_with_truncation(
        &ByteTokenizer,
        specials(),
        "[MASK]",
        &State::Text("old history old history old history old history and the latest task".into()),
        "urgency",
        &score_question(),
        small_sequence(),
        StateTruncation::Reject,
    )
    .unwrap_err();
    assert!(matches!(
        error,
        a3s_apofasi::Error::StateOverflow { max_len: 96, .. }
    ));
}

#[test]
fn recent_state_is_preserved_and_marked_when_truncated() {
    let packed = pack_question_with_truncation(
        &ByteTokenizer,
        specials(),
        "[MASK]",
        &State::Text("old history old history old history old history and the latest task".into()),
        "urgency",
        &score_question(),
        small_sequence(),
        StateTruncation::TailAndMark,
    )
    .unwrap();
    assert!(packed.state_truncated);
    let text: String = packed
        .input_ids
        .iter()
        .filter_map(|id| {
            id.checked_sub(16)
                .and_then(|code| char::from_u32(code % 50_000))
        })
        .collect();
    assert!(text.contains("history is partial"), "{text}");
    assert!(text.contains("latest task"), "{text}");
}
