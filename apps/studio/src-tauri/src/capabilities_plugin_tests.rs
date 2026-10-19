#![cfg(test)]

use std::io::{Cursor, Write};

use zip::{ZipWriter, write::SimpleFileOptions};

use super::*;

struct InstalledPlugin {
    root: PathBuf,
}

impl InstalledPlugin {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "studio-plugin-publication-{}",
            uuid::Uuid::now_v7()
        ));
        let caps = root.join("capabilities");
        let agents = root.join("agents");
        fs::create_dir_all(&agents).unwrap();
        fs::create_dir_all(caps.join("plugins")).unwrap();
        let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
        for (path, text) in [
            ("skills/reason/SKILL.md", "# Reason"),
            ("tools/echo", "echo ready"),
            ("mcp/server", "mcp fixture"),
            ("flows/reason.ts", "export async function run() {}"),
        ] {
            zip.start_file(path, SimpleFileOptions::default()).unwrap();
            zip.write_all(text.as_bytes()).unwrap();
        }
        let bytes = zip.finish().unwrap().into_inner();
        let pin = PluginPin {
            require_aix: false,
            package_id: "acme/composable".into(),
            digest: digest_of(&bytes),
        };
        install_plugin(&caps.join("plugins"), &agents, "composable", &pin, &|_| {
            Ok(bytes.clone())
        })
        .unwrap();
        Self { root }
    }

    fn caps(&self) -> PathBuf {
        self.root.join("capabilities")
    }
    fn raw(&self) -> PathBuf {
        self.caps().join("plugins/composable")
    }
    fn store(&self) -> ReconcileStore {
        ReconcileStore::open(self.root.join("package-generations")).unwrap()
    }
    fn query(&self) -> io::Result<Value> {
        let bytes = LocalCapabilities::open_at(Some(self.caps())).callable_plugin("composable")?;
        Ok(serde_json::from_slice(&bytes).unwrap())
    }
}

impl Drop for InstalledPlugin {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

// A leftover unpacked directory must not resurrect a withdrawn package.
#[test]
fn callable_withdraws_with_publication_even_when_unpacked_files_remain() {
    let plugin = InstalledPlugin::new();
    let record = plugin.query().unwrap();
    assert_eq!(record["packageId"], "acme/composable");
    for kind in ["skill", "mcp", "tool", "flow"] {
        assert!(
            record["surfaces"]
                .as_array()
                .unwrap()
                .iter()
                .any(|row| row["kind"] == kind)
        );
    }
    plugin.store().withdraw_sync("acme/composable").unwrap();
    assert!(plugin.raw().join("skills/reason/SKILL.md").is_file());
    assert_eq!(plugin.query().unwrap_err().kind(), io::ErrorKind::NotFound);
}

// Raw directory edits must neither supply a new capability nor replace a published entry.
#[test]
fn callable_uses_published_entries_and_ignores_unpublished_raw_files() {
    let plugin = InstalledPlugin::new();
    fs::write(plugin.raw().join("tools/echo"), b"changed raw tool").unwrap();
    fs::write(plugin.raw().join("tools/unpublished"), b"extra tool").unwrap();
    let record = plugin.query().unwrap();
    let rows = record["surfaces"].as_array().unwrap();
    assert!(!rows.iter().any(|row| row["id"] == "unpublished"));
    let entry = rows.iter().find(|row| row["kind"] == "tool").unwrap()["entry"]
        .as_str()
        .unwrap();
    assert!(
        Path::new(entry).starts_with(
            plugin
                .root
                .join("package-generations")
                .canonicalize()
                .unwrap()
        )
    );
    assert_eq!(fs::read(entry).unwrap(), b"echo ready");
}

#[test]
fn callable_refuses_changed_published_payload() {
    let plugin = InstalledPlugin::new();
    let current = plugin
        .store()
        .current_sync("acme/composable")
        .unwrap()
        .unwrap();
    let tool = current
        .surfaces
        .iter()
        .find(|surface| surface.kind == SurfaceKind::Tool)
        .unwrap();
    fs::write(tool.entry.as_ref().unwrap(), b"unverified bytes").unwrap();
    assert_eq!(
        plugin.query().unwrap_err().kind(),
        io::ErrorKind::InvalidData
    );
}

#[test]
fn uninstall_hides_admission_and_preserves_an_accepted_generation_until_release() {
    let plugin = InstalledPlugin::new();
    let store = plugin.store();
    let lease = store
        .acquire_current_sync("acme/composable")
        .unwrap()
        .unwrap();
    let entry = lease
        .publication()
        .surfaces
        .iter()
        .find(|surface| surface.kind == SurfaceKind::Tool)
        .unwrap()
        .entry
        .clone()
        .unwrap();
    remove_plugin(
        &plugin.caps().join("plugins"),
        &plugin.root.join("agents"),
        "composable",
    )
    .unwrap();
    assert!(!plugin.raw().exists());
    assert_eq!(plugin.query().unwrap_err().kind(), io::ErrorKind::NotFound);
    assert!(
        store
            .acquire_current_sync("acme/composable")
            .unwrap()
            .is_none()
    );
    assert_eq!(fs::read(&entry).unwrap(), b"echo ready");
    lease.release_sync().unwrap();
    assert!(!entry.exists());
}

#[test]
fn callable_follows_a_new_publication_without_rescanning_the_unpacked_directory() {
    let plugin = InstalledPlugin::new();
    let files = vec![PackageFile {
        path: "bin/replacement".into(),
        bytes: b"replacement tool".to_vec(),
    }];
    let next = plugin
        .store()
        .apply_sync(&PackageSpec {
            package_id: "acme/composable".into(),
            surfaces: vec![SurfaceSpec {
                kind: SurfaceKind::Tool,
                id: "replacement".into(),
                sha256: files_sha256(&files).unwrap(),
                entry: Some("bin/replacement".into()),
                payload: Payload::Files(files),
            }],
        })
        .unwrap();
    let record = plugin.query().unwrap();
    assert!(next.generation > 1);
    assert_eq!(record["generation"], next.generation);
    assert_eq!(
        record["packageDigest"],
        fs::read_to_string(plugin.raw().join(DIGEST_MARKER)).unwrap()
    );
    let rows = record["surfaces"].as_array().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["id"], "replacement");
    assert_eq!(
        fs::read(rows[0]["entry"].as_str().unwrap()).unwrap(),
        b"replacement tool"
    );
    assert!(plugin.raw().join("tools/echo").is_file());
}

