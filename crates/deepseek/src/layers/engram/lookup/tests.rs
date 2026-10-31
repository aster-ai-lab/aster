use super::*;
use crate::weights::{tests::fixture, CheckpointWeights};
use aster_power::inference::InferenceLimits;
use tempfile::TempDir;

pub(crate) fn checkpoint(
    data: Vec<u8>,
    sf: Vec<u8>,
    rows: usize,
    budget: u64,
) -> (TempDir, CheckpointWeights) {
    fixture(
        &[
            ("embedding", "F8_E4M3", vec![rows, EMBEDDING_WIDTH], data),
            ("sf", "F8_E8M0", vec![rows, SCALE_WIDTH], sf),
        ],
        budget,
    )
}

pub(crate) fn table<'a>(
    device: &CudaLayerDevice,
    checkpoint: &'a CheckpointWeights,
    rows: usize,
) -> EngramTable<'a> {
    EngramTable::new(
        device.clone(),
        checkpoint
            .matrix(
                "embedding",
                Some("sf"),
                MatrixLayout::Fp8Row32,
                [rows, EMBEDDING_WIDTH],
            )
            .unwrap(),
    )
    .unwrap()
}

fn bytes(t: &Tensor) -> Vec<u8> {
    t.flatten_all().unwrap().to_vec1().unwrap()
}

#[test]
fn sparse_gather_preserves_exact_head_order_duplicates_signed_zero_and_checkpoint_scale_zero() {
    let device = CudaLayerDevice::cpu_for_tests(InferenceLimits::default());
    let data: Vec<_> = (0..4 * EMBEDDING_WIDTH)
        .map(|i| [56, 184, 0, 128, 1, 57][i % 6])
        .collect();
    let sf: Vec<_> = (0..4 * SCALE_WIDTH)
        .map(|i| [127, 128, 126, 0][i % 4])
        .collect();
    // A one-row budget forces adjacent unique IDs to split into bounded reads.
    let (_dir, checkpoint) = checkpoint(data.clone(), sf.clone(), 4, RAW_ROW_BYTES as u64);
    let table = table(&device, &checkpoint, 4);
    let row = std::array::from_fn(|i| [3, 0, 3, 2, 1][i % 5]);
    let fetched = table
        .fetch(&[row, [-1; TABLES_PER_LAYER]], &CancellationToken::new())
        .unwrap();
    let expected_data: Vec<_> = row
        .iter()
        .flat_map(|i| {
            data[*i as usize * EMBEDDING_WIDTH..(*i as usize + 1) * EMBEDDING_WIDTH].to_vec()
        })
        .chain(vec![0; PROJECTED_INPUT_WIDTH])
        .collect();
    let expected_sf: Vec<_> = row
        .iter()
        .flat_map(|i| sf[*i as usize * SCALE_WIDTH..(*i as usize + 1) * SCALE_WIDTH].to_vec())
        .chain(vec![127; PROJECTED_INPUT_WIDTH / 32])
        .collect();
    assert_eq!(bytes(fetched.activations().data_bytes()), expected_data);
    assert_eq!(bytes(fetched.activations().scale_bytes()), expected_sf);
    assert_eq!(bytes(fetched.image_mask()), vec![0, 1]);
    // Live cast's public padding refusal is unchanged by the separate table path.
    assert!(Fp8Activations::from_packed(
        &device,
        fetched.activations().data_bytes(),
        fetched.activations().scale_bytes()
    )
    .is_err());
}

