use super::*;

#[test]
fn zero_rank_never_creates_or_changes_an_entry() {
    let mut fusion = RankFusion::<_, _, f64>::new();
    fusion.add("absent".into(), 0, 0, 9, |_, _| panic!("unadmitted merge"));
    fusion.add("present".into(), 0, 4, 1, |_, _| unreachable!());
    fusion.add("present".into(), 1, 0, 9, |_, _| panic!("unadmitted merge"));
    let entries = fusion.into_entries().collect::<Vec<_>>();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].0, "present");
    assert_eq!(entries[0].1.score(), 0.015625);
}

#[test]
fn each_channel_contributes_once_and_keeps_its_first_rank() {
    let mut fusion = RankFusion::<_, _, f64>::new();
    fusion.add("record".into(), "lexical", 4, 10, |_, _| unreachable!());
    fusion.add("record".into(), "lexical", 1, 100, |_, _| {
        panic!("duplicate merge")
    });
    fusion.add("record".into(), "semantic", 4, 5, |current, next| {
        *current += next
    });
    let (_, entry) = fusion.into_entries().next().unwrap();
    assert_eq!(entry.score(), 0.03125);
    assert_eq!(entry.rank("lexical").unwrap().get(), 4);
    assert_eq!(entry.rank("semantic").unwrap().get(), 4);
    assert!(entry.rank("structural").is_none());
    let (value, ranks) = entry.into_parts();
    assert_eq!(value, 15);
    assert_eq!(ranks.len(), 2);
}

#[test]
fn scalar_width_and_channel_arrival_order_are_preserved() {
    let mut fusion = RankFusion::<_, _, f32>::new();
    fusion.add("record".into(), 2, 4, (), |_, _| {});
    fusion.add("record".into(), 1, 4, (), |_, _| {});
    let (_, entry) = fusion.into_entries().next().unwrap();
    assert_eq!(entry.score().to_bits(), 0.03125_f32.to_bits());
    let (_, ranks) = entry.into_parts();
    assert_eq!(
        ranks
            .iter()
            .map(|(channel, _)| *channel)
            .collect::<Vec<_>>(),
        [2, 1]
    );
}

#[test]
fn largest_rank_saturates_the_denominator_without_overflow() {
    let mut fusion = RankFusion::<_, _, f64>::new();
    fusion.add("record".into(), 0, usize::MAX, (), |_, _| {});
    let (_, entry) = fusion.into_entries().next().unwrap();
    assert!(entry.score().is_finite() && entry.score() > 0.0);
    assert_eq!(entry.rank(0).unwrap().get(), usize::MAX);
}

#[test]
fn empty_input_stays_empty() {
    let fusion = RankFusion::<(), u8, f64>::new();
    assert_eq!(fusion.values().count(), 0);
    assert_eq!(fusion.into_entries().count(), 0);
}

#[test]
fn native_scores_share_deduplication_payload_and_rank_admission() {
    let mut fusion = RankFusion::<_, _, f32>::new();
    fusion.add_with_score("record".into(), 0, 0, Some(1.0), 9, |_, _| unreachable!());
    assert_eq!(fusion.values().count(), 0);
    fusion.add_with_score("record".into(), 0, 4, Some(0.25), 1, |_, _| unreachable!());
    fusion.add_with_score("record".into(), 0, 1, Some(1.0), 9, |_, _| {
        panic!("duplicate native score must not merge")
    });
    fusion.add_with_score("record".into(), 1, 8, Some(0.5), 2, |current, next| {
        *current += next
    });
    let (_, entry) = fusion.into_entries().next().unwrap();
    assert_eq!(entry.score(), 0.75);
    assert_eq!(entry.rank(0).unwrap().get(), 4);
    assert_eq!(entry.rank(1).unwrap().get(), 8);
    assert_eq!(entry.into_parts().0, 3);
}