#[test]
fn an_installed_agent_entry_is_the_published_file() {
    let root = std::env::temp_dir().join(format!("studio-agent-entry-{}", uuid::Uuid::now_v7()));
    let caps = root.join("capabilities");
    fs::create_dir_all(caps.join("plugins/weather")).unwrap();
    fs::write(
        caps.join("plugins/weather").join(PACKAGE_ID_MARKER),
        "use/a3s/weather",
    )
    .unwrap();
    let script = b"process.stdout.write('port 9\\n')\n".to_vec();
    let spec = PackageSpec {
        package_id: "use/a3s/weather".to_owned(),
        surfaces: vec![
            surface_files(
                SurfaceKind::Agent,
                "weather".to_owned(),
                Some("agent/run.mjs".to_owned()),
                vec![PackageFile {
                    path: "agent/run.mjs".to_owned(),
                    bytes: script.clone(),
                }],
            )
            .unwrap(),
        ],
    };
    ReconcileStore::open(root.join("package-generations"))
        .unwrap()
        .apply_sync(&spec)
        .unwrap();
    let entry = installed_agent_entry(&caps, "weather").unwrap();
    assert_eq!(fs::read(&entry).unwrap(), script);
    assert!(entry.ends_with("agent/run.mjs"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn callable_refuses_incomplete_or_invalid_installation_markers() {
    let plugin = InstalledPlugin::new();
    let marker = plugin.raw().join(DIGEST_MARKER);
    let bytes = fs::read(&marker).unwrap();
    fs::remove_file(&marker).unwrap();
    assert_eq!(plugin.query().unwrap_err().kind(), io::ErrorKind::NotFound);
    fs::write(&marker, b"invalid").unwrap();
    assert_eq!(
        plugin.query().unwrap_err().kind(),
        io::ErrorKind::InvalidData
    );
    fs::write(&marker, bytes).unwrap();
    fs::write(plugin.raw().join(PACKAGE_ID_MARKER), b"../outside").unwrap();
    assert_eq!(
        plugin.query().unwrap_err().kind(),
        io::ErrorKind::InvalidData
    );
}

#[cfg(unix)]
#[test]
fn callable_refuses_a_linked_installed_directory() {
    let plugin = InstalledPlugin::new();
    let target = plugin.root.join("elsewhere");
    fs::rename(plugin.raw(), &target).unwrap();
    std::os::unix::fs::symlink(target, plugin.raw()).unwrap();
    assert_eq!(plugin.query().unwrap_err().kind(), io::ErrorKind::NotFound);
}

#[test]
fn native_mcp_routing_omits_only_selected_package_projections_and_preserves_custom_conflicts() {
    let plugin = InstalledPlugin::new();
    let doc = json!({"mcpServers":{
        "selected":{"type":"stdio","command":"/managed/server","_a3sPlugin":"acme/composable"},
        "unselected":{"type":"stdio","command":"/managed/other","_a3sPlugin":"acme/other"},
        "custom":{"type":"stdio","command":"/custom/server"}
    }});
    fs::write(
        plugin.caps().join("mcp.json"),
        serde_json::to_vec(&doc).unwrap(),
    )
    .unwrap();
    let names: Vec<_> =
        acp_session_servers_excluding_native(&plugin.caps(), &["selected".into(), "custom".into()])
            .into_iter()
            .map(|server| server["name"].as_str().unwrap().to_owned())
            .collect();
    assert_eq!(names, ["custom", "unselected"]);
    let all = acp_session_servers(&plugin.caps());
    assert_eq!(all.len(), 3);
}
