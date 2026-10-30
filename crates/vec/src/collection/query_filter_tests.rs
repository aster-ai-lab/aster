use crate::{Collection, CollectionSchema, DataType, Doc, FieldSchema, IndexParams};

#[test]
fn filtered_reads_are_indexed_bounded_projected_and_persistent() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("filtered");
    let mut kind = FieldSchema::new("kind", DataType::String, false, 0).unwrap();
    kind.set_index_params(&IndexParams::invert(false, false).unwrap())
        .unwrap();
    let schema = CollectionSchema::builder("filtered")
        .add_field(kind)
        .add_field(FieldSchema::new("body", DataType::String, false, 0).unwrap())
        .build()
        .unwrap();
    let collection = Collection::create_and_open(path.to_str().unwrap(), &schema, None).unwrap();
    let docs: Vec<_> = ["a", "b", "c"]
        .iter()
        .map(|id| {
            let mut doc = Doc::with_pk(*id).unwrap();
            doc.add_string("kind", if *id == "c" { "other" } else { "event" })
                .unwrap();
            doc.add_string("body", "original evidence").unwrap();
            doc
        })
        .collect();
    assert_eq!(
        collection
            .upsert(&docs.iter().collect::<Vec<_>>())
            .unwrap()
            .success_count,
        3
    );
    let found = collection
        .query_by_filter("kind='event'", 1, Some(&["kind"]))
        .unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].get_pk(), Some("a"));
    assert!(found[0].field("body").is_none());
    assert!(collection.query_by_filter("kind='event'", 0, None).is_err());
    assert!(collection.query_by_filter("", 10, None).is_err());
    assert!(collection
        .query_by_filter("kind='unterminated", 10, None)
        .is_err());
    collection.close().unwrap();
    let reopened = Collection::open(path.to_str().unwrap(), None).unwrap();
    assert_eq!(
        reopened
            .query_by_filter("kind='event'", 10, None)
            .unwrap()
            .len(),
        2
    );
}
