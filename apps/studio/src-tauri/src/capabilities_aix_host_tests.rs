#![cfg(test)]

use super::super::{DIGEST_MARKER, LocalCapabilities, PluginPin, digest_of};
use super::*;
use a3s_use_reconcile::{PackageFile, PackageSpec, Payload, SurfaceSpec, files_sha256};
use serde_json::Value;
use std::io::{Cursor, Write};
use std::path::PathBuf;

struct PreviousHost {
    root: PathBuf,
    store: ReconcileStore,
    local: LocalCapabilities,
    bundles: BTreeMap<String, Vec<u8>>,
    pins: BTreeMap<String, PluginPin>,
}

impl PreviousHost {
    fn new(installed: bool) -> Self {
        let root = std::env::temp_dir().join(format!("studio-aix-host-{}", uuid::Uuid::now_v7()));
        let store = ReconcileStore::open(root.join("package-generations")).unwrap();
        let local = LocalCapabilities::open_at(Some(root.join("capabilities")));
        let mut fixture = Self {
            root,
            store,
            local,
            bundles: BTreeMap::new(),
            pins: BTreeMap::new(),
        };
        let mut cursors = Vec::new();
        for (slug, requirement) in [("old", "=0.3.0"), ("healthy", ">=0.3.0, <0.4.0")] {
            let acl = format!(
                r#"extension "proof/{slug}" {{
                schema_version = 3 version = "1.0.0" requires_use = "{requirement}"
                route = "{slug}" actions = ["read"] repository {{ url = "https://example.org/proof/{slug}" }}
                skill "read" {{ path = "skills/read/SKILL.md" }}
                ui "app" {{ entry = "ui/app/index.html" point = "studio.applets" }}
            }}"#
            );
            let parsed = ExtensionManifest::parse_acl(&acl).unwrap();
            assert!(parsed.supports_use_version("0.3.0").unwrap());
            let originals = [
                ("a3s-use-extension.acl", acl.as_str()),
                ("README.md", "# Host proof"),
                (
                    "skills/read/SKILL.md",
                    "---\nname: read\ndescription: Read proof\n---\n# Read",
                ),
                ("ui/app/index.html", "<html>owned view</html>"),
            ];
            let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
            for (path, body) in originals {
                zip.start_file(path, zip::write::SimpleFileOptions::default())
                    .unwrap();
                zip.write_all(body.as_bytes()).unwrap();
            }
            let archive = zip.finish().unwrap().into_inner();
            let pin = PluginPin {
                require_aix: true,
                package_id: format!("use/proof/{slug}"),
                digest: digest_of(&archive),
            };
            if installed {
                let mut surfaces = Vec::new();
                for (kind, id, entry, path, body) in [
                    (
                        SurfaceKind::Ui,
                        "app",
                        Some("ui/app/index.html"),
                        "ui/app/index.html",
                        "<html>owned view</html>",
                    ),
                    (SurfaceKind::Skill, "read", None, "SKILL.md", originals[2].1),
                ] {
                    let files = [
                        ("a3s-use-extension.acl", acl.as_str()),
                        ("README.md", "# Host proof"),
                        (path, body),
                    ]
                    .into_iter()
                    .map(|(path, body)| PackageFile {
                        path: path.into(),
                        bytes: body.as_bytes().to_vec(),
                    })
                    .collect::<Vec<_>>();
                    surfaces.push(SurfaceSpec {
                        kind,
                        id: id.into(),
                        sha256: files_sha256(&files).unwrap(),
                        entry: entry.map(str::to_owned),
                        payload: Payload::Files(files),
                    });
                }
                // A previous compatible host published these real immutable generations.
                let publication = fixture
                    .store
                    .apply_sync(&PackageSpec {
                        package_id: pin.package_id.clone(),
                        surfaces,
                    })
                    .unwrap();
                cursors.push(publication.cursor());
                let target = fixture.root.join("capabilities/plugins").join(slug);
                for (path, body) in originals {
                    let path = target.join(path);
                    fs::create_dir_all(path.parent().unwrap()).unwrap();
                    fs::write(path, body).unwrap();
                }
                fs::write(target.join(PACKAGE_ID_MARKER), &pin.package_id).unwrap();
                fs::write(target.join(DIGEST_MARKER), &pin.digest).unwrap();
            }
            fixture.bundles.insert(slug.into(), archive);
            fixture.pins.insert(slug.into(), pin);
        }
        if installed {
            fixture
                .store
                .publish_selection_sync("studio-installed-plugins", None, &cursors)
                .unwrap();
        }
        fixture
    }
}

