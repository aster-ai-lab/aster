use super::*;
use crate::{layers::xq::mhc::XqMhcContinuation, model::DeepSeekModel};
use candle_core::{DType, Device};

#[test]
fn explicit_replica_device_maps_preserve_local_order_without_aliasing_foreign_cards() {
    for pair in [vec![0, 1], vec![2, 3], vec![4, 5], vec![7, 6]] {
        let mapping = XqDeviceMap::new(XqReplicaWidth::Two, pair.clone()).unwrap();
        assert_eq!(mapping.local_rank(pair[0]).unwrap(), 0);
        assert_eq!(mapping.local_rank(pair[1]).unwrap(), 1);
        assert_eq!(
            mapping.peer_edges(&[(0, 1)]).unwrap(),
            vec![(pair[0], pair[1])]
        );
        assert!(mapping.peer_edges(&[(0, 2)]).is_err());
        assert!(mapping.peer_edges(&[(1, 1)]).is_err());
        assert!(mapping.local_rank(8).is_err());
    }
    for invalid in [vec![], vec![1], vec![0, 0], vec![0, 8], vec![0, 1, 2]] {
        assert!(XqDeviceMap::new(XqReplicaWidth::Two, invalid).is_err());
    }
    assert_eq!(
        XqDeviceMap::identity(XqReplicaWidth::Four).ordinals(),
        &[0, 1, 2, 3]
    );
}

#[test]
fn replica_widths_bind_complete_models_and_only_their_local_peer_edges() {
    for width in [
        XqReplicaWidth::Two,
        XqReplicaWidth::Four,
        XqReplicaWidth::Eight,
    ] {
        assert_eq!(XqReplicaWidth::parse(width.as_str()).unwrap(), width);
        let placement = XqPlacement::balanced(width);
        assert_eq!(placement.device_count(), width.device_count());
        assert_eq!(placement.layer_ordinals().len(), 40);
        assert_eq!(placement.embedding_ordinal(), 0);
        assert_eq!(placement.head_ordinal(), width.device_count() - 1);
        for ordinal in 0..width.device_count() {
            assert_eq!(
                placement
                    .layer_ordinals()
                    .iter()
                    .filter(|&&rank| rank == ordinal)
                    .count(),
                40 / width.device_count()
            );
        }
        let edges = placement.peer_edges(&architecture()).unwrap();
        assert!(edges.iter().all(
            |&(source, target)| source < width.device_count() && target < width.device_count()
        ));
        if width == XqReplicaWidth::Two {
            assert_eq!(edges, [(0, 1)]);
        } else if width == XqReplicaWidth::Four {
            assert_eq!(edges, [(0, 1), (1, 2), (2, 3)]);
        }
    }
    for value in ["1", "3", "16", "two", "", " 2"] {
        assert!(XqReplicaWidth::parse(value).is_err());
    }
}

#[test]
fn replica_local_placement_refuses_missing_owners_and_foreign_ordinals() {
    for width in [XqReplicaWidth::Two, XqReplicaWidth::Four] {
        let count = width.device_count();
        let placement = XqPlacement::balanced(width);
        assert!(XqPlacement::for_replica(width, placement.layers.clone(), 0, count - 1).is_ok());
        assert!(XqPlacement::for_replica(width, vec![0; 40], 0, 0).is_err());
        assert!(
            XqPlacement::for_replica(width, placement.layers.clone(), count, count - 1).is_err()
        );
        let mut foreign = placement.layers.clone();
        foreign[39] = count;
        assert!(XqPlacement::for_replica(width, foreign, 0, count - 1).is_err());
        assert!(XqPlacement::new(placement.layers, 0, count - 1).is_err());
    }
}

fn architecture() -> TextArchitecture {
    DeepSeekModel::V41Flash1017Xq
        .verify_config(include_bytes!("../../fixtures/xq-model-config.json"))
        .unwrap()
        .text_architecture()
        .unwrap()
}

