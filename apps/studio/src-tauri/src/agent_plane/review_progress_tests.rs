#![cfg(test)]

use super::*;

#[test]
fn review_progress_is_one_bounded_host_part_and_never_a_finding() {
    let mut stream = TurnStream::new("review");
    stream.review_document("/tmp/contract.docx", &"a".repeat(64));
    let first = stream.review_progress(0, 2, &"合".repeat(100));
    let part = first.last().unwrap();
    assert_eq!(part["data"]["index"], 1);
    assert_eq!(part["data"]["quote"].as_str().unwrap().chars().count(), 72);
    let next = stream.review_progress(1, 2, "\n最后一段。");
    assert_eq!(next.last().unwrap()["data"]["quote"], "最后一段。");
    let progress: Vec<_> = stream
        .parts
        .iter()
        .filter(|part| part["type"] == "data-review-progress")
        .collect();
    assert_eq!(progress.len(), 1);
    assert_eq!(progress[0]["data"]["index"], 2);
    assert!(!stream.parts.iter().any(|part| part["type"] == "text"));
}
