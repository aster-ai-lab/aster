use super::*;

#[test]
fn exact_normalization_keeps_space_and_invalid_byte_spellings_distinct() {
    let normalizer = compression_normalizer().unwrap();
    for (text, expected) in [
        ("ÉCOLE", "ecole"),
        ("ＥＣＯＬＥ", "ecole"),
        (" A\tB\n", "a b"),
        (" \t\n", " "),
        (" ", " "),
        ("", ""),
        ("汉字", "汉字"),
    ] {
        assert_eq!(compression_key(text, "raw", &normalizer).unwrap(), expected);
    }
    assert_eq!(
        compression_key("�", "<byte-80>", &normalizer).unwrap(),
        "<byte-80>"
    );
    assert_ne!(
        compression_key("�", "<byte-80>", &normalizer).unwrap(),
        compression_key("�", "<byte-81>", &normalizer).unwrap()
    );
}

#[test]
fn compressed_ids_follow_first_raw_id_occurrence_including_special_tokens() {
    let bytes=serde_json::to_vec(&serde_json::json!({"version":"1.0","truncation":null,"padding":null,
        "added_tokens":[],"normalizer":null,"pre_tokenizer":null,"post_processor":null,"decoder":null,
        "model":{"type":"WordLevel","vocab":{"É":0,"é":1,"<｜▁pad▁｜>":2,"Ａ":3,"a":4," ":5,"�x":6,"�y":7},"unk_token":"<｜▁pad▁｜>"}})).unwrap();
    let tokenizer = Tokenizer::from_bytes(&bytes).unwrap();
    let (mapping, count) = build_map(&tokenizer, 8).unwrap();
    assert_eq!(mapping, vec![0, 0, 1, 2, 2, 3, 4, 5]);
    assert_eq!(count, 6);
    assert!(build_map(&tokenizer, 7).is_err());
    let vocabulary = CompressedVocabulary { mapping };
    assert_eq!(vocabulary.compress(2).unwrap(), 1);
    assert!(vocabulary.compress(8).is_err());
}

#[test]
fn tokenizer_artifact_identity_and_byte_budget_refuse_unknown_data() {
    for bytes in [b"".as_slice(), b"{}", b"not JSON"] {
        assert!(CompressedVocabulary::from_official_tokenizer(bytes).is_err());
    }
    assert!(
        CompressedVocabulary::from_official_tokenizer(&vec![0; MAX_TOKENIZER_BYTES + 1]).is_err()
    );
}
