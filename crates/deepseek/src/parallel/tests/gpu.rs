//! Real eight-device peer qualification. No model/throughput claim.
use super::*;
use aster_power::inference::{DevicePreference, RuntimeDevice};

#[test]
#[ignore = "requires an owned idle two-H200 replica group; bind its exact GPU UUID pair before execution"]
fn h200x2_xq_replica_local_peer_cache_preserves_bits_and_step_isolation() {
    let limits = InferenceLimits::default();
    let runtimes: Vec<_> = (0..2)
        .map(|ordinal| RuntimeDevice::resolve(DevicePreference::Cuda { ordinal }).unwrap())
        .collect();
    let devices: Vec<_> = runtimes
        .iter()
        .cloned()
        .map(|runtime| CudaLayerDevice::new(runtime, limits.clone()).unwrap())
        .collect();
    let mut identities = BTreeSet::new();
    for (ordinal, device) in devices.iter().enumerate() {
        let stream = device
            .tensor_device()
            .as_cuda_device()
            .unwrap()
            .cuda_stream();
        let context = stream.context();
        assert_eq!(context.compute_capability().unwrap(), (9, 0));
        assert!(context.total_mem().unwrap() >= 128 * 1024 * 1024 * 1024);
        let uuid = context.uuid().unwrap().bytes.map(|value| value as u8);
        assert!(identities.insert(uuid));
        println!("replica_gpu local_ordinal={ordinal} uuid={uuid:?}");
    }
    let placement = XqPlacement::balanced(XqReplicaWidth::Two);
    let edges = placement.peer_edges(&architecture()).unwrap();
    assert_eq!(edges, [(0, 1)]);
    let group = CudaPeerGroup::new(&runtimes, &edges, limits.clone()).unwrap();
    let source: Vec<f32> = (0..32)
        .map(|value| {
            if value % 9 == 0 {
                -0.
            } else {
                value as f32 / 8.
            }
        })
        .collect();
    let original = Tensor::from_vec(source.clone(), (4, 8), devices[0].tensor_device()).unwrap();
    let view = original.narrow(1, 1, 3).unwrap().t().unwrap();
    let expected: Vec<_> = (1..4)
        .flat_map(|column| (0..4).map(move |row| row * 8 + column))
        .map(|index| source[index].to_bits())
        .collect();
    let cancel = CancellationToken::new();
    let mut transfers = XqStepTransfers::new(&group, &limits, 65536, 16).unwrap();
    let copy = transfers.copy_frozen(&view, &devices[1], &cancel).unwrap();
    let again = transfers.copy_frozen(&view, &devices[1], &cancel).unwrap();
    assert_eq!(copy.id(), again.id());
    assert_eq!(
        copy.flatten_all()
            .unwrap()
            .to_vec1::<f32>()
            .unwrap()
            .into_iter()
            .map(f32::to_bits)
            .collect::<Vec<_>>(),
        expected
    );
    assert_eq!(transfers.receipts().len(), 1);
    assert_eq!(transfers.receipts()[0].bytes, 48);
    assert!(transfers.receipts()[0].peer_dma_completed);
    transfers.validate_complete(&cancel).unwrap();
    let mut next = XqStepTransfers::new(&group, &limits, 65536, 16).unwrap();
    assert_ne!(
        next.copy_frozen(&view, &devices[1], &cancel).unwrap().id(),
        copy.id()
    );
    cancel.cancel();
    assert!(next.copy_frozen(&view, &devices[1], &cancel).is_err());
    assert!(next.validate_complete(&CancellationToken::new()).is_err());
}

