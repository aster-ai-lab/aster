//! Executes the real loaded source+consumer pipeline. Other layers supply
//! declared synthetic state so the 53-participant transaction can be tested;
//! this is explicitly not 40-layer model inference or an eight-card proof.
use super::*;
use crate::{
    engram::{tests::text, EngramToken},
    layers::{
        indexer::SparseIndexSelection,
        xq::quant::{XqActivationFormat, XqQuantizedActivation},
    },
    state::backbone::tests::stage_synthetic_rest,
};
use aster_power::inference::{DevicePreference, RuntimeDevice};

fn values(t: &Tensor) -> Vec<f32> {
    t.flatten_all().unwrap().to_vec1().unwrap()
}
fn bytes(t: &Tensor) -> Vec<u8> {
    t.flatten_all().unwrap().to_vec1().unwrap()
}
fn snapshot(state: &BackboneState, device: &CudaLayerDevice, source: usize) -> Vec<u8> {
    let mut out = (state.position() as u64).to_le_bytes().to_vec();
    for id in [source, source + 1] {
        out.extend(bytes(
            state
                .window(id)
                .unwrap()
                .attention_rows(device)
                .unwrap()
                .records(),
        ));
    }
    let source = state.compressed_source(source).unwrap();
    out.extend(bytes(
        source.main().attention_rows(device).unwrap().records(),
    ));
    let index = source.index().index_rows(device).unwrap();
    out.extend(bytes(index.data_bytes()));
    out.extend(bytes(index.scale_bytes()));
    out
}

fn bootstrap_sharing(
    architecture: &TextArchitecture,
    pending: &PendingBackboneStep,
    device: &CudaLayerDevice,
    first: usize,
) -> StepIndexSharing {
    let positions: Vec<_> =
        (pending.position()..pending.position() + pending.token_count()).collect();
    let mut sharing = StepIndexSharing::new(device.limits(), &positions).unwrap();
    let cancel = CancellationToken::new();
    for id in 0..first {
        let binding = architecture.layer(id).unwrap();
        let window = pending
            .window(id)
            .unwrap()
            .ready_positions()
            .collect::<Vec<_>>();
        let ready = binding.sources().map(|s| {
            pending
                .compressed_source(s.kv())
                .unwrap()
                .main()
                .ready_positions()
                .collect::<Vec<_>>()
        });
        let computed: Option<SparseIndexSelection> = if binding.is_index_topk_source() {
            let source = pending
                .compressed_source(binding.sources().unwrap().index_k())
                .unwrap();
            let keys = source.index().index_rows(device).unwrap();
            let q = Tensor::zeros(
                (positions.len() * 2, 128),
                DType::F32,
                device.tensor_device(),
            )
            .unwrap();
            let q =
                XqQuantizedActivation::encode(device, XqActivationFormat::IndexFp4, &q, &cancel)
                    .unwrap()
                    .into_index_rows(&cancel)
                    .unwrap();
            let weights =
                Tensor::ones((positions.len(), 2), DType::F32, device.tensor_device()).unwrap();
            Some(
                SparseIndexer::new(device.clone(), 2, 128, 512, None)
                    .unwrap()
                    .select_xq_f32_packed(
                        &q,
                        &keys,
                        &weights,
                        &sharing
                            .index_plan(binding, ready.as_deref().unwrap())
                            .unwrap(),
                        &cancel,
                    )
                    .unwrap(),
            )
        } else {
            None
        };
        sharing
            .attention_plan(
                binding,
                &window,
                ready.as_deref(),
                computed.as_ref(),
                &cancel,
            )
            .unwrap();
    }
    sharing
}

