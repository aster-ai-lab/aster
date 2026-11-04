//! Fixed primary-store workload; this does not qualify semantic M-tier recall.

use aster_core::durable_memory_vec::LazyFileMemoryStore;
use aster_durable_memory::{MemoryItem, MemoryStore, PrunePolicy};
use std::time::Instant;

const RECORDS: usize = 5_000;
const CONTENT_BYTES: usize = 256;
const AGED: usize = 2_500;

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("aster_core::memory_retention=debug")
        .with_ansi(false)
        .with_writer(std::io::stderr)
        .init();
    let root = tempfile::tempdir()?;
    let store = LazyFileMemoryStore::new(root.path().join("memory 空间"));
    let now = chrono::Utc::now();
    let started = Instant::now();
    for index in 0..RECORDS {
        let prefix = format!("retention {index:04} ");
        let content = format!("{prefix}{}", "x".repeat(CONTENT_BYTES - prefix.len()));
        let mut item = MemoryItem::new(content).with_importance(0.1);
        item.id = format!("memory-{index:04}");
        item.timestamp = now;
        if index < AGED {
            item.timestamp -= chrono::Duration::days(400);
            if index % 5 == 0 {
                item.tags.push("pinned".into());
            }
        }
        store.store(item).await?;
    }
    let seed_ms = started.elapsed().as_secs_f64() * 1_000.0;
    anyhow::ensure!(
        store.count().await? == RECORDS,
        "workload did not seed every record"
    );
    let started = Instant::now();
    let deleted = store.prune(&PrunePolicy::default()).await?;
    let prune_ms = started.elapsed().as_secs_f64() * 1_000.0;
    let remaining = store.count().await?;
    anyhow::ensure!(
        deleted == 2_000 && remaining == 3_000,
        "retention result changed"
    );
    println!(
        "{}",
        serde_json::json!({
            "profile": "native-primary-retention-5000-v1",
            "records": RECORDS, "contentBytesPerRecord": CONTENT_BYTES,
            "sourceBytes": RECORDS * CONTENT_BYTES, "agedRecords": AGED,
            "protectedAgedRecords": 500, "deleted": deleted, "remaining": remaining,
            "seedMs": seed_ms, "pruneMs": prune_ms, "samples": 1,
            "semanticMTierQualified": false,
        })
    );
    Ok(())
}
