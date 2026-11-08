use super::*;
use crate::error::ErrorCode;
use crate::schema::FieldSchema;
use crate::types::DataType;
use std::process::Command;

fn schema() -> CollectionSchema {
    CollectionSchema::builder("legacy")
        .add_field(FieldSchema::new("value", DataType::String, false, 0).expect("field"))
        .build()
        .expect("schema")
}

fn document(id: &str, value: &str) -> Doc {
    let mut doc = Doc::new().expect("document");
    doc.set_pk(id);
    doc.add_string("value", value).expect("value");
    doc
}

fn chain(root: &Path, depth: u64) -> (Manifest, Vec<Doc>) {
    let schema = schema();
    let mut docs = vec![document("kept", "unchanged"), document("live", "1")];
    let mut digest = write_with_faults(
        root,
        &schema,
        &docs,
        1,
        1,
        false,
        &FaultInjector::default(),
        MAX_SNAPSHOT_BYTES,
    )
    .expect("full snapshot");
    for generation in 2..=depth {
        let mut next = vec![
            document("kept", "unchanged"),
            document("live", &generation.to_string()),
        ];
        if generation % 3 == 0 {
            next.push(document("transient", "removed and reinserted"));
        }
        digest = write_delta_with_faults(
            root,
            &schema,
            &next,
            &docs,
            generation,
            generation,
            generation - 1,
            digest,
            false,
            &FaultInjector::default(),
            MAX_SNAPSHOT_BYTES,
        )
        .expect("delta snapshot");
        docs = next;
    }
    let mut manifest = Manifest::new("legacy", schema.digest());
    manifest.generation = depth;
    manifest.revision = depth;
    manifest.checkpoint_revision = depth;
    manifest.docs_checksum = digest;
    (manifest, docs)
}

