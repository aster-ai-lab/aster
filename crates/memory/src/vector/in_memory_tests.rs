use super::*;

#[test]
fn blocking_ports_work_without_a_runtime_and_preserve_admission_atomicity() {
    let index =
        InMemoryVectorIndex::new(VectorIndexDescriptor::new(2).with_max_records(1)).unwrap();
    let stored = index
        .replace_partition_blocking("source", vec![VectorRecord::new("one", vec![2.0, 0.0])])
        .unwrap();
    let hits = index
        .search_blocking(VectorSearchRequest::new(vec![3.0, 0.0], 1))
        .unwrap();
    assert_eq!(hits.status, stored);
    assert_eq!(hits.hits[0].id, "one");
    assert_eq!(hits.hits[0].score, 1.0);
    assert!(matches!(
        index.replace_partition_blocking(
            "source",
            vec![VectorRecord::new("bad", vec![f32::NAN, 0.0])]
        ),
        Err(VectorIndexError::NonFiniteVector { .. })
    ));
    assert!(matches!(
        index.replace_partition_blocking("other", vec![VectorRecord::new("two", vec![0.0, 1.0])]),
        Err(VectorIndexError::BudgetExceeded {
            resource: VectorBudgetResource::Records,
            ..
        })
    ));
    assert_eq!(index.status(), stored);
    assert!(matches!(
        index.search_blocking(VectorSearchRequest::new(vec![1.0], 1)),
        Err(VectorIndexError::DimensionMismatch { .. })
    ));
    assert!(matches!(
        index.remove_partition_blocking(" "),
        Err(VectorIndexError::InvalidPartition)
    ));
    let removed = index.remove_partition_blocking("source").unwrap();
    assert_eq!(removed.record_count, 0);
    assert_eq!(removed.revision.value(), stored.revision.value() + 1);
}

#[tokio::test]
async fn blocking_and_async_ports_share_one_history_and_snapshot_owner() {
    let index = InMemoryVectorIndex::new(VectorIndexDescriptor::new(2)).unwrap();
    let clone = index.clone();
    index
        .replace_partition_blocking("source", vec![VectorRecord::new("first", vec![1.0, 0.0])])
        .unwrap();
    let result = clone
        .search(VectorSearchRequest::new(vec![1.0, 0.0], 1))
        .await
        .unwrap();
    assert_eq!(result.hits[0].id, "first");
    let before = index.change_token().unwrap();
    clone
        .replace_partition("source", vec![VectorRecord::new("second", vec![0.0, 1.0])])
        .await
        .unwrap();
    let result = index
        .search_blocking(VectorSearchRequest::new(vec![0.0, 2.0], 1))
        .unwrap();
    assert_eq!(result.hits[0].id, "second");
    assert_eq!(result.hits[0].score, 1.0);
    let observed = clone.observe().await.unwrap();
    assert_eq!(observed.status, result.status);
    assert_eq!(observed.change_token, index.change_token());
    assert_ne!(before, observed.change_token.unwrap());
    index.remove_partition_blocking("source").unwrap();
    assert_eq!(clone.observe().await.unwrap().status.record_count, 0);
    assert!(clone
        .search(VectorSearchRequest::new(vec![1.0, 0.0], 1))
        .await
        .unwrap()
        .hits
        .is_empty());
}

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
