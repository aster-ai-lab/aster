use super::*;

#[tokio::test]
async fn retained_reader_generations_survive_replacement_removal_and_reinsertion() {
    let index = InMemoryVectorIndex::new(VectorIndexDescriptor::new(2)).unwrap();
    index
        .replace_partition("source", vec![VectorRecord::new("before", vec![1.0, 0.0])])
        .await
        .unwrap();
    let first = index.snapshot();
    let first_status = first.status();
    index
        .replace_partition("source", vec![VectorRecord::new("after", vec![0.0, 1.0])])
        .await
        .unwrap();
    let second = index.snapshot();
    assert_eq!(first.status(), first_status);
    assert_eq!(first.partitions["source"].ids, ["before"]);
    assert_eq!(second.partitions["source"].ids, ["after"]);
    index.remove_partition("source").await.unwrap();
    let empty = index.snapshot();
    assert!(empty.partitions.is_empty());
    assert_eq!(second.partitions["source"].ids, ["after"]);
    assert_eq!(second.status().record_count, 1);
    drop(first);
    drop(second);
    index
        .replace_partition("new", vec![VectorRecord::new("new", vec![1.0, 0.0])])
        .await
        .unwrap();
    assert!(empty.partitions.is_empty());
    assert_eq!(index.status().record_count, 1);
    assert_eq!(index.status().revision.value(), 4);
}