#[test]
fn peer_plan_includes_trunk_and_shared_source20_consumers_without_all_to_all() {
    let plan = XqPlacement::five_layers_per_device();
    let edges = plan.peer_edges(&architecture()).unwrap();
    assert_eq!(
        edges,
        vec![
            (0, 1),
            (1, 2),
            (2, 3),
            (3, 4),
            (4, 5),
            (4, 6),
            (4, 7),
            (5, 6),
            (6, 7)
        ]
    );
    assert_eq!(
        plan.layer_ordinals(),
        &(0..40).map(|layer| layer / 5).collect::<Vec<_>>()
    );
    let mut reversed = plan.layer_ordinals().to_vec();
    reversed.reverse();
    let custom = XqPlacement::new(reversed, 7, 0).unwrap();
    let edges = custom.peer_edges(&architecture()).unwrap();
    assert!(edges.contains(&(3, 0))); // source20's shared main/index-K to layer35
    assert!(!edges.contains(&(0, 3)));
}

#[test]
fn placement_refuses_missing_layers_unbound_ordinals_or_a_disguised_single_device() {
    for layers in [
        vec![0; 40],
        vec![0; 39],
        (0..40).map(|layer| layer / 5 + 1).collect(),
    ] {
        assert!(XqPlacement::new(layers, 0, 7).is_err());
    }
    let layers = XqPlacement::five_layers_per_device()
        .layer_ordinals()
        .to_vec();
    assert!(XqPlacement::new(layers.clone(), 8, 7).is_err());
    assert!(XqPlacement::new(layers, 0, 8).is_err());
}

#[test]
fn local_continuation_preserves_shifted_carry_and_failed_transfer_poison_is_sticky() {
    let limits = InferenceLimits::default();
    let device = CudaLayerDevice::cpu_for_tests(limits.clone());
    let cancellation = CancellationToken::new();
    let embedding = Tensor::from_vec(vec![1_f32, 2., 3., 4.], (2, 2), &Device::Cpu).unwrap();
    let input = XqMhcContinuation::from_embedding(&device, &embedding, &cancellation).unwrap();
    let residual = input.residual().to_vec3::<f32>().unwrap();
    let carry = input
        .next_pre_mix()
        .flatten_all()
        .unwrap()
        .to_vec1::<f32>()
        .unwrap();
    let mut transfers = XqStepTransfers::local(&limits);
    let moved = input
        .move_to(&device, &mut transfers, &cancellation)
        .unwrap();
    assert_eq!(moved.residual().to_vec3::<f32>().unwrap(), residual);
    assert_eq!(
        moved
            .next_pre_mix()
            .flatten_all()
            .unwrap()
            .to_vec1::<f32>()
            .unwrap(),
        carry
    );
    assert!(transfers.receipts().is_empty());
    let mut different = limits.clone();
    different.max_state_bytes /= 2;
    let target = CudaLayerDevice::cpu_for_tests(different);
    assert!(transfers
        .copy_frozen(moved.residual(), &target, &cancellation)
        .is_err());
    assert!(transfers.copy(&embedding, &device, &cancellation).is_err());
    assert!(transfers.validate_complete(&cancellation).is_err());
}

#[test]
fn cancellation_refuses_even_local_cache_borrows_without_a_receipt() {
    let limits = InferenceLimits::default();
    let device = CudaLayerDevice::cpu_for_tests(limits.clone());
    let mut transfers = XqStepTransfers::local(&limits);
    let empty = Tensor::zeros((0, 288), DType::U8, &Device::Cpu).unwrap();
    let cancellation = CancellationToken::new();
    cancellation.cancel();
    assert!(transfers
        .copy_frozen(&empty, &device, &cancellation)
        .is_err());
    assert!(transfers
        .validate_complete(&CancellationToken::new())
        .is_err());
    assert!(transfers.receipts().is_empty());
}

#[cfg(feature = "cuda")]
mod gpu;
