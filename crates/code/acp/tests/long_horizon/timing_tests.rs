use super::*;

#[test]
fn spans_retain_monotonic_intervals_parallel_overlap_and_shutdown_records() {
    let root = tempfile::tempdir().unwrap();
    let (state, worker) = State::open(root.path()).unwrap();
    let outer = Span::new(Some(state.clone()), Phase::CampaignPrepare, None);
    let barrier = Arc::new(std::sync::Barrier::new(3));
    let threads: Vec<_> = [1, 2]
        .into_iter()
        .map(|request| {
            let state = state.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                let _span = Span::new(
                    Some(state),
                    Phase::ModelRequest,
                    Some(Subject::Request(request)),
                );
                barrier.wait();
                std::thread::sleep(std::time::Duration::from_millis(5));
            })
        })
        .collect();
    barrier.wait();
    for thread in threads {
        thread.join().unwrap();
    }
    drop(outer);
    state.write("close", 0, Phase::TimingClose, None, None);
    drop(worker);
    let text = std::fs::read_to_string(root.path().join("phase-timing.jsonl")).unwrap();
    let rows: Vec<serde_json::Value> = text
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(rows.last().unwrap()["event"], "close");
    assert!(rows.iter().all(|row| row["dropped_records"] == 0));
    let begins: Vec<_> = rows.iter().filter(|row| row["event"] == "begin").collect();
    assert_eq!(begins.len(), 3);
    for begin in &begins {
        let end = rows
            .iter()
            .find(|row| row["event"] == "end" && row["span"] == begin["span"])
            .unwrap();
        assert!(end["elapsed_us"].as_u64().unwrap() >= begin["elapsed_us"].as_u64().unwrap());
        assert!(end["duration_us"].as_u64().unwrap() > 0);
        assert_eq!(end["subject"], begin["subject"]);
        assert_eq!(end["pid"], std::process::id());
    }
    let model_begins: Vec<_> = begins
        .into_iter()
        .filter(|row| row["phase"] == "model_request")
        .collect();
    let latest_start = model_begins
        .iter()
        .map(|row| row["elapsed_us"].as_u64().unwrap())
        .max()
        .unwrap();
    let first_end = rows
        .iter()
        .filter(|row| row["event"] == "end" && row["phase"] == "model_request")
        .map(|row| row["elapsed_us"].as_u64().unwrap())
        .min()
        .unwrap();
    assert!(
        latest_start < first_end,
        "parallel waits must remain overlapping intervals"
    );
}

#[test]
fn sink_creation_refuses_overwriting_previous_timing_evidence() {
    let root = tempfile::tempdir().unwrap();
    let original = b"previous evidence\n";
    std::fs::write(root.path().join("phase-timing.jsonl"), original).unwrap();
    assert!(State::open(root.path()).is_err());
    assert_eq!(
        std::fs::read(root.path().join("phase-timing.jsonl")).unwrap(),
        original
    );
}

#[test]
fn unwinding_closes_the_observed_scope_without_claiming_task_success() {
    let root = tempfile::tempdir().unwrap();
    let (state, worker) = State::open(root.path()).unwrap();
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _span = Span::new(Some(state.clone()), Phase::Prompt, None);
        panic!("synthetic failed operation");
    }));
    drop(worker);
    let rows: Vec<serde_json::Value> =
        std::fs::read_to_string(root.path().join("phase-timing.jsonl"))
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
    assert_eq!(rows.iter().filter(|row| row["event"] == "end").count(), 1);
    assert!(rows.iter().all(|row| row.get("task_pass").is_none()));
}
