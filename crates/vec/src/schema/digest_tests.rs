use super::*;

pub(crate) fn indexed_schema() -> CollectionSchema {
    let params = IndexParams::fts(Some("whitespace"), None, None).unwrap();
    let mut field = FieldSchema::new("content", DataType::String, false, 0).unwrap();
    field.set_index_params(&params).unwrap();
    CollectionSchema::builder("identity")
        .add_field(field)
        .build()
        .unwrap()
}

#[test]
fn schema_digest_ignores_parameter_map_insertion_order() {
    let original = indexed_schema();
    let mut reordered = original.clone();
    reordered.fields[0]
        .index_params
        .as_mut()
        .unwrap()
        .params
        .sort_keys();
    assert_ne!(
        serde_json::to_vec(&original).unwrap(),
        serde_json::to_vec(&reordered).unwrap(),
        "fixture requires the preserve_order feature used by the paired CLI build"
    );
    assert_eq!(original.digest(), reordered.digest());
}

#[test]
fn schema_identity_preserves_field_and_array_order_and_sorts_vector_parameters() {
    let mut original = indexed_schema();
    let params = IndexParams::hnsw(MetricType::Cosine, 8, 32).unwrap();
    let mut vector = VectorSchema::new("embedding", DataType::VectorFp32, 4).unwrap();
    vector.index_params = Some(params);
    original.vectors.push(vector);
    let mut sorted = original.clone();
    sorted.vectors[0]
        .index_params
        .as_mut()
        .unwrap()
        .params
        .sort_keys();
    assert_eq!(original.digest(), sorted.digest());
    let mut changed = original.clone();
    changed
        .fields
        .push(FieldSchema::new("second", DataType::String, true, 0).unwrap());
    let mut reversed = changed.clone();
    reversed.fields.reverse();
    assert_ne!(changed.digest(), reversed.digest());
    original.fields[0]
        .index_params
        .as_mut()
        .unwrap()
        .params
        .insert("filters".into(), json!(["lowercase", "ascii_folding"]));
    sorted = original.clone();
    sorted.fields[0]
        .index_params
        .as_mut()
        .unwrap()
        .params
        .insert("filters".into(), json!(["ascii_folding", "lowercase"]));
    assert_ne!(original.digest(), sorted.digest());
}