#[test]
fn image_only_lookup_performs_no_storage_read_and_never_casts_minus_one_to_an_offset() {
    let device = CudaLayerDevice::cpu_for_tests(InferenceLimits::default());
    let (dir, checkpoint) = checkpoint(vec![56; EMBEDDING_WIDTH], vec![127; SCALE_WIDTH], 1, 264);
    let table = table(&device, &checkpoint, 1);
    for path in std::fs::read_dir(dir.path()).unwrap() {
        // Power pins an already verified file handle: unlinking its pathname
        // does not invalidate that handle. Truncate this private fixture's
        // backing storage instead so an actual positional read must fail.
        std::fs::OpenOptions::new()
            .write(true)
            .open(path.unwrap().path())
            .unwrap()
            .set_len(0)
            .unwrap();
    }
    let fetched = table
        .fetch(&[[-1; TABLES_PER_LAYER]], &CancellationToken::new())
        .unwrap();
    assert_eq!(
        bytes(fetched.activations().data_bytes()),
        vec![0; PROJECTED_INPUT_WIDTH]
    );
    assert_eq!(bytes(fetched.image_mask()), vec![1]);
    assert!(table
        .fetch(&[[0; TABLES_PER_LAYER]], &CancellationToken::new())
        .is_err());
}

#[test]
fn lookup_refuses_mixed_images_bad_ids_nan_data_scales_and_cancellation() {
    let device = CudaLayerDevice::cpu_for_tests(InferenceLimits::default());
    let (_dir, checkpoint) = checkpoint(vec![56; EMBEDDING_WIDTH], vec![127; SCALE_WIDTH], 1, 264);
    let table = table(&device, &checkpoint, 1);
    for row in [
        [1; TABLES_PER_LAYER],
        [-2; TABLES_PER_LAYER],
        std::array::from_fn(|i| if i == 0 { -1 } else { 0 }),
    ] {
        assert!(table.fetch(&[row], &CancellationToken::new()).is_err());
    }
    assert!(table.fetch(&[], &CancellationToken::new()).is_err());
    let cancelled = CancellationToken::new();
    cancelled.cancel();
    assert!(matches!(
        table.fetch(&[[0; TABLES_PER_LAYER]], &cancelled),
        Err(PowerError::InferenceCancelled)
    ));
    for (data_code, scale_code) in [(127, 127), (255, 127), (56, 255)] {
        let (_dir, checkpoint) = self::checkpoint(
            vec![data_code; EMBEDDING_WIDTH],
            vec![scale_code; SCALE_WIDTH],
            1,
            264,
        );
        let table = self::table(&device, &checkpoint, 1);
        assert!(table
            .fetch(&[[0; TABLES_PER_LAYER]], &CancellationToken::new())
            .is_err());
    }
}

#[test]
fn lookup_bounds_selected_rows_without_requiring_the_full_table_to_fit_device_memory() {
    let device = CudaLayerDevice::cpu_for_tests(InferenceLimits {
        max_resident_weight_bytes: 1,
        max_context_tokens: 1,
        ..InferenceLimits::default()
    });
    let (_dir, checkpoint) = checkpoint(
        vec![56; 2 * EMBEDDING_WIDTH],
        vec![127; 2 * SCALE_WIDTH],
        2,
        528,
    );
    let table = table(&device, &checkpoint, 2);
    assert!(table
        .fetch(&[[1; TABLES_PER_LAYER]], &CancellationToken::new())
        .is_ok());
    assert!(table
        .fetch(&[[1; TABLES_PER_LAYER]; 2], &CancellationToken::new())
        .is_err());
    assert!(table
        .fetch_ngrams(EngramLayer::Layer1, &[], &CancellationToken::new())
        .is_err());
    let tiny = CudaLayerDevice::cpu_for_tests(InferenceLimits {
        max_state_bytes: 400000,
        ..InferenceLimits::default()
    });
    assert!(self::table(&tiny, &checkpoint, 2)
        .fetch(&[[1; TABLES_PER_LAYER]], &CancellationToken::new())
        .is_err());
    let (_dir, too_small) =
        self::checkpoint(vec![56; EMBEDDING_WIDTH], vec![127; SCALE_WIDTH], 1, 263);
    assert!(EngramTable::new(
        device.clone(),
        too_small
            .matrix(
                "embedding",
                Some("sf"),
                MatrixLayout::Fp8Row32,
                [1, EMBEDDING_WIDTH]
            )
            .unwrap()
    )
    .is_err());
}
