#![cfg(test)]

use super::*;

fn observation(run: &str, sequence: u64, turn: Option<u64>) -> Value {
    json!({"protocolVersion":1,"promptId":"prompt-1","coreSessionId":"core-1",
        "runId":run,"sequence":sequence,"modelTurn":turn})
}

fn status(hub: &EventHub, topic: &str, execution: Option<Value>) {
    let mut status =
        json!({"status":"streaming","activeExecutions":[],"awaitingApprovalAnchors":[]});
    if let Some(execution) = execution {
        status["nativeExecution"] = execution;
    } else {
        status["status"] = json!("done");
    }
    hub.publish("topic.status", json!({"topicId":topic,"status":status}));
}

async fn next(stream: &mut SseStream) -> Value {
    let event = tokio::time::timeout(std::time::Duration::from_secs(2), stream.next())
        .await
        .expect("event deadline")
        .expect("event stream open")
        .expect("event encoded");
    let encoded = String::from_utf8(event.encode()).expect("SSE UTF-8");
    serde_json::from_str(encoded.trim().strip_prefix("data: ").expect("SSE data"))
        .expect("SSE JSON")
}

// Catch opening a view during an active Run, or settlement between subscription and first poll.
#[tokio::test]
async fn snapshot_precedes_buffered_settlement_and_reconnect_withdraws_the_retired_run() {
    let hub = Arc::new(EventHub::new());
    status(
        &hub,
        "agent-session:one",
        Some(observation("run-1", 2, Some(1))),
    );
    status(
        &hub,
        "agent-session:two",
        Some(observation("run-2", 1, None)),
    );
    let mut connected = stream(hub.clone());
    status(&hub, "agent-session:one", None);
    let snapshot = next(&mut connected).await;
    assert_eq!(snapshot["channel"], "execution.snapshot");
    assert_eq!(snapshot["payload"]["topics"].as_array().unwrap().len(), 2);
    assert_eq!(
        snapshot["payload"]["topics"][0]["execution"]["modelTurn"],
        1
    );
    let terminal = next(&mut connected).await;
    assert_eq!(terminal["payload"]["topicId"], "agent-session:one");
    assert!(
        terminal["payload"]["status"]
            .get("nativeExecution")
            .is_none()
    );

    let mut reconnected = stream(hub);
    let snapshot = next(&mut reconnected).await;
    assert_eq!(snapshot["payload"]["topics"].as_array().unwrap().len(), 1);
    assert_eq!(
        snapshot["payload"]["topics"][0]["topicId"],
        "agent-session:two"
    );
}

// Catch broadcast loss retaining a settled owner or applying a newer delta before an older snapshot.
#[tokio::test]
async fn lag_invalidates_then_resubscribes_with_current_owners_before_new_deltas() {
    let hub = Arc::new(EventHub::new());
    status(&hub, "agent-session:old", Some(observation("old", 1, None)));
    let mut connected = stream(hub.clone());
    next(&mut connected).await;
    for _ in 0..=EVENT_BUFFER {
        hub.publish("data.changed", json!({"sessionId":"old"}));
    }
    status(&hub, "agent-session:old", None);
    status(&hub, "agent-session:new", Some(observation("new", 1, None)));
    assert_eq!(
        next(&mut connected).await["channel"],
        "execution.invalidated"
    );
    status(
        &hub,
        "agent-session:new",
        Some(observation("new", 2, Some(1))),
    );
    let snapshot = next(&mut connected).await;
    assert_eq!(snapshot["payload"]["topics"].as_array().unwrap().len(), 1);
    assert_eq!(
        snapshot["payload"]["topics"][0]["topicId"],
        "agent-session:new"
    );
    assert_eq!(snapshot["payload"]["topics"][0]["execution"]["sequence"], 1);
    assert_eq!(
        next(&mut connected).await["payload"]["status"]["nativeExecution"]["sequence"],
        2
    );
}

#[tokio::test]
async fn snapshot_lists_only_live_topic_statuses() {
    let hub = Arc::new(EventHub::new());
    status(
        &hub,
        "agent-session:live",
        Some(observation("run-live", 1, None)),
    );
    hub.publish(
        "topic.status",
        json!({"topicId":"agent-session:idle","status":{"status":"done","activeExecutions":[]}}),
    );
    let mut connected = stream(hub);
    let snapshot = next(&mut connected).await;
    assert_eq!(snapshot["channel"], "execution.snapshot");
    let live = snapshot["payload"]["liveStatuses"]
        .as_array()
        .expect("liveStatuses");
    assert_eq!(live.len(), 1);
    assert_eq!(live[0]["topicId"], "agent-session:live");
    assert_eq!(live[0]["status"]["status"], "streaming");
}

// Catch actual Core/ACP identities changing or failing to withdraw while crossing the SSE encoder.
#[tokio::test]
#[ignore = "requires retained actual ACP packets and an explicit SSE artifact output"]
async fn retained_core_runs_cross_the_native_observer_and_real_sse_encoder() {
    use super::super::execution_observation::Observer;

    let input = std::env::var("A3S_CLOUD_TEST_AIX_EXECUTION_OBSERVATIONS").expect("ACP input");
    let output = std::env::var("A3S_CLOUD_TEST_AIX_EXECUTION_SSE_OUTPUT").expect("SSE output");
    let bytes = std::fs::read_to_string(input).expect("retained ACP artifact");
    let hub = Arc::new(EventHub::new());
    let mut connected = stream(hub.clone());
    let mut retained = vec![next(&mut connected).await];
    let mut observers = BTreeMap::new();
    let mut packets = 0;
    let mut closed = 0;
    for line in bytes.lines() {
        let packet: Value = serde_json::from_str(line).expect("ACP packet");
        let session = packet["sessionId"].as_str().expect("ACP session");
        let prompt = packet["promptId"].as_str().expect("ACP prompt");
        let observer = observers
            .entry((session.to_owned(), prompt.to_owned()))
            .or_insert_with(|| Observer::new(session, prompt));
        let observed = observer.accept(&packet).expect("correlated actual packet");
        if packet["kind"] == "run-closed" {
            assert!(observed.is_none());
            closed += 1;
        } else {
            let value = serde_json::to_value(observed.as_ref().expect("active observation"))
                .expect("observation wire");
            assert_eq!(value["runId"], packet["runId"]);
            assert_eq!(value["coreSessionId"], packet["coreSessionId"]);
            assert_eq!(value["sequence"], packet["sequence"]);
        }
        status(
            &hub,
            &format!("agent-session:{session}"),
            observed.map(|v| json!(v)),
        );
        retained.push(next(&mut connected).await);
        packets += 1;
    }
    assert_eq!(packets, 14);
    assert_eq!(closed, 3);
    let mut reconnected = stream(hub);
    let final_snapshot = next(&mut reconnected).await;
    assert_eq!(final_snapshot["payload"]["topics"], json!([]));
    retained.push(final_snapshot);
    let encoded: Vec<String> = retained
        .iter()
        .map(|value| serde_json::to_string(value).unwrap())
        .collect();
    std::fs::write(output, encoded.join("\n") + "\n").expect("retained SSE artifact");
}
