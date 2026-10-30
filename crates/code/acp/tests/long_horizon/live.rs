#![cfg(all(test, unix))]

use super::{candidate::Candidate, config, driver::Driver, fixture, relay::Relay, upstream};
use a3s_code_core::config::CodeConfig;
use std::path::{Path, PathBuf};

pub fn build_id(binary: &Path) -> Result<String, &'static str> {
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
    let source = Path::new(env!("CARGO_BIN_EXE_a3s-code-acp"));
    let expected = env!("ASTER_PAIR_BUILD_ID");
    let candidate = Candidate::from_environment(expected)
        .expect("registered candidate must match its source and bytes");
    std::fs::create_dir(directory).expect("new qualification artifact directory");
    if let Some(candidate) = &candidate {
        candidate
            .copy_into(directory)
            .expect("freeze the registered pair without canonical fallback");
    } else {
        for name in ["aster", "a3s-code-acp"] {
            let source = source.parent().unwrap().join(name);
            assert_eq!(
                build_id(&source).unwrap(),
                expected,
                "build the current CLI pair together"
            );
            let hash = fixture::digest(&source).unwrap();
            let frozen = directory.join(name);
            std::fs::copy(&source, &frozen).unwrap();
            assert_eq!(
                fixture::digest(&frozen).unwrap(),
                hash,
                "binary changed during freeze"
            );
            assert_eq!(build_id(&frozen).unwrap(), expected);
        }
    }
    for name in ["aster", "a3s-code-acp"] {
        assert_eq!(build_id(&directory.join(name)).unwrap(), expected);
    }
    std::fs::write(directory.join("pair-input.json"), serde_json::to_vec_pretty(&serde_json::json!({
        "registered_candidate":candidate.is_some(), "candidate":candidate, "pair_build_id":expected,
    })).unwrap()).unwrap();
    let test = std::env::current_exe().unwrap();
    let test_hash = fixture::digest(&test).unwrap();
    let frozen_test = directory.join("long-horizon-tests");
    std::fs::copy(&test, &frozen_test).unwrap();
    assert_eq!(fixture::digest(&frozen_test).unwrap(), test_hash);
    std::fs::write(directory.join("test-identity.json"),serde_json::to_vec_pretty(&serde_json::json!({"sha256":test_hash,"pair_build_id":expected,"scope":"exact executed test bytes retained; registered candidate selection is independently recorded in pair-input.json"})).unwrap()).unwrap();
    (directory.join("a3s-code-acp"), directory.join("aster"))
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
    pub relay: Option<Relay>,
    acl: String,
}

impl Campaign {
    pub async fn prepare(prefix: &str) -> Self {
        let selected = std::env::var("A3S_LIVE_MODEL").expect("explicit A3S_LIVE_MODEL required");
        let source = PathBuf::from(
            std::env::var_os("A3S_CONFIG_FILE").expect("explicit trusted ACL required"),
        );
        let evidence = PathBuf::from(
            std::env::var_os("A3S_LH_EVIDENCE_DIR").expect("explicit evidence directory required"),
        );
        assert!(source.is_absolute() && evidence.is_absolute());
        let trusted = CodeConfig::from_file(&source)
            .unwrap_or_else(|_| panic!("trusted ACL refused; source withheld"));
        let (provider_id, model_id) = selected
            .split_once('/')
            .expect("exact provider/model required");
        let (provider, model) = trusted
            .list_models()
            .into_iter()
            .find(|(p, m)| p.name == provider_id && m.id == model_id)
            .expect("model must be explicitly declared");
        let route = upstream::openai_route(
            trusted
                .llm_config(provider_id, model_id)
                .expect("declared LLM configuration"),
        )
        .await
        .expect("supported real relay profile");
        let expected = env!("ASTER_PAIR_BUILD_ID");
        let root = tempfile::tempdir().unwrap();
        let home = root.path().join("home");
        std::fs::create_dir_all(home.join("tmp")).unwrap();
        let name = format!("{prefix}-{}", uuid::Uuid::new_v4());
        let output = evidence.join(&name);
        std::fs::create_dir(&output).unwrap();
        let frozen = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(3)
            .unwrap()
            .join("target/qualification")
            .join(&name);
        let (binary, aster) = freeze_pair(&frozen);
        let relay = Relay::start(route, 40, 4096).await.unwrap();
        let mut model = model.clone();
        model.limit.output = if model.limit.output == 0 {
            4096
        } else {
            model.limit.output.min(4096)
        };
        let acl = config::isolated_acl(provider, &model, relay.endpoint(), &home.join("state"), 0)
            .unwrap();
        Self {
            acp_hash: fixture::digest(&binary).unwrap(),
            aster_hash: fixture::digest(&aster).unwrap(),
            root,
            home,
            binary,
            aster,
            build_id: expected.into(),
            name,
            output,
            model: selected,
            relay: Some(relay),
            acl,
        }
    }

    pub fn spawn(
        &self,
        workspace: &Path,
        assets: Option<&Path>,
        rust_verification: bool,
    ) -> Driver {
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
        fixture::digest(&self.binary).ok().as_ref() == Some(&self.acp_hash)
            && fixture::digest(&self.aster).ok().as_ref() == Some(&self.aster_hash)
    }
}
