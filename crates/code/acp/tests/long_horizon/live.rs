#![cfg(all(test, unix))]

use super::artifact::timing::{self, Phase, Subject};
use super::{artifact, candidate::Candidate, config, driver::Driver, relay::Relay, upstream};
use aster_core::config::CodeConfig;
use std::path::{Path, PathBuf};

pub fn qualification_output_tokens() -> u32 {
    admitted_output_tokens(std::env::var_os("ASTER_LH_OUTPUT_TOKENS").as_deref())
        .expect("registered output token budget")
}

fn admitted_output_tokens(value: Option<&std::ffi::OsStr>) -> Result<u32, &'static str> {
    match value {
        None => Ok(4096),
        Some(value) if value == "4096" => Ok(4096),
        Some(value) if value == "8192" => Ok(8192),
        Some(_) => Err("ASTER_LH_OUTPUT_TOKENS must be 4096 or 8192"),
    }
}

#[path = "output_budget_tests.rs"]
mod output_budget_tests;

pub fn build_id(binary: &Path) -> Result<String, &'static str> {
    let _timing = timing::span_for(
        Phase::BuildIdQuery,
        Some(Subject::Executable(
            binary
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
        )),
    );
    let output = std::process::Command::new(binary)
        .arg("--build-id")
        .output()
        .map_err(|_| "build_identity_query")?;
    if !output.status.success() || output.stdout.len() > 512 {
        return Err("build_identity_output");
    }
    String::from_utf8(output.stdout)
        .map(|id| id.trim().to_owned())
        .map_err(|_| "build_identity_encoding")
}

pub fn freeze_pair(directory: &Path) -> (PathBuf, PathBuf) {
    let _timing = timing::span(Phase::FreezePair);
    let source = Path::new(env!("CARGO_BIN_EXE_aster-acp"));
    let expected = env!("ASTER_PAIR_BUILD_ID");
    let candidate = Candidate::from_environment(expected)
        .expect("registered candidate must match its source and bytes");
    std::fs::create_dir(directory).expect("new qualification artifact directory");
    if let Some(candidate) = &candidate {
        candidate
            .copy_into(directory)
            .expect("freeze the registered pair without canonical fallback");
    } else {
        for name in ["aster", "aster-acp"] {
            let source = source.parent().unwrap().join(name);
            assert_eq!(
                build_id(&source).unwrap(),
                expected,
                "build the current CLI pair together"
            );
            let hash = artifact::digest(&source).unwrap();
            let frozen = directory.join(name);
            std::fs::copy(&source, &frozen).unwrap();
            assert_eq!(
                artifact::digest(&frozen).unwrap(),
                hash,
                "binary changed during freeze"
            );
            assert_eq!(build_id(&frozen).unwrap(), expected);
        }
    }
    for name in ["aster", "aster-acp"] {
        assert_eq!(build_id(&directory.join(name)).unwrap(), expected);
    }
    std::fs::write(directory.join("pair-input.json"), serde_json::to_vec_pretty(&serde_json::json!({
        "registered_candidate":candidate.is_some(), "candidate":candidate, "pair_build_id":expected,
    })).unwrap()).unwrap();
    let test = std::env::current_exe().unwrap();
    let test_hash = artifact::digest(&test).unwrap();
    let frozen_test = directory.join("long-horizon-tests");
    {
        let _timing = timing::span(Phase::TesterCopy);
        std::fs::copy(&test, &frozen_test).unwrap();
    }
    assert_eq!(artifact::digest(&frozen_test).unwrap(), test_hash);
    std::fs::write(directory.join("test-identity.json"),serde_json::to_vec_pretty(&serde_json::json!({"sha256":test_hash,"pair_build_id":expected,"scope":"exact executed test bytes retained; registered candidate selection is independently recorded in pair-input.json"})).unwrap()).unwrap();
    (directory.join("aster-acp"), directory.join("aster"))
}

pub struct Campaign {
    pub root: tempfile::TempDir,
    pub home: PathBuf,
    pub binary: PathBuf,
    pub aster: PathBuf,
    pub build_id: String,
    pub acp_hash: String,
    pub aster_hash: String,
    pub name: String,
    pub output: PathBuf,
    pub model: String,
    pub output_tokens: u32,
    pub relay: Option<Relay>,
    pub(crate) acl: String,
    _timing: timing::Guard,
}

impl Campaign {
    pub async fn prepare(prefix: &str) -> Self {
        Self::prepare_with_context(prefix, None).await
    }

    pub async fn prepare_with_context(prefix: &str, context: Option<u32>) -> Self {
        Self::prepare_with_output(prefix, context, qualification_output_tokens()).await
    }

