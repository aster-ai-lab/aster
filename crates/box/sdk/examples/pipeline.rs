//! A tiny CI pipeline driven from Rust. Needs `aster-box` + /dev/kvm.
//!
//! Run: `ASTER_BOX=/path/to/aster-box cargo run -p aster-box-sdk --example pipeline`
//! (set `DEMO_IMAGE` to override the alpine default).

use aster_box_sdk::pipeline::{warm_base, FileCache, Step, WarmBase};

fn main() -> Result<(), aster_box_sdk::pipeline::PipelineError> {
    let image = std::env::var("DEMO_IMAGE")
        .unwrap_or_else(|_| "docker.m.daocloud.io/library/alpine:latest".to_string());
    let _ = std::fs::remove_dir_all("/tmp/.aster-ci-demo"); // fresh cache for the demo (before create)
    let cache = FileCache::new("/tmp/.aster-ci-demo")?;

    // Warm the base once (here: write a marker = "deps installed"), snapshot it.
    let base = warm_base(WarmBase::new(image, "echo DEPS-INSTALLED > /warmed").cache(&cache))?;

    // Sequential, fail-fast: a non-zero exit returns Err.
    let r = base.step(Step::new("read", "cat /warmed"))?;
    println!("read   -> code={} out={:?}", r.exit_code, r.stdout.trim());
    let r2 = base.step(Step::new("read", "cat /warmed"))?; // identical -> cache hit
    println!("read#2 -> cached={}", r2.cached);

    // Parallel matrix, collect-all: each step is an isolated CoW fork of the base.
    // A step reports a metric by printing `::metric key=value`.
    let report = base.run_parallel(
        vec![
            Step::new("ok", "echo fine"),
            Step::new("perf", "echo '::metric duration_ms=12.5'"),
            Step::new("fail", "exit 7"), // collected, not fatal
        ],
        4,
    );
    println!("report -> {}", report.to_json());
    println!(
        "passed={} failures={:?}",
        report.passed,
        report
            .failures()
            .iter()
            .map(|s| &s.name)
            .collect::<Vec<_>>()
    );

    // base.dispose() is optional — the snapshot is removed when `base` drops.
    println!("demo ok");
    Ok(())
}