fn run(
    layers: &[LoadedXqLayer<'_>; 2],
    state: &mut BackboneState,
    device: &CudaLayerDevice,
    embedding: &[f32],
    tokens: &[EngramToken],
    accepted: usize,
) -> Vec<f32> {
    let cancel = CancellationToken::new();
    let mut pending = state.begin(tokens, &cancel).unwrap();
    let first = layers[0].binding().layer();
    stage_synthetic_rest(state, &mut pending, device, &[first, first + 1], first);
    let mut sharing = bootstrap_sharing(&architecture(), &pending, device, first);
    let input = Tensor::from_slice(embedding, (tokens.len(), 256), device.tensor_device()).unwrap();
    let mut trunk = XqMhcContinuation::from_embedding(device, &input, &cancel).unwrap();
    for layer in layers {
        trunk = layer
            .forward_pending(trunk, state, &mut pending, &mut sharing, &cancel)
            .unwrap();
    }
    let output = values(trunk.residual());
    state.commit_prefix(pending, accepted, &cancel).unwrap();
    output
}

#[test]
#[ignore = "requires an owned H200; run explicitly with --ignored --nocapture"]
fn h200_xq_loaded_source_consumer_f32_attention_ffn_and_atomic_prefix_match_chunked_execution() {
    let device = CudaLayerDevice::new(
        RuntimeDevice::resolve(DevicePreference::Cuda { ordinal: 0 }).unwrap(),
        InferenceLimits::default(),
    )
    .unwrap();
    {
        // This worker runs hardware tests serially. Restore the process-global
        // BLAS mode even on panic; production never changes it automatically.
        struct Restore(bool);
        impl Drop for Restore {
            fn drop(&mut self) {
                candle_core::cuda_backend::set_gemm_reduced_precision_f32(self.0);
            }
        }
        let previous = candle_core::cuda_backend::gemm_reduced_precision_f32();
        assert!(
            !previous,
            "XQ qualification requires explicit full F32 GEMM precision"
        );
        let input = Tensor::ones((1, 256), DType::F32, device.tensor_device()).unwrap();
        let trunk =
            XqMhcContinuation::from_embedding(&device, &input, &CancellationToken::new()).unwrap();
        let mix = crate::layers::xq::attention::sublayer::tests::mhc(&device, 256);
        let prepared = mix.prepare(trunk, &CancellationToken::new()).unwrap();
        let _restore = Restore(previous);
        candle_core::cuda_backend::set_gemm_reduced_precision_f32(true);
        let error = prepared
            .finish(
                &Tensor::zeros((1, 256), DType::F32, device.tensor_device()).unwrap(),
                &CancellationToken::new(),
            )
            .err()
            .unwrap();
        assert!(error.to_string().contains("TF32"));
    }
    let tokens = [text(10), text(20), EngramToken::image(), text(40)];
    let embedding: Vec<_> = (0..4 * 256)
        .map(|i| (((i * 11 + 3) % 71) as f32 - 35.) / 23.)
        .collect();
    for source in [2, 20] {
        let layers = [layer(&device, source), layer(&device, source + 1)];
        // Identical geometry/weights do not authorize another projection
        // owner's prepared result. This valid window plan would otherwise run.
        let other = layer(&device, source);
        let trunk = XqMhcContinuation::from_embedding(
            &device,
            &Tensor::from_slice(&embedding, (4, 256), device.tensor_device()).unwrap(),
            &CancellationToken::new(),
        )
        .unwrap();
        let prepared = other
            .attention
            .prepare(trunk, &[0, 1, 2, 3], &CancellationToken::new())
            .unwrap();
        let window = crate::layers::kv::MainKvRows::from_records(
            &device,
            MainKvFormat::Fp8Ue8m0,
            prepared.projected().window_rows().records(),
        )
        .unwrap();
        let candidates: Vec<_> = (0..4)
            .map(|p| {
                crate::layers::attention::KvCandidate::new(
                    crate::layers::attention::KvSource::Window,
                    p,
                )
            })
            .collect();
        let selections: Vec<_> = (0..4).map(|p| (0..=p).collect::<Vec<_>>()).collect();
        let plan = crate::layers::attention::SparseAttentionPlan::new(
            device.limits(),
            128,
            &candidates,
            &[0, 1, 2, 3],
            &selections,
        )
        .unwrap();
        let error = layers[0]
            .attention
            .finish(prepared, &window, None, &plan, &CancellationToken::new())
            .err()
            .unwrap();
        assert!(error.to_string().contains("projection owner"));
        let mut whole = state(&device);
        let expected = run(&layers, &mut whole, &device, &embedding, &tokens, 4);
        assert!(expected.iter().all(|v| v.is_finite()));
        let initial = Tensor::from_slice(&embedding, (4, 256), device.tensor_device()).unwrap();
        let initial = values(
            XqMhcContinuation::from_embedding(&device, &initial, &CancellationToken::new())
                .unwrap()
                .residual(),
        );
        assert!(expected
            .iter()
            .zip(&initial)
            .any(|(a, b)| (a - b).abs() > 1e-5));
        let mut split = state(&device);
        let mut actual = Vec::new();
        for i in 0..4 {
            actual.extend(run(
                &layers,
                &mut split,
                &device,
                &embedding[i * 256..(i + 1) * 256],
                &tokens[i..i + 1],
                1,
            ));
        }
        // Frozen pre-hardware composition budget; exact codec bits and per-
        // operator F64 comparisons remain in separate H200 qualification tests.
        for (index, (actual, expected)) in actual.iter().zip(&expected).enumerate() {
            assert!(
                (actual - expected).abs() <= 2e-4 + 2e-4 * expected.abs(),
                "source {source} index {index}: {actual} != {expected}"
            );
        }
        assert_eq!(
            snapshot(&split, &device, source),
            snapshot(&whole, &device, source)
        );
        for accepted in 0..=3 {
            let mut rejected = state(&device);
            run(
                &layers,
                &mut rejected,
                &device,
                &embedding,
                &tokens,
                accepted,
            );
            run(
                &layers,
                &mut rejected,
                &device,
                &embedding[accepted * 256..],
                &tokens[accepted..],
                4 - accepted,
            );
            assert_eq!(
                snapshot(&rejected, &device, source),
                snapshot(&whole, &device, source)
            );
        }
        let mut cancelled = state(&device);
        let before = snapshot(&cancelled, &device, source);
        let cancel = CancellationToken::new();
        let mut pending = cancelled.begin(&tokens, &cancel).unwrap();
        stage_synthetic_rest(
            &cancelled,
            &mut pending,
            &device,
            &[source, source + 1],
            source,
        );
        let mut sharing = bootstrap_sharing(&architecture(), &pending, &device, source);
        let input = Tensor::from_slice(&embedding, (4, 256), device.tensor_device()).unwrap();
        let trunk = XqMhcContinuation::from_embedding(&device, &input, &cancel).unwrap();
        let trunk = layers[0]
            .forward_pending(trunk, &cancelled, &mut pending, &mut sharing, &cancel)
            .unwrap();
        cancel.cancel();
        assert!(layers[1]
            .forward_pending(trunk, &cancelled, &mut pending, &mut sharing, &cancel)
            .is_err());
        assert!(pending.is_poisoned());
        assert!(cancelled
            .commit_prefix(pending, 4, &CancellationToken::new())
            .is_err());
        assert_eq!(snapshot(&cancelled, &device, source), before);
    }
}