impl Drop for PreviousHost {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

// Catch view admission checking only UI declarations while other native surfaces disagree.
#[test]
fn workbench_refuses_aix_with_incompatible_divergent_or_missing_skill_manifest() {
    for (replacement, skill_id, include_skill, omit_ui_manifest) in [
        (Some("=0.3.0"), "read", true, false),
        (Some(">=0.3.1, <0.4.0"), "read", true, false),
        (None, "read", true, false),
        (Some(">=0.3.0, <0.4.0"), "undeclared", true, false),
        (Some(">=0.3.0, <0.4.0"), "read", false, false),
        (Some(">=0.3.0, <0.4.0"), "read", true, true),
    ] {
        let f = PreviousHost::new(true);
        let healthy = f.store.current_sync("use/proof/healthy").unwrap().unwrap();
        let mut surfaces = Vec::new();
        for surface in &healthy.surfaces {
            if surface.kind == SurfaceKind::Skill && !include_skill {
                continue;
            }
            let mut files = Vec::new();
            for path in [
                "a3s-use-extension.acl",
                "README.md",
                if surface.kind == SurfaceKind::Ui {
                    "ui/app/index.html"
                } else {
                    "SKILL.md"
                },
            ] {
                if omit_ui_manifest
                    && surface.kind == SurfaceKind::Ui
                    && path == "a3s-use-extension.acl"
                {
                    continue;
                }
                let mut bytes = fs::read(surface.directory.join(path)).unwrap();
                if surface.kind == SurfaceKind::Skill && path == "a3s-use-extension.acl" {
                    let Some(requirement) = replacement else {
                        continue;
                    };
                    bytes = String::from_utf8(bytes)
                        .unwrap()
                        .replace(">=0.3.0, <0.4.0", requirement)
                        .into_bytes();
                }
                files.push(PackageFile {
                    path: path.into(),
                    bytes,
                });
            }
            surfaces.push(SurfaceSpec {
                kind: surface.kind,
                id: if surface.kind == SurfaceKind::Skill {
                    skill_id.into()
                } else {
                    surface.id.clone()
                },
                entry: (surface.kind == SurfaceKind::Ui).then(|| "ui/app/index.html".into()),
                sha256: files_sha256(&files).unwrap(),
                payload: Payload::Files(files),
            });
        }
        let next = f
            .store
            .apply_sync(&PackageSpec {
                package_id: healthy.package_id.clone(),
                surfaces,
            })
            .unwrap();
        let before = f
            .store
            .selection_sync("studio-installed-plugins")
            .unwrap()
            .unwrap()
            .cursor();
        let old = f.store.current_sync("use/proof/old").unwrap().unwrap();
        f.store
            .publish_selection_sync(
                "studio-installed-plugins",
                Some(&before),
                &[old.cursor(), next.cursor()],
            )
            .unwrap();
        assert!(f.local.callable_plugin("healthy").is_err());
        let catalogue: Value = serde_json::from_slice(&f.local.plugin_views().unwrap()).unwrap();
        assert!(
            catalogue["packages"].as_array().unwrap().is_empty(),
            "{replacement:?}"
        );
        assert_eq!(catalogue["diagnostics"].as_array().unwrap().len(), 2);
        let request = json!({"protocolVersion":VIEW_PROTOCOL,
            "requestId":format!("{:064x}", uuid::Uuid::now_v7().as_u128()),
            "activationId":format!("{:064x}", uuid::Uuid::now_v7().as_u128()),
            "instanceId":format!("{:064x}", uuid::Uuid::now_v7().as_u128()),
            "expiresAt":now_ms().unwrap()+ADMISSION_LIFETIME_MS,
            "slug":"healthy", "surface":"app", "revision":next.cursor().revision });
        assert!(
            f.local
                .open_plugin_view("member", &serde_json::to_vec(&request).unwrap())
                .is_err()
        );
        let mut sessions = UiSessions::default();
        let token = "ab".repeat(32);
        sessions.sessions.insert(
            token.clone(),
            UiSession {
                request_id: "cd".repeat(32),
                activation_id: "ef".repeat(32),
                instance_id: "01".repeat(32),
                admission_deadline: now_ms().unwrap() + ADMISSION_LIFETIME_MS,
                owner: "member".into(),
                slug: "healthy".into(),
                surface: "app".into(),
                package_id: next.package_id.clone(),
                package_generation: next.generation,
                publication_revision: next.cursor().revision,
                expires: Instant::now() + SESSION_LIFETIME,
            },
        );
        assert!(
            sessions
                .asset(
                    &f.root.join("capabilities"),
                    "member",
                    &token,
                    "ui/app/index.html"
                )
                .is_err()
        );
    }
}

// Catch syntactically valid version requirements bypassing current host admission.
#[test]
fn incompatible_aix_refuses_fresh_install_before_publication() {
    let f = PreviousHost::new(false);
    let pin = &f.pins["old"];
    let entry = json!({"slug":"old", "kind":"plugin", "installed":true, "install":{
        "type":"plugin", "packageType":"aix", "available":true, "packageId":pin.package_id,
        "digest":pin.digest, "size":f.bundles["old"].len() }});
    let error = f
        .local
        .reconcile(&[entry], |_| Ok(f.bundles["old"].clone()))
        .unwrap_err();
    assert!(error.to_string().contains("incompatible"));
    assert!(f.store.current_sync(&pin.package_id).unwrap().is_none());
    assert!(!f.root.join("capabilities/plugins/old").exists());
}

// Catch restart adoption and callable projections trusting an earlier host's acceptance.
#[test]
fn incompatible_previous_host_generation_refuses_reconcile_and_callable_projection() {
    let f = PreviousHost::new(true);
    for (slug, replacement) in [("old", ">=0.3.0, <0.4.0"), ("healthy", "=0.3.0")] {
        let path = f
            .root
            .join("capabilities/plugins")
            .join(slug)
            .join("a3s-use-extension.acl");
        let text = fs::read_to_string(&path).unwrap();
        let original = if slug == "old" {
            "=0.3.0"
        } else {
            ">=0.3.0, <0.4.0"
        };
        fs::write(path, text.replace(original, replacement)).unwrap();
    }
    assert!(f.local.callable_plugin("old").is_err());
    assert!(f.local.plugin_ui("old", "app").is_err());
    assert!(f.local.callable_plugin("healthy").is_ok());
    assert!(f.local.plugin_ui("healthy", "app").is_ok());
    let before = f
        .store
        .selection_sync("studio-installed-plugins")
        .unwrap()
        .unwrap()
        .cursor();
    let error = super::super::sync_plugins(
        &f.root.join("capabilities"),
        &f.root.join("agents"),
        &f.pins,
        &|_| panic!("host incompatibility cannot trigger an archive fallback"),
    )
    .unwrap_err();
    assert!(error.to_string().contains("incompatible"));
    assert_eq!(
        f.store
            .selection_sync("studio-installed-plugins")
            .unwrap()
            .unwrap()
            .cursor(),
        before
    );
    assert!(f.local.callable_plugin("healthy").is_ok());
}

// Catch a still-content-valid previous-host ticket serving assets after a host upgrade.
#[test]
fn incompatible_previous_host_views_and_cached_assets_withdraw_without_harming_sibling() {
    let f = PreviousHost::new(true);
    let old = f.store.current_sync("use/proof/old").unwrap().unwrap();
    let token = "previous-host-ticket";
    let mut sessions = UiSessions::default();
    sessions.sessions.insert(
        token.into(),
        UiSession {
            request_id: "previous-host-request".into(),
            activation_id: "previous-host-activation".into(),
            instance_id: "previous-host-instance".into(),
            admission_deadline: now_ms().unwrap() + ADMISSION_LIFETIME_MS,
            owner: "member".into(),
            slug: "old".into(),
            surface: "app".into(),
            package_id: old.package_id.clone(),
            package_generation: old.generation,
            publication_revision: old.cursor().revision,
            expires: Instant::now() + SESSION_LIFETIME,
        },
    );
    assert!(
        sessions
            .asset(
                &f.root.join("capabilities"),
                "member",
                token,
                "ui/app/index.html"
            )
            .is_err()
    );
    let catalogue: serde_json::Value =
        serde_json::from_slice(&f.local.plugin_views().unwrap()).unwrap();
    assert_eq!(catalogue["packages"].as_array().unwrap().len(), 1);
    assert_eq!(catalogue["packages"][0]["slug"], "healthy");
    let request = |slug: &str, revision: &str| {
        json!({"protocolVersion":VIEW_PROTOCOL,
        "requestId":format!("{:064x}", uuid::Uuid::now_v7().as_u128()),
        "activationId":format!("{:064x}", uuid::Uuid::now_v7().as_u128()),
        "instanceId":format!("{:064x}", uuid::Uuid::now_v7().as_u128()),
        "expiresAt":now_ms().unwrap()+ADMISSION_LIFETIME_MS,
        "slug":slug, "surface":"app", "revision":revision })
    };
    assert!(
        f.local
            .open_plugin_view(
                "member",
                &serde_json::to_vec(&request("old", &old.cursor().revision)).unwrap()
            )
            .is_err()
    );
    let healthy = f.store.current_sync("use/proof/healthy").unwrap().unwrap();
    let opened: Value = serde_json::from_slice(
        &f.local
            .open_plugin_view(
                "member",
                &serde_json::to_vec(&request("healthy", &healthy.cursor().revision)).unwrap(),
            )
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        f.local
            .plugin_view_asset(
                "member",
                opened["token"].as_str().unwrap(),
                "ui/app/index.html"
            )
            .unwrap()
            .1,
        b"<html>owned view</html>"
    );
}
