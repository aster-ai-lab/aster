//! Explicit catalog accounting probes; individual provider failures remain in the report.
use aster_core::CodeConfig;
use std::path::PathBuf;
#[path = "support/provider_usage.rs"]
mod provider_usage;

#[tokio::test]
#[ignore = "explicit provider-accounting campaign; one bounded actual Core stream per configured model"]
async fn inspect_all_declared_model_stream_usage() {
    let source =
        PathBuf::from(std::env::var_os("ASTER_CONFIG_FILE").expect("explicit trusted config"));
    let output =
        PathBuf::from(std::env::var_os("ASTER_MODEL_USAGE_REPORT").expect("explicit report"));
    assert!(source.is_absolute() && output.is_absolute());
    let config =
        CodeConfig::from_file(&source).unwrap_or_else(|_| panic!("trusted config refused"));
    let mut reports = Vec::new();
    for (provider, model) in config.list_models() {
        let selected = format!("{}/{}", provider.name, model.id);
        let llm = config
            .llm_config(&provider.name, &model.id)
            .expect("declared model");
        let report = provider_usage::probe(llm, &selected).await;
        println!("MODEL_USAGE_PROBE {report}");
        reports.push(report);
        std::fs::write(&output, serde_json::to_vec_pretty(&reports).unwrap()).unwrap();
    }
    assert_eq!(reports.len(), config.list_models().len());
}
