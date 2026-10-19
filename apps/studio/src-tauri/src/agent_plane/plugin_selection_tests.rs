#![cfg(test)]

use super::*;

fn config(root: PathBuf) -> PlaneConfig {
    PlaneConfig {
        root: Some(root),
        credentials: std::sync::Arc::new(|_| None),
        binary: None,
        workspace_documents: None,
        schedule_mcp: None,
        mail_mcp: None,
        slug: String::new(),
        personal_wiki: None,
        cloud_agent_runtime: None,
    }
}

#[tokio::test]
async fn launcher_and_session_share_exact_tenant_selection_without_raw_tool_paths() {
    let root = std::env::temp_dir().join(format!(
        "studio-native-selection-launch-{}",
        uuid::Uuid::now_v7()
    ));
    let store = ReconcileStore::open(root.join("package-generations")).unwrap();
    let snapshot = store
        .publish_selection_sync("studio-installed-plugins", None, &[])
        .unwrap();
    let binding = current(&config(root.join("agent-plane")), &[])
        .await
        .unwrap()
        .unwrap();
    assert_eq!(binding.cursor, snapshot.cursor());
    let mut args = Vec::new();
    binding.append_launch_args(&mut args);
    assert_eq!(
        args,
        [
            "--plugin-store",
            root.join("package-generations").to_str().unwrap()
        ]
    );
    let mut params = json!({"cwd":"/workspace", "_meta":{"a3sHarness":{"components":["system"]}}});
    binding.bind_session(&mut params);
    assert_eq!(
        params["_meta"]["a3sPluginSelection"],
        json!(snapshot.cursor())
    );
    assert!(params["_meta"].get("a3sTools").is_none());
    store.withdraw_selection_sync(&snapshot.cursor()).unwrap();
    assert!(
        current(&config(root.join("agent-plane")), &[])
            .await
            .is_err()
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn missing_selection_refuses_legacy_installed_tool_admission() {
    let root = std::env::temp_dir().join(format!(
        "studio-native-selection-missing-{}",
        uuid::Uuid::now_v7()
    ));
    let config = config(root.join("agent-plane"));
    assert!(current(&config, &[]).await.unwrap().is_none());
    assert!(
        current(&config, &[json!({"id":"echo", "entry":"/bin/echo"})])
            .await
            .is_err()
    );
    std::fs::remove_dir_all(root).unwrap();
}

// Catch an upgraded Studio launching Code with a still-valid earlier-host package selection.
#[tokio::test]
async fn incompatible_aix_selection_refuses_harness_launch_before_binding() {
    use a3s_use_reconcile::{
        PackageFile, PackageSpec, Payload, SurfaceKind, SurfaceSpec, files_sha256,
    };
    let root = std::env::temp_dir().join(format!(
        "studio-native-host-version-{}",
        uuid::Uuid::now_v7()
    ));
    let store = ReconcileStore::open(root.join("package-generations")).unwrap();
    let acl = r#"extension "proof/old" {
        schema_version = 3 version = "1.0.0" requires_use = "=0.3.0"
        route = "old" actions = ["read"] repository { url = "https://example.org/proof/old" }
        skill "read" { path = "skills/read/SKILL.md" }
    }"#;
    let files = [
        ("a3s-use-extension.acl", acl),
        (
            "SKILL.md",
            "---\nname: read\ndescription: Read proof\n---\n# Read",
        ),
    ]
    .into_iter()
    .map(|(path, text)| PackageFile {
        path: path.into(),
        bytes: text.as_bytes().to_vec(),
    })
    .collect::<Vec<_>>();
    let old = store
        .apply_sync(&PackageSpec {
            package_id: "use/proof/old".into(),
            surfaces: vec![SurfaceSpec {
                kind: SurfaceKind::Skill,
                id: "read".into(),
                entry: None,
                sha256: files_sha256(&files).unwrap(),
                payload: Payload::Files(files),
            }],
        })
        .unwrap();
    let snapshot = store
        .publish_selection_sync("studio-installed-plugins", None, &[old.cursor()])
        .unwrap();
    let error = current(&config(root.join("agent-plane")), &[])
        .await
        .unwrap_err();
    assert!(error.contains("incompatible"));
    assert_eq!(
        store
            .selection_sync("studio-installed-plugins")
            .unwrap()
            .unwrap()
            .cursor(),
        snapshot.cursor()
    );
    let compatible = acl
        .replace("=0.3.0", ">=0.3.0, <0.4.0")
        .replace("1.0.0", "1.0.1");
    let files = [
        ("a3s-use-extension.acl", compatible.as_str()),
        (
            "SKILL.md",
            "---\nname: read\ndescription: Read proof\n---\n# Read",
        ),
    ]
    .into_iter()
    .map(|(path, text)| PackageFile {
        path: path.into(),
        bytes: text.as_bytes().to_vec(),
    })
    .collect::<Vec<_>>();
    let new = store
        .apply_sync(&PackageSpec {
            package_id: "use/proof/old".into(),
            surfaces: vec![SurfaceSpec {
                kind: SurfaceKind::Skill,
                id: "read".into(),
                entry: None,
                sha256: files_sha256(&files).unwrap(),
                payload: Payload::Files(files),
            }],
        })
        .unwrap();
    let selected = store
        .publish_selection_sync(
            "studio-installed-plugins",
            Some(&snapshot.cursor()),
            &[new.cursor()],
        )
        .unwrap();
    assert_eq!(
        current(&config(root.join("agent-plane")), &[])
            .await
            .unwrap()
            .unwrap()
            .cursor,
        selected.cursor()
    );
    std::fs::remove_dir_all(root).unwrap();
}
