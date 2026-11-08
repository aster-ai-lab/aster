use super::*;

#[test]
fn model_reference_preserves_paths_and_refuses_invalid_or_unbounded_input() {
    let root = tempfile::Builder::new()
        .prefix("OCR 插件 ")
        .tempdir()
        .unwrap();
    let reference = root.path().join(RECEIPT_FILE);
    assert_eq!(read_model_reference(&reference).unwrap(), None);
    let models = root.path().join("native package/ocr-models/PP-OCRv6_small");
    std::fs::write(&reference, serde_json::to_vec(&models).unwrap()).unwrap();
    assert_eq!(read_model_reference(&reference).unwrap(), Some(models));
    for value in [
        b"\"relative/path\"".to_vec(),
        vec![b' '; 16 * 1024 + 1],
        b"invalid".to_vec(),
    ] {
        std::fs::write(&reference, value).unwrap();
        assert_eq!(
            read_model_reference(&reference).unwrap_err().code,
            "use.ocr.model_invalid"
        );
    }
}
