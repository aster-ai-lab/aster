use super::*;

#[tokio::test]
async fn the_adapter_serves_a_batch_through_the_core_trait() {
    let provider = LightTierProvider::load().expect("embedded assets load");
    let inputs = vec![
        aster_core::EmbeddingInput::new("a", "parse the config"),
        aster_core::EmbeddingInput::new("b", "hello"),
    ];
    let request = EmbeddingBatchRequest::new(inputs);
    let response = provider
        .embed(request, CancellationToken::new())
        .await
        .expect("light tier serves the batch");
    assert_eq!(response.vectors.len(), 2);
    assert_eq!(response.vectors[0].id.as_ref(), "a");
    assert_eq!(
        response.vectors[0].values.len(),
        provider.descriptor().dimension
    );
    assert_eq!(
        response.descriptor.normalization,
        EmbeddingNormalization::Unit
    );
}

#[test]
fn descriptor_identifies_the_entire_verified_bundle() {
    let provider = LightTierProvider::load().unwrap();
    assert_eq!(
        provider.descriptor().revision.as_deref(),
        Some("sha256:2320c68f3da5ca9eb8ed56b2354d1cc85d47849be77aa4876ab569e352bade3e")
    );
    let debug = format!("{provider:?}");
    assert!(debug.contains("potion-code-16M-v2"));
    assert!(debug.len() < 512, "Debug must not dump the embedded model");
}