    pub async fn prepare_with_output(
        prefix: &str,
        context: Option<u32>,
        output_tokens: u32,
    ) -> Self {
        let selected =
            std::env::var("ASTER_LIVE_MODEL").expect("explicit ASTER_LIVE_MODEL required");
        let source = PathBuf::from(
            std::env::var_os("ASTER_CONFIG_FILE").expect("explicit trusted ACL required"),
        );
        let evidence = PathBuf::from(
            std::env::var_os("ASTER_LH_EVIDENCE_DIR")
                .expect("explicit evidence directory required"),
        );
        assert!(source.is_absolute() && evidence.is_absolute());
        let name = format!("{prefix}-{}", uuid::Uuid::new_v4());
        let output = evidence.join(&name);
        std::fs::create_dir(&output).unwrap();
        let timing_directory = output.clone();
        let timing = tokio::task::spawn_blocking(move || timing::Guard::open(&timing_directory))
            .await
            .expect("timing writer initialization task")
            .expect("timing writer initialization");
        let _prepare_timing = timing::span(Phase::CampaignPrepare);
        let trusted = {
            let _timing = timing::span(Phase::ConfigLoad);
            CodeConfig::from_file(&source)
                .unwrap_or_else(|_| panic!("trusted ACL refused; source withheld"))
        };
        let (provider_id, model_id) = selected
            .split_once('/')
            .expect("exact provider/model required");
        let (provider, model) = trusted
            .list_models()
            .into_iter()
            .find(|(p, m)| p.name == provider_id && m.id == model_id)
            .expect("model must be explicitly declared");
        assert!(output_tokens > 0);
        if output_tokens > 4096 {
            assert!(model.limit.output == 0 || model.limit.output >= output_tokens);
        }
        let route_timing = timing::span(Phase::RouteDiscovery);
        let route = upstream::provider_route(
            trusted
                .llm_config(provider_id, model_id)
                .expect("declared LLM configuration"),
        )
        .await
        .expect("supported real relay profile");
        drop(route_timing);
        let expected = env!("ASTER_PAIR_BUILD_ID");
        let root = tempfile::tempdir().unwrap();
        let home = root.path().join("home");
        std::fs::create_dir_all(home.join("tmp")).unwrap();
        let frozen = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(3)
            .unwrap()
            .join("target/qualification")
            .join(&name);
        let (binary, aster) = freeze_pair(&frozen);
        let relay = Relay::start(route, 40, u64::from(output_tokens))
            .await
            .unwrap();
        let mut model = model.clone();
        if let Some(context) = context {
            assert!(context > 0 && (model.limit.context == 0 || model.limit.context >= context));
            model.limit.context = context;
        }
        model.limit.output = if model.limit.output == 0 {
            output_tokens
        } else {
            model.limit.output.min(output_tokens)
        };
        let acl = {
            let _timing = timing::span(Phase::IsolatedConfig);
            config::isolated_acl(provider, &model, relay.endpoint(), &home.join("state"), 0)
                .unwrap()
        };
        Self {
            acp_hash: artifact::digest(&binary).unwrap(),
            aster_hash: artifact::digest(&aster).unwrap(),
            root,
            home,
            binary,
            aster,
            build_id: expected.into(),
            name,
            output,
            model: selected,
            output_tokens: model.limit.output,
            relay: Some(relay),
            acl,
            _timing: timing,
        }
    }

    pub fn spawn(
        &self,
        workspace: &Path,
        assets: Option<&Path>,
        rust_verification: bool,
    ) -> Driver {
        let _timing = timing::span(Phase::CampaignSpawn);
        let acl = if rust_verification {
            config::with_rust_verification(&self.acl, workspace).unwrap()
        } else {
            self.acl.clone()
        };
        let path = self.root.path().join("isolated.acl");
        std::fs::write(&path, acl).unwrap();
        let driver = Driver::spawn(
            &self.binary,
            workspace,
            &self.home,
            &path,
            self.relay.as_ref().unwrap().key(),
            true,
            assets,
        )
        .expect("owned isolated ACP");
        std::fs::write(
            self.output.join("process-launch.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "pid":driver.process_id(), "parent_pid":std::process::id(), "workspace":workspace,
                "binary":self.binary, "scope":"actual Driver child; excludes build-id probes",
            }))
            .unwrap(),
        )
        .unwrap();
        driver
    }

    pub fn identity_unchanged(&self) -> bool {
        let _timing = timing::span(Phase::IdentityRecheck);
        artifact::digest(&self.binary).ok().as_ref() == Some(&self.acp_hash)
            && artifact::digest(&self.aster).ok().as_ref() == Some(&self.aster_hash)
    }
}
