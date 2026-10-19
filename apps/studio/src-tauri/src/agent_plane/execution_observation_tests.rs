#![cfg(test)]

use super::*;
use serde_json::json;

fn packet(sequence: u64, kind: &str, turn: Option<u64>) -> Value {
    json!({"protocolVersion":1,"sessionId":"acp","promptId":"prompt","coreSessionId":"core",
        "runId":"run","sequence":sequence,"kind":kind,"turn":turn,"status":null})
}

#[test]
fn model_turns_are_owned_by_one_correlated_run_and_close_independently() {
    let mut observer = Observer::new("acp", "prompt");
    let run = observer
        .accept(&packet(1, "run-opened", None))
        .unwrap()
        .unwrap();
    assert_eq!(run.run_id, "run");
    assert_eq!(
        observer
            .accept(&packet(2, "model-turn-opened", Some(1)))
            .unwrap()
            .unwrap()
            .model_turn,
        Some(1)
    );
    assert_eq!(
        observer
            .accept(&packet(3, "model-turn-closed", Some(1)))
            .unwrap()
            .unwrap()
            .model_turn,
        None
    );
    assert_eq!(
        observer
            .accept(&packet(4, "model-turn-opened", Some(2)))
            .unwrap()
            .unwrap()
            .model_turn,
        Some(2)
    );
    let mut end = packet(5, "run-closed", None);
    end["status"] = json!("completed");
    assert!(observer.accept(&end).unwrap().is_none());
    assert!(observer.accept(&packet(1, "run-opened", None)).is_none());
}

#[test]
fn foreign_prompt_is_ignored_but_correlated_replay_or_gaps_revoke_the_owner() {
    for sequence in [1, 3] {
        let mut observer = Observer::new("acp", "prompt");
        observer
            .accept(&packet(1, "run-opened", None))
            .unwrap()
            .unwrap();
        let mut foreign = packet(2, "model-turn-opened", Some(1));
        foreign["promptId"] = json!("previous-prompt");
        assert!(observer.accept(&foreign).is_none());
        assert!(
            observer
                .accept(&packet(sequence, "model-turn-opened", Some(1)))
                .unwrap()
                .is_none()
        );
        assert!(
            observer
                .accept(&packet(2, "model-turn-opened", Some(1)))
                .is_none()
        );
    }
}

#[test]
fn changed_identity_invalid_terminal_and_unknown_fields_cannot_keep_a_run_active() {
    for (field, value) in [
        ("runId", json!("other-run")),
        ("coreSessionId", json!("other-session")),
        ("protocolVersion", json!(2)),
        ("kind", json!("invented")),
        ("turn", json!(0)),
        ("status", json!("executing")),
        ("resourceGrant", json!(true)),
    ] {
        let mut observer = Observer::new("acp", "prompt");
        observer
            .accept(&packet(1, "run-opened", None))
            .unwrap()
            .unwrap();
        let mut altered = packet(2, "model-turn-opened", Some(1));
        altered[field] = value;
        assert!(
            observer.accept(&altered).unwrap().is_none(),
            "field {field}"
        );
    }
}

#[test]
fn wrong_turn_end_and_overlapping_turns_refuse_instead_of_reparenting() {
    for invalid in [
        packet(3, "model-turn-closed", Some(2)),
        packet(3, "model-turn-opened", Some(2)),
    ] {
        let mut observer = Observer::new("acp", "prompt");
        observer
            .accept(&packet(1, "run-opened", None))
            .unwrap()
            .unwrap();
        observer
            .accept(&packet(2, "model-turn-opened", Some(1)))
            .unwrap()
            .unwrap();
        assert!(observer.accept(&invalid).unwrap().is_none());
    }
}

// Catch producer/consumer drift with packets retained from the actual Core -> ACP prompt transport.
#[test]
#[ignore = "requires A3S_CLOUD_TEST_AIX_EXECUTION_OBSERVATIONS from the ACP wire campaign"]
fn retained_actual_acp_runs_open_model_turns_and_withdraw_at_settlement() {
    let path = std::env::var_os("A3S_CLOUD_TEST_AIX_EXECUTION_OBSERVATIONS")
        .expect("explicit retained ACP observations");
    let bytes = std::fs::read(path).unwrap();
    assert!(
        bytes.len() <= 64 * 1024,
        "bounded retained protocol evidence"
    );
    let lines: Vec<Value> = std::str::from_utf8(&bytes)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(lines.len(), 14);
    assert_ne!(lines[0]["runId"], lines[4]["runId"]);
    assert_ne!(lines[0]["promptId"], lines[4]["promptId"]);
    assert_ne!(lines[4]["runId"], lines[8]["runId"]);
    for run in [&lines[..4], &lines[4..8], &lines[8..]] {
        let mut observer = Observer::new(
            run[0]["sessionId"].as_str().unwrap(),
            run[0]["promptId"].as_str().unwrap(),
        );
        for (index, packet) in run.iter().enumerate() {
            let snapshot = observer
                .accept(packet)
                .expect("actual ACP protocol admitted");
            if index == run.len() - 1 {
                assert!(snapshot.is_none(), "completed Run withdraws its UI owner");
            } else {
                let snapshot = snapshot.unwrap();
                assert_eq!(snapshot.run_id, packet["runId"].as_str().unwrap());
                assert_eq!(
                    snapshot.core_session_id,
                    packet["coreSessionId"].as_str().unwrap()
                );
                assert_eq!(snapshot.sequence, (index + 1) as u64);
                assert_eq!(
                    snapshot.model_turn,
                    match index {
                        1 => Some(1),
                        3 => Some(2),
                        _ => None,
                    }
                );
            }
        }
        assert!(
            observer.accept(&run[0]).is_none(),
            "settled owner cannot reopen from replay"
        );
    }
}