#[test]
#[ignore = "requires an owned idle eight-H200 allocation; never run in the one-GPU worker"]
fn h200x8_xq_peer_snapshots_preserve_view_bits_and_request_local_cache_affinity() {
    let limits = InferenceLimits::default();
    let runtimes: Vec<_> = (0..8)
        .map(|ordinal| RuntimeDevice::resolve(DevicePreference::Cuda { ordinal }).unwrap())
        .collect();
    let devices: Vec<_> = runtimes
        .iter()
        .cloned()
        .map(|d| CudaLayerDevice::new(d, limits.clone()).unwrap())
        .collect();
    let mut identities = BTreeSet::new();
    for (ordinal, d) in devices.iter().enumerate() {
        let stream = d.tensor_device().as_cuda_device().unwrap().cuda_stream();
        let ctx = stream.context();
        assert_eq!(ctx.compute_capability().unwrap(), (9, 0));
        assert!(ctx.total_mem().unwrap() >= 128 * 1024 * 1024 * 1024);
        let uuid = ctx.uuid().unwrap().bytes.map(|v| v as u8);
        assert!(identities.insert(uuid));
        println!(
            "owned_gpu ordinal={ordinal} name={:?} uuid={uuid:?}",
            ctx.name().unwrap()
        );
    }
    let edges = XqPlacement::five_layers_per_device()
        .peer_edges(&architecture())
        .unwrap();
    let group = CudaPeerGroup::new(&runtimes, &edges, limits.clone()).unwrap();
    let cancel = CancellationToken::new();
    let source: Vec<_> = (0..1024)
        .map(|n| {
            if n % 9 == 0 {
                -0.0_f32
            } else {
                (n as f32 - 512.) / 16.
            }
        })
        .collect();
    let original = Tensor::from_vec(source.clone(), (32, 32), devices[0].tensor_device()).unwrap();
    let view = original
        .narrow(0, 7, 2)
        .unwrap()
        .narrow(1, 9, 3)
        .unwrap()
        .t()
        .unwrap();
    let expected: Vec<_> = (9..12)
        .flat_map(|column| [source[7 * 32 + column], source[8 * 32 + column]])
        .map(f32::to_bits)
        .collect();
    let mut transfers = XqStepTransfers::new(&group, &limits, 1_000_000, 64).unwrap();
    let first = transfers.copy_frozen(&view, &devices[1], &cancel).unwrap();
    let again = transfers.copy_frozen(&view, &devices[1], &cancel).unwrap();
    assert_eq!(first.id(), again.id());
    assert_eq!(transfers.receipts().len(), 1);
    let mut moved = first;
    for target in 2..8 {
        moved = transfers.copy(&moved, &devices[target], &cancel).unwrap();
    }
    assert_eq!(
        moved
            .flatten_all()
            .unwrap()
            .to_vec1::<f32>()
            .unwrap()
            .into_iter()
            .map(f32::to_bits)
            .collect::<Vec<_>>(),
        expected
    );
    assert!(transfers
        .receipts()
        .iter()
        .all(|r| r.bytes == 24 && r.peer_dma_completed));
    assert_eq!(transfers.receipts().len(), 7);
    // Shared source20 (ordinal4) reaches multiple consumers. Repeated reads
    // reuse exactly one completed snapshot for each target, not one per layer.
    let packed = Tensor::from_vec(
        (0..1152).map(|v| (v % 251) as u8).collect::<Vec<_>>(),
        (4, 288),
        devices[4].tensor_device(),
    )
    .unwrap();
    for target in [5, 6, 7] {
        let copied = transfers
            .copy_frozen(&packed, &devices[target], &cancel)
            .unwrap();
        let again = transfers
            .copy_frozen(&packed, &devices[target], &cancel)
            .unwrap();
        assert_eq!(copied.id(), again.id());
        assert_eq!(
            copied.flatten_all().unwrap().to_vec1::<u8>().unwrap(),
            (0..1152).map(|v| (v % 251) as u8).collect::<Vec<_>>()
        );
    }
    assert_eq!(transfers.receipts().len(), 10);
    let empty = Tensor::zeros((0, 288), DType::U8, devices[4].tensor_device()).unwrap();
    let empty = transfers.copy_frozen(&empty, &devices[7], &cancel).unwrap();
    assert_eq!(empty.dims(), [0, 288]);
    assert!(empty.device().same_device(devices[7].tensor_device()));
    assert_eq!(transfers.receipts().len(), 10);
    transfers.validate_complete(&cancel).unwrap();
    let mut refused = XqStepTransfers::new(&group, &limits, 64, 8).unwrap();
    assert!(refused.copy(&view, &devices[7], &cancel).is_err()); // undeclared edge
    assert!(refused.copy(&view, &devices[1], &cancel).is_err());
    let mut next = XqStepTransfers::new(&group, &limits, 64, 8).unwrap();
    let fresh = next.copy_frozen(&view, &devices[1], &cancel).unwrap();
    assert_ne!(fresh.id(), again.id());
    assert_eq!(next.receipts().len(), 1);
    cancel.cancel();
    assert!(next.copy_frozen(&view, &devices[1], &cancel).is_err());
    assert!(next.validate_complete(&CancellationToken::new()).is_err());
}
