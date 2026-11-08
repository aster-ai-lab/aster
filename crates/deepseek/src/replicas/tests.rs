use super::*;

#[test]
fn declared_replica_layouts_partition_all_eight_cards_without_overlapping_owners() {
    for layout in [XqReplicaLayout::TwoByFour, XqReplicaLayout::FourByTwo] {
        assert_eq!(XqReplicaLayout::parse(layout.as_str()).unwrap(), layout);
        let maps = layout.mappings();
        assert_eq!(maps.len(), 8 / layout.width().device_count());
        assert_eq!(
            maps.iter()
                .flat_map(|map| map.ordinals().iter().copied())
                .collect::<Vec<_>>(),
            (0..8).collect::<Vec<_>>()
        );
    }
    assert!(XqReplicaLayout::parse("8x1").is_err());
}

#[tokio::test]
async fn dropped_response_cancels_the_actual_request_and_missing_receipts_are_errors() {
    let cancellation = CancellationToken::new();
    let observed = cancellation.clone();
    let (sender, receiver) = output::channel(1);
    let timer = tokio::spawn(std::future::pending());
    let mut generation = NativeGeneration {
        replica: 1,
        receiver,
        cancellation,
        timed_out: Arc::new(AtomicBool::new(false)),
        timer,
        terminal: false,
    };
    sender
        .send(Ok(NativeGenerationEvent::Token(42)))
        .await
        .unwrap_or_else(|_| panic!("receiver lives"));
    assert!(matches!(
        generation.next().await,
        Some(Ok(NativeGenerationEvent::Token(42)))
    ));
    drop(sender);
    assert!(matches!(
        generation.next().await,
        Some(Err(PowerError::InferenceFailed(_)))
    ));
    assert!(generation.next().await.is_none());
    drop(generation);
    assert!(observed.is_cancelled());
}

#[test]
fn cancelled_or_dropped_work_returns_its_bounded_capacity_permit() {
    let pending = Arc::new(AtomicUsize::new(1));
    let (sender, receiver) = output::channel(1);
    let cancellation = CancellationToken::new();
    let work = Work {
        prompt: vec![0],
        maximum: 1,
        cancellation: cancellation.clone(),
        deadline: Instant::now() + Duration::from_secs(1),
        output: sender,
        _pending: Pending(pending.clone()),
    };
    cancellation.cancel();
    assert!(matches!(
        send(&work, NativeGenerationEvent::Token(42)),
        Err(PowerError::InferenceCancelled)
    ));
    assert!(receiver.is_empty());
    drop(work);
    assert_eq!(pending.load(Ordering::Acquire), 0);
}