#[test]
fn legacy_long_chain_recovers_on_a_bounded_worker_stack() {
    const CHILD: &str = "ASTER_VEC_LEGACY_CHAIN_CHILD";
    if std::env::var_os(CHILD).is_some() {
        std::thread::Builder::new()
            .stack_size(2 * 1024 * 1024)
            .spawn(|| {
                let temp = tempfile::tempdir().expect("temporary directory");
                let root = temp.path().join("legacy 空间");
                let (manifest, expected) = chain(&root, 2048);
                let (loaded_schema, docs) = read(&root, &manifest, MAX_SNAPSHOT_BYTES)
                    .expect("recover every acknowledged legacy snapshot");
                assert_eq!(loaded_schema.digest(), schema().digest());
                assert_eq!(docs, expected);
            })
            .expect("worker")
            .join()
            .expect("worker completed");
        return;
    }
    // A legacy stack overflow must fail this test without aborting the whole suite.
    let output = Command::new(std::env::current_exe().expect("test executable"))
        .args([
            "storage::snapshot::recovery_tests::legacy_long_chain_recovers_on_a_bounded_worker_stack",
            "--exact",
            "--nocapture",
        ])
        .env(CHILD, "1")
        .output()
        .expect("recovery subprocess");
    assert!(
        output.status.success(),
        "legacy recovery failed: {:?}\n{}\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

#[test]
fn chain_recovery_preserves_checksums_metadata_and_per_file_limits() {
    let temp = tempfile::tempdir().expect("temporary directory");
    let root = temp.path();
    let (mut manifest, expected) = chain(root, 12);
    assert_eq!(
        read(root, &manifest, MAX_SNAPSHOT_BYTES).unwrap().1,
        expected
    );
    assert_eq!(
        read(root, &manifest, 1).expect_err("size admission").code,
        ErrorCode::ResourceExhausted,
    );
    manifest.checkpoint_revision += 1;
    assert_eq!(
        read(root, &manifest, MAX_SNAPSHOT_BYTES)
            .expect_err("tip revision")
            .code,
        ErrorCode::InternalError,
    );
    manifest.checkpoint_revision -= 1;
    let ancestor = root.join(binary_relative_path(2));
    let mut bytes = fs::read(&ancestor).expect("ancestor");
    bytes.push(0);
    fs::write(&ancestor, bytes).expect("corrupt ancestor");
    assert_eq!(
        read(root, &manifest, MAX_SNAPSHOT_BYTES)
            .expect_err("ancestor checksum")
            .code,
        ErrorCode::InternalError,
    );
}

#[test]
fn iterative_recovery_rejects_invalid_delta_metadata_and_records() {
    let temp = tempfile::tempdir().expect("temporary directory");
    let root = temp.path();
    let schema = schema();
    let base = vec![document("kept", "original")];
    let digest = write_with_faults(
        root,
        &schema,
        &base,
        1,
        1,
        false,
        &FaultInjector::default(),
        MAX_SNAPSHOT_BYTES,
    )
    .unwrap();
    let other_schema = CollectionSchema::builder("legacy")
        .add_field(FieldSchema::new("different", DataType::String, false, 0).unwrap())
        .build()
        .unwrap();
    for (generation, base_generation, tip_schema, upserted, trailing) in [
        (99, 1, &schema, Vec::new(), false),
        (2, 0, &schema, Vec::new(), false),
        (2, 2, &schema, Vec::new(), false),
        (2, 1, &other_schema, Vec::new(), false),
        (2, 1, &schema, vec![Doc::new().unwrap()], false),
        (2, 1, &schema, Vec::new(), true),
    ] {
        let delta = DeltaSnapshot::new(
            DELTA_SNAPSHOT_FORMAT_VERSION,
            generation,
            2,
            tip_schema,
            base_generation,
            digest,
            Vec::new(),
            &upserted,
        );
        let mut bytes = rmp_serde::to_vec(&delta).unwrap();
        if trailing {
            bytes.push(0);
        }
        let checksum = write_bytes(
            root,
            &binary_relative_path(2),
            &bytes,
            false,
            &FaultInjector::default(),
            MAX_SNAPSHOT_BYTES,
        )
        .unwrap();
        let mut manifest = Manifest::new("legacy", tip_schema.digest());
        manifest.generation = 2;
        manifest.checkpoint_revision = 2;
        manifest.docs_checksum = checksum;
        assert_eq!(
            read(root, &manifest, MAX_SNAPSHOT_BYTES)
                .expect_err("invalid delta must fail closed")
                .code,
            ErrorCode::InternalError,
        );
    }
}

#[test]
fn full_snapshot_order_and_delta_last_write_semantics_are_preserved() {
    let temp = tempfile::tempdir().expect("temporary directory");
    let root = temp.path();
    let schema = schema();
    let base = vec![document("z", "last"), document("a", "first")];
    let digest = write_with_faults(
        root,
        &schema,
        &base,
        1,
        1,
        false,
        &FaultInjector::default(),
        MAX_SNAPSHOT_BYTES,
    )
    .unwrap();
    let mut manifest = Manifest::new("legacy", schema.digest());
    manifest.generation = 1;
    manifest.checkpoint_revision = 1;
    manifest.docs_checksum = digest;
    assert_eq!(read(root, &manifest, MAX_SNAPSHOT_BYTES).unwrap().1, base);
    let delta = DeltaSnapshot::new(
        DELTA_SNAPSHOT_FORMAT_VERSION,
        2,
        2,
        &schema,
        1,
        digest,
        vec!["a".into(), "z".into()],
        &[document("a", "reinserted"), document("a", "latest")],
    );
    let checksum = write_bytes(
        root,
        &binary_relative_path(2),
        &rmp_serde::to_vec(&delta).unwrap(),
        false,
        &FaultInjector::default(),
        MAX_SNAPSHOT_BYTES,
    )
    .unwrap();
    manifest.generation = 2;
    manifest.checkpoint_revision = 2;
    manifest.docs_checksum = checksum;
    assert_eq!(
        read(root, &manifest, MAX_SNAPSHOT_BYTES).unwrap().1,
        vec![document("a", "latest")],
    );
}
