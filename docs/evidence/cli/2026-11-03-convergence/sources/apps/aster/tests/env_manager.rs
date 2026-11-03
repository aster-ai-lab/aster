//! Isolated environment-manager tests against the actual Aster CLI binary.

use std::ffi::OsStr;
use std::path::Path;
use std::process::Output;
use std::time::Duration;

#[cfg(unix)]
use aster_use_tool_acquire::{
    CompanionFile, EnvDeclarations, LocalApply, ToolStore, project_env_lock_path,
    write_env_declarations,
};
use aster_use_tool_acquire::{EnvConfig, project_env_path, write_env_acl};
#[cfg(unix)]
use aster_use_tool_acquire::{StaticSource, VersionListing, parse_tool_spec};

fn isolated_command(project: &Path, home: &Path) -> tokio::process::Command {
    isolated_binary_command(Path::new(env!("CARGO_BIN_EXE_aster")), project, home)
}

fn isolated_binary_command(binary: &Path, project: &Path, home: &Path) -> tokio::process::Command {
    let mut command = tokio::process::Command::new(binary);
    command
        .env_clear()
        .env("ASTER_HOME", home)
        .current_dir(project)
        .arg("--no-auto-update")
        .kill_on_drop(true);
    for name in [
        "PATH",
        "SystemRoot",
        "COMSPEC",
        "PATHEXT",
        "TEMP",
        "TMP",
        "TMPDIR",
    ] {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }
    command
}

async fn run(project: &Path, home: &Path, args: &[impl AsRef<OsStr>]) -> Output {
    tokio::time::timeout(
        Duration::from_secs(20),
        isolated_command(project, home).args(args).output(),
    )
    .await
    .expect("fixture command timed out")
    .expect("spawn Aster CLI")
}

#[cfg(unix)]
async fn install_uv_fixture(store: &ToolStore, temporary: &Path, version: &str) {
    install_tool_fixture(store, temporary, "uv", "github:astral-sh/uv", version).await;
}

#[cfg(unix)]
async fn install_tool_fixture(
    store: &ToolStore,
    temporary: &Path,
    name: &str,
    spec: &str,
    version: &str,
) {
    let payload = temporary.join(format!("payload-{version}"));
    std::fs::write(&payload, format!("#!/bin/sh\nprintf '%s\\n' '{version}'\n")).unwrap();
    store
        .apply_local(LocalApply {
            name: name.into(),
            spec: spec.into(),
            version: version.into(),
            executable_name: name.into(),
            payload,
            sha256: None,
            require_checksum: false,
            companions: vec![CompanionFile {
                relative_path: "lib/required.txt".into(),
                bytes: b"required fixture".to_vec(),
                executable: false,
            }],
        })
        .await
        .unwrap();
}

#[cfg(unix)]
async fn install_multiversion_uv_fixture(store: &ToolStore, temporary: &Path, version: &str) {
    let payload = temporary.join(format!("uv-{version}"));
    std::fs::write(&payload, format!("#!/bin/sh\nprintf '%s\\n' '{version}'\n")).unwrap();
    let companions = if version == "2.0.0" {
        vec![CompanionFile {
            relative_path: "uv-extra".into(),
            bytes: b"#!/bin/sh\nprintf '%s\\n' 'extra:2.0.0'\n".to_vec(),
            executable: true,
        }]
    } else {
        Vec::new()
    };
    store
        .apply_local(LocalApply {
            name: "uv".into(),
            spec: "github:astral-sh/uv".into(),
            version: version.into(),
            executable_name: "uv".into(),
            payload,
            sha256: None,
            require_checksum: false,
            companions,
        })
        .await
        .unwrap();
}

#[cfg(unix)]
#[tokio::test]
async fn prune_matches_twelve_frozen_native_cases_including_a_running_executable() {
    async fn without_mise(project: &Path, home: &Path, args: &[impl AsRef<OsStr>]) -> Output {
        tokio::time::timeout(
            Duration::from_secs(20),
            isolated_command(project, home)
                .env("PATH", "/usr/bin:/bin")
                .args(args)
                .output(),
        )
        .await
        .expect("prune fixture command timed out")
        .expect("spawn actual Aster CLI")
    }
    #[derive(serde::Deserialize)]
    #[serde(untagged)]
    enum Requests {
        One(String),
        Many(Vec<String>),
    }
    #[derive(serde::Deserialize)]
    struct Case {
        name: String,
        declarations: Vec<(String, Requests)>,
        operation: String,
        flags: Vec<String>,
        expected_exit: i32,
        expected_retained: Vec<String>,
    }
    #[derive(serde::Deserialize)]
    struct Reference {
        cases: Vec<Case>,
    }
    let reference: Reference = serde_json::from_str(include_str!(
        "../../../specs/031-cli-mise-replacement/contracts/mise-prune-reference.json"
    ))
    .unwrap();
    assert_eq!(reference.cases.len(), 12);
    for case in reference.cases {
        let temporary = tempfile::tempdir().unwrap();
        let home = temporary.path().join("aster home 中文");
        let work = temporary.path().join("work");
        std::fs::create_dir_all(&work).unwrap();
        let store = ToolStore::open(home.join("tools")).unwrap();
        for version in ["1.0.0", "1.5.0", "2.0.0"] {
            if case.operation == "running" && version == "1.0.0" {
                let native = temporary.path().join("native sleep");
                let system = [Path::new("/bin/sleep"), Path::new("/usr/bin/sleep")]
                    .into_iter()
                    .find(|p| p.is_file())
                    .unwrap();
                std::fs::copy(system, &native).unwrap();
                #[cfg(target_os = "macos")]
                {
                    let output = std::process::Command::new("/usr/bin/codesign")
                        .args(["--force", "--sign", "-"])
                        .arg(&native)
                        .output()
                        .unwrap();
                    assert!(
                        output.status.success(),
                        "{}",
                        String::from_utf8_lossy(&output.stderr)
                    );
                }
                let payload = temporary.path().join("uv fixture");
                std::fs::write(&payload, "#!/bin/sh\nprintf '%s\\n' '1.0.0'\n").unwrap();
                store
                    .apply_local(LocalApply {
                        name: "uv".into(),
                        spec: "github:astral-sh/uv".into(),
                        version: version.into(),
                        executable_name: "uv".into(),
                        payload,
                        sha256: None,
                        require_checksum: false,
                        companions: vec![CompanionFile {
                            relative_path: "native-sleep".into(),
                            bytes: std::fs::read(native).unwrap(),
                            executable: true,
                        }],
                    })
                    .await
                    .unwrap();
            } else {
                install_uv_fixture(&store, temporary.path(), version).await;
            }
        }
        let mut configs = Vec::new();
        for (location, requests) in case.declarations {
            let project = if location == "global" {
                work.clone()
            } else {
                temporary.path().join(format!("project {location} 中文"))
            };
            std::fs::create_dir_all(&project).unwrap();
            let path = if location == "global" {
                aster_use_tool_acquire::user_env_path(&home)
            } else {
                project_env_path(&project)
            };
            let requests = match requests {
                Requests::One(request) => vec![request],
                Requests::Many(requests) => requests,
            };
            write_env_declarations(
                &path,
                &EnvDeclarations {
                    tools: std::collections::BTreeMap::from([("uv".into(), requests)]),
                    vars: std::collections::BTreeMap::new(),
                },
            )
            .unwrap();
            let output = without_mise(
                &project,
                &home,
                &["exec", "--offline", "--no-install", "--", "uv"],
            )
            .await;
            assert!(
                output.status.success(),
                "{}: {}",
                case.name,
                String::from_utf8_lossy(&output.stderr)
            );
            configs.push(path);
        }
        match case.operation.as_str() {
            "change" => write_env_declarations(
                &configs[0],
                &EnvDeclarations {
                    tools: std::collections::BTreeMap::from([("uv".into(), vec!["2.0.0".into()])]),
                    vars: std::collections::BTreeMap::new(),
                },
            )
            .unwrap(),
            "delete" => std::fs::remove_file(&configs[0]).unwrap(),
            "none" | "running" => {}
            other => panic!("unknown frozen operation {other}"),
        }
        let before = configs
            .iter()
            .filter(|p| p.is_file())
            .map(|p| (p.clone(), std::fs::read(p).unwrap()))
            .collect::<Vec<_>>();
        let tracking = home.join("tools/env-sources-v1.json");
        let tracking_before = std::fs::read(&tracking).ok();
        let mut held = if case.operation == "running" {
            Some(
                tokio::process::Command::new(
                    store.root_path().join("installs/uv/1.0.0/native-sleep"),
                )
                .arg("300")
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .kill_on_drop(true)
                .spawn()
                .unwrap(),
            )
        } else {
            None
        };
        if held.is_some() {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        let args = ["tool".to_owned(), "prune".to_owned(), "--json".to_owned()]
            .into_iter()
            .chain(case.flags.iter().cloned())
            .collect::<Vec<_>>();
        let output = without_mise(&work, &home, &args).await;
        assert_eq!(
            output.status.code(),
            Some(case.expected_exit),
            "{}: {}",
            case.name,
            String::from_utf8_lossy(&output.stderr)
        );
        let retained = store
            .all_uninstall_candidates()
            .unwrap()
            .into_iter()
            .map(|receipt| receipt.version)
            .collect::<Vec<_>>();
        assert_eq!(retained, case.expected_retained, "{}", case.name);
        for (path, bytes) in before {
            assert_eq!(std::fs::read(path).unwrap(), bytes, "{}", case.name);
        }
        if case.flags.iter().any(|flag| flag.starts_with("--dry-run")) {
            assert_eq!(
                std::fs::read(&tracking).ok(),
                tracking_before,
                "{}",
                case.name
            );
        }
        if case.flags.iter().any(|flag| flag == "--configs") {
            assert!(store.stale_env_sources().unwrap().is_empty());
        }
        for version in retained {
            let executable = store
                .root_path()
                .join("installs/uv")
                .join(&version)
                .join("uv");
            let result = tokio::process::Command::new(executable)
                .output()
                .await
                .unwrap();
            assert!(result.status.success());
            assert_eq!(String::from_utf8(result.stdout).unwrap().trim(), version);
        }
        if let Some(child) = held.as_mut() {
            assert!(child.try_wait().unwrap().is_none(), "{}", case.name);
            child.kill().await.unwrap();
            child.wait().await.unwrap();
        }
        eprintln!("prune reference passed: {}", case.name);
    }
}

#[cfg(unix)]
#[tokio::test]
async fn prune_keeps_a_running_script_and_its_later_dependency_read() {
    use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
    let temporary = tempfile::tempdir().unwrap();
    let home = temporary.path().join("项目 with spaces");
    let store = ToolStore::open(home.join("tools")).unwrap();
    let payload = temporary.path().join("script");
    std::fs::write(
        &payload,
        b"#!/bin/sh\nprintf 'ready\\n'\nread -r line\ncat \"$(dirname \"$0\")/lib/required.txt\"\n",
    )
    .unwrap();
    store
        .apply_local(LocalApply {
            name: "uv".into(),
            spec: "github:astral-sh/uv".into(),
            version: "1.0.0".into(),
            executable_name: "uv".into(),
            payload,
            sha256: None,
            require_checksum: false,
            companions: vec![CompanionFile {
                relative_path: "lib/required.txt".into(),
                bytes: b"required-after-prune\n".to_vec(),
                executable: false,
            }],
        })
        .await
        .unwrap();
    let mut child = tokio::process::Command::new("/bin/sh")
        .arg(store.root_path().join("installs/uv/1.0.0/uv"))
        .env("PATH", "/usr/bin:/bin")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let mut ready = String::new();
    tokio::time::timeout(Duration::from_secs(20), stdout.read_line(&mut ready))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(ready, "ready\n");
    let output = tokio::time::timeout(
        Duration::from_secs(20),
        isolated_command(temporary.path(), &home)
            .env("PATH", "/usr/bin:/bin")
            .args(["tool", "prune", "--yes", "--json"])
            .output(),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(child.try_wait().unwrap().is_none());
    assert_eq!(store.all_uninstall_candidates().unwrap().len(), 1);
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"continue\n")
        .await
        .unwrap();
    let mut result = String::new();
    tokio::time::timeout(Duration::from_secs(20), stdout.read_to_string(&mut result))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(result, "required-after-prune\n");
    assert!(
        tokio::time::timeout(Duration::from_secs(20), child.wait())
            .await
            .unwrap()
            .unwrap()
            .success()
    );
}

#[cfg(unix)]
#[tokio::test]
async fn project_uninstall_matches_eight_frozen_mise_cases_and_preserves_declarations() {
    #[derive(serde::Deserialize)]
    #[serde(untagged)]
    enum Requests {
        One(String),
        Many(Vec<String>),
    }
    #[derive(serde::Deserialize)]
    struct Case {
        name: String,
        requests: Option<std::collections::BTreeMap<String, Requests>>,
        flags: Vec<String>,
        exit: i32,
        retained: std::collections::BTreeMap<String, Vec<String>>,
    }
    #[derive(serde::Deserialize)]
    struct Reference {
        installations: std::collections::BTreeMap<String, Vec<String>>,
        cases: Vec<Case>,
    }
    let reference: Reference = serde_json::from_str(include_str!(
        "../../../specs/031-cli-mise-replacement/contracts/mise-project-uninstall-reference.json"
    ))
    .unwrap();
    let tool_name = |name: &str| match name {
        "fixture" => "uv",
        "other" => "fd",
        _ => panic!("unknown native reference fixture"),
    };
    let temporary = tempfile::tempdir().unwrap();
    for (index, case) in reference.cases.into_iter().enumerate() {
        let root = temporary.path().join(format!("project case 中文 {index}"));
        let home = root.join("home");
        let project = root.join("parent/project");
        let cwd = project.join("nested");
        std::fs::create_dir_all(&cwd).unwrap();
        let store = ToolStore::open(home.join("tools")).unwrap();
        for (name, versions) in &reference.installations {
            let tool = aster_use_tool_acquire::catalog_by_name(tool_name(name)).unwrap();
            for version in versions {
                install_tool_fixture(&store, &root, tool.name, tool.spec, version).await;
            }
        }
        let global_path = aster_use_tool_acquire::user_env_path(&home);
        write_env_acl(
            &global_path,
            &EnvConfig {
                tools: std::collections::BTreeMap::from([("fd".into(), "9.0.0".into())]),
                vars: Default::default(),
            },
        )
        .unwrap();
        let mut paths = vec![global_path];
        if let Some(requests) = case.requests {
            let parent = project_env_path(project.parent().unwrap());
            write_env_acl(
                &parent,
                &EnvConfig {
                    tools: std::collections::BTreeMap::from([("uv".into(), "2.0.0".into())]),
                    vars: Default::default(),
                },
            )
            .unwrap();
            let nearest = project_env_path(&project);
            write_env_declarations(
                &nearest,
                &EnvDeclarations {
                    tools: requests
                        .into_iter()
                        .map(|(name, requests)| {
                            let requests = match requests {
                                Requests::One(request) => vec![request],
                                Requests::Many(requests) => requests,
                            };
                            (tool_name(&name).into(), requests)
                        })
                        .collect(),
                    vars: Default::default(),
                },
            )
            .unwrap();
            paths.extend([parent, nearest]);
        }
        let before: Vec<_> = paths
            .iter()
            .map(|path| std::fs::read(path).unwrap())
            .collect();
        let mut args = vec!["tool", "uninstall", "--project", "--json"];
        args.extend(case.flags.iter().map(String::as_str));
        let output = run(&cwd, &home, &args).await;
        assert_eq!(
            output.status.code(),
            Some(case.exit),
            "{}: {}",
            case.name,
            String::from_utf8_lossy(&output.stderr)
        );
        for (name, expected) in case.retained {
            let name = tool_name(&name);
            let receipts = store.installed_versions(name).unwrap();
            assert_eq!(
                receipts
                    .iter()
                    .map(|receipt| &receipt.version)
                    .collect::<Vec<_>>(),
                expected.iter().collect::<Vec<_>>(),
                "{}: {name}",
                case.name
            );
            for receipt in receipts {
                let paths = store.installation_paths(&receipt).unwrap();
                let mut command = tokio::process::Command::new(paths.executable());
                command.env_clear().current_dir(&cwd).kill_on_drop(true);
                let result = tokio::time::timeout(Duration::from_secs(20), command.output())
                    .await
                    .unwrap()
                    .unwrap();
                assert!(result.status.success());
                assert_eq!(result.stdout, format!("{}\n", receipt.version).as_bytes());
            }
        }
        assert_eq!(
            paths
                .iter()
                .map(|path| std::fs::read(path).unwrap())
                .collect::<Vec<_>>(),
            before,
            "{}",
            case.name
        );
    }
}

#[cfg(unix)]
#[tokio::test]
async fn project_uninstall_removes_an_identified_version_with_a_missing_executable() {
    #[derive(serde::Deserialize)]
    struct Reference {
        exit: i32,
        retained_versions: Vec<String>,
        retained_stdout: String,
        declaration_unchanged: bool,
    }
    let reference: Reference = serde_json::from_str(include_str!(
        "../../../specs/031-cli-mise-replacement/contracts/mise-project-uninstall-damaged-reference.json"
    ))
    .unwrap();
    let temporary = tempfile::tempdir().unwrap();
    let home = temporary.path().join("home");
    let project = temporary.path().join("project");
    let store = ToolStore::open(home.join("tools")).unwrap();
    for version in ["1.0.0", "2.0.0"] {
        install_uv_fixture(&store, temporary.path(), version).await;
    }
    write_env_acl(
        project_env_path(&project),
        &EnvConfig {
            tools: std::collections::BTreeMap::from([("uv".into(), "1.0.0".into())]),
            vars: Default::default(),
        },
    )
    .unwrap();
    let receipt = store
        .installed_receipt("uv", Some("1.0.0"))
        .unwrap()
        .unwrap();
    let paths = store.installation_paths(&receipt).unwrap();
    std::fs::remove_file(paths.executable()).unwrap();
    let before = std::fs::read(project_env_path(&project)).unwrap();
    let output = run(
        &project,
        &home,
        &["tool", "uninstall", "--project", "--yes", "--json"],
    )
    .await;
    assert_eq!(
        output.status.code(),
        Some(reference.exit),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let retained = store.all_uninstall_candidates().unwrap();
    assert_eq!(
        retained
            .iter()
            .map(|receipt| receipt.version.as_str())
            .collect::<Vec<_>>(),
        reference.retained_versions
    );
    for receipt in retained {
        let paths = store.installation_paths(&receipt).unwrap();
        let result = tokio::time::timeout(
            Duration::from_secs(20),
            tokio::process::Command::new(paths.executable())
                .env_clear()
                .current_dir(&project)
                .kill_on_drop(true)
                .output(),
        )
        .await
        .unwrap()
        .unwrap();
        assert!(result.status.success());
        assert_eq!(result.stdout, reference.retained_stdout.as_bytes());
    }
    assert_eq!(
        std::fs::read(project_env_path(&project)).unwrap() == before,
        reference.declaration_unchanged
    );
}

#[cfg(unix)]
#[tokio::test]
async fn project_uninstall_preflights_flags_paths_and_nearest_acl_before_removal() {
    let temporary = tempfile::tempdir().unwrap();
    let home = temporary.path().join("home");
    let parent = temporary.path().join("parent");
    let project = parent.join("project");
    let nested = project.join("nested");
    std::fs::create_dir_all(&nested).unwrap();
    let store = ToolStore::open(home.join("tools")).unwrap();
    install_uv_fixture(&store, temporary.path(), "1.0.0").await;
    std::fs::create_dir_all(project_env_path(&parent).parent().unwrap()).unwrap();
    std::fs::write(project_env_path(&parent), "invalid outer declaration").unwrap();
    std::fs::write(
        aster_use_tool_acquire::user_env_path(&home),
        "invalid global declaration",
    )
    .unwrap();
    let config = EnvConfig {
        tools: std::collections::BTreeMap::from([("uv".into(), "1.0.0".into())]),
        vars: Default::default(),
    };
    let path = project_env_path(&project);
    write_env_acl(&path, &config).unwrap();
    for conflict in ["--all", "--unpin", "uv"] {
        let output = run(
            &nested,
            &home,
            &["tool", "uninstall", "--project", "--yes", conflict],
        )
        .await;
        assert_eq!(output.status.code(), Some(2));
        assert!(
            store
                .installed_receipt("uv", Some("1.0.0"))
                .unwrap()
                .is_some()
        );
    }
    std::fs::write(&path, "invalid nearest declaration").unwrap();
    let output = run(&nested, &home, &["tool", "uninstall", "--project", "--yes"]).await;
    assert!(!output.status.success());
    assert!(
        store
            .installed_receipt("uv", Some("1.0.0"))
            .unwrap()
            .is_some()
    );
    // APFS rejects this filename before the CLI can inspect it.
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::ffi::OsStringExt;
        let non_utf8 = temporary
            .path()
            .join(std::ffi::OsString::from_vec(b"project-\xff".to_vec()));
        write_env_acl(project_env_path(&non_utf8), &config).unwrap();
        let output = run(
            &non_utf8,
            &home,
            &["tool", "uninstall", "--project", "--yes", "--json"],
        )
        .await;
        assert!(!output.status.success());
        assert!(
            store
                .installed_receipt("uv", Some("1.0.0"))
                .unwrap()
                .is_some()
        );
    }
    write_env_acl(&path, &config).unwrap();
    let before = std::fs::read(&path).unwrap();
    let output = run(
        &nested,
        &home,
        &["tool", "uninstall", "--project", "--yes", "--json"],
    )
    .await;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        store
            .installed_receipt("uv", Some("1.0.0"))
            .unwrap()
            .is_none()
    );
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

#[cfg(unix)]
#[tokio::test]
async fn project_shims_match_frozen_native_mise_dispatch_and_repair_cases() {
    #[derive(serde::Deserialize)]
    struct Reference {
        cases: Vec<Case>,
    }
    #[derive(serde::Deserialize)]
    struct Case {
        name: String,
        exit: i32,
        stdout: String,
    }
    let reference: Reference = serde_json::from_str(include_str!(
        "../../../specs/031-cli-mise-replacement/contracts/mise-shims-reference.json"
    ))
    .unwrap();
    let temporary = tempfile::tempdir().unwrap();
    let home = temporary.path().join("shim home 中文 '$literal");
    let store = ToolStore::open(home.join("tools")).unwrap();
    for version in ["1.0.0", "2.0.0"] {
        let payload = temporary.path().join(format!("shim-{version}"));
        std::fs::write(
            &payload,
            format!("#!/bin/sh\nprintf '%s\\n' '{version}'\nprintf '%s\\n' \"$@\"\n"),
        )
        .unwrap();
        store
            .apply_local(LocalApply {
                name: "uv".into(),
                spec: "github:astral-sh/uv".into(),
                version: version.into(),
                executable_name: "uv".into(),
                payload,
                sha256: None,
                require_checksum: false,
                companions: if version == "2.0.0" {
                    vec![CompanionFile {
                        relative_path: "uv-extra".into(),
                        bytes: b"#!/bin/sh\nprintf '%s\\n' 'extra:2.0.0'\n".to_vec(),
                        executable: true,
                    }]
                } else {
                    Vec::new()
                },
            })
            .await
            .unwrap();
    }
    for (name, requests) in [("one", ["1.0.0", "2.0.0"]), ("two", ["2.0.0", "1.0.0"])] {
        let path = project_env_path(temporary.path().join(name));
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        write_env_declarations(
            path,
            &EnvDeclarations {
                tools: std::collections::BTreeMap::from([(
                    "uv".into(),
                    requests.map(String::from).to_vec(),
                )]),
                vars: Default::default(),
            },
        )
        .unwrap();
    }
    let empty = temporary.path().join("empty");
    std::fs::create_dir(&empty).unwrap();
    let output = run(&empty, &home, &["tool", "reshim"]).await;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    for (name, project, command, args) in [
        (
            "project-forward",
            "one",
            "uv",
            vec!["space $value", "quote'\"", "中文"],
        ),
        ("project-reverse", "two", "uv", vec![]),
        ("secondary-command", "one", "uv-extra", vec![]),
        ("unconfigured", "empty", "uv", vec![]),
        ("repair-missing-shim", "two", "uv", vec![]),
    ] {
        if name == "repair-missing-shim" {
            std::fs::remove_file(store.shim_dir().join("uv")).unwrap();
            assert!(
                run(&empty, &home, &["tool", "reshim"])
                    .await
                    .status
                    .success()
            );
        }
        let expected = reference
            .cases
            .iter()
            .find(|case| case.name == name)
            .unwrap();
        let output = tokio::time::timeout(
            Duration::from_secs(20),
            tokio::process::Command::new(store.shim_dir().join(command))
                .args(args)
                .env_clear()
                .env("PATH", "/usr/bin:/bin")
                .env("ASTER_HOME", temporary.path().join("different-home"))
                .current_dir(temporary.path().join(project))
                .kill_on_drop(true)
                .output(),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(
            output.status.code(),
            Some(expected.exit),
            "{name}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8(output.stdout).unwrap(),
            expected.stdout,
            "{name}"
        );
    }
}

#[cfg(unix)]
#[tokio::test]
#[ignore = "Requires downloading an explicitly selected real uv release from its canonical backend"]
async fn project_shim_installs_its_missing_declared_version_from_the_real_backend() {
    let temporary = tempfile::tempdir().unwrap();
    let home = temporary.path().join("home");
    let project = temporary.path().join("project");
    let store = ToolStore::open(home.join("tools")).unwrap();
    install_uv_fixture(&store, temporary.path(), "1.0.0").await;
    let version = std::env::var("ASTER_TEST_SHIM_AUTO_VERSION").expect("explicit real uv version");
    write_env_acl(
        project_env_path(&project),
        &EnvConfig {
            tools: std::collections::BTreeMap::from([("uv".into(), version.clone())]),
            vars: Default::default(),
        },
    )
    .unwrap();
    assert!(
        store
            .installed_receipt("uv", Some(&version))
            .unwrap()
            .is_none()
    );
    assert!(
        run(&project, &home, &["tool", "reshim"])
            .await
            .status
            .success()
    );
    let output = tokio::time::timeout(
        Duration::from_secs(120),
        tokio::process::Command::new(store.shim_dir().join("uv"))
            .arg("--version")
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .current_dir(&project)
            .kill_on_drop(true)
            .output(),
    )
    .await
    .unwrap()
    .unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let receipt = store.installed_receipt("uv", Some(&version));
    if let Some(path) = std::env::var_os("ASTER_TEST_SHIM_AUTO_RESULT") {
        std::fs::write(
            path,
            serde_json::to_vec_pretty(&serde_json::json!({
                "version": version, "exit": output.status.code(), "stdout": stdout,
                "stderr": String::from_utf8_lossy(&output.stderr),
                "receipt": receipt.as_ref().ok().and_then(|value| value.as_ref()),
                "receiptError": receipt.as_ref().err().map(ToString::to_string)
            }))
            .unwrap(),
        )
        .unwrap();
    }
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        stdout.split_whitespace().take(2).collect::<Vec<_>>(),
        ["uv", &version]
    );
    assert_eq!(receipt.unwrap().unwrap().version, version);
}

#[cfg(unix)]
#[tokio::test]
async fn multi_version_declarations_and_execution_match_five_frozen_native_mise_cases() {
    #[derive(serde::Deserialize)]
    struct Reference {
        cases: Vec<Case>,
    }
    #[derive(serde::Deserialize)]
    struct Case {
        name: String,
        arguments: Vec<String>,
        configured: Option<Vec<String>>,
        pins: std::collections::BTreeMap<String, serde_json::Value>,
        installed: Vec<String>,
        execution: Execution,
    }
    #[derive(serde::Deserialize)]
    struct Execution {
        command: String,
        stdout: String,
        selected_relative_path: String,
    }
    let reference: Reference = serde_json::from_str(include_str!(
        "../../../specs/031-cli-mise-replacement/contracts/mise-multiversion-reference.json"
    ))
    .unwrap();
    assert_eq!(reference.cases.len(), 5);
    for case in reference.cases {
        let temporary = tempfile::tempdir().unwrap();
        let project = temporary.path().join("多版本 project");
        let home = temporary.path().join("aster-home");
        let store = ToolStore::open(home.join("tools")).unwrap();
        for version in &case.installed {
            install_multiversion_uv_fixture(&store, temporary.path(), version).await;
        }
        let path = project_env_path(&project);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let tools = case
            .configured
            .as_ref()
            .map(|versions| {
                format!(
                    "tools {{ tool \"uv\" {{ version = {} }} }}",
                    serde_json::to_string(versions).unwrap()
                )
            })
            .unwrap_or_default();
        std::fs::write(
            &path,
            format!("env {{ {tools} vars {{ var \"KEPT\" {{ value = \"literal value\" }} }} }}\n"),
        )
        .unwrap();
        let before = std::fs::read(&path).unwrap();
        let mut arguments = vec!["tool".to_owned()];
        arguments.extend(
            case.arguments
                .iter()
                .filter(|arg| *arg != "--pin")
                .map(|arg| {
                    if arg == "fixture" {
                        "uv".to_owned()
                    } else if let Some(version) = arg.strip_prefix("fixture@") {
                        format!("uv@{version}")
                    } else {
                        arg.clone()
                    }
                }),
        );
        if case.arguments[0] == "install" {
            arguments.push("--offline".into());
        }
        arguments.push("--json".into());
        let output = run(&project, &home, &arguments).await;
        assert!(
            output.status.success(),
            "{}: {}",
            case.name,
            String::from_utf8_lossy(&output.stderr)
        );
        if case.arguments[0] == "install" {
            assert_eq!(std::fs::read(&path).unwrap(), before);
        }
        let document = aster_acl::parse_acl(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let env = document.blocks.first().unwrap();
        let tool = env
            .blocks
            .iter()
            .find(|block| block.name == "tools")
            .unwrap()
            .blocks
            .iter()
            .find(|tool| tool.labels == ["uv"])
            .unwrap();
        let pin = match tool.attributes.get("version").unwrap() {
            aster_acl::Value::String(value) => serde_json::json!(value),
            aster_acl::Value::List(values) => serde_json::json!(
                values
                    .iter()
                    .map(|v| v.as_str().unwrap())
                    .collect::<Vec<_>>()
            ),
            other => panic!("invalid version declaration: {other:?}"),
        };
        assert_eq!(pin, case.pins["fixture"], "{}", case.name);
        let command = match case.execution.command.as_str() {
            "fixture" => "uv",
            "fixture-extra" => "uv-extra",
            other => panic!("unadmitted reference executable: {other}"),
        };
        let output = run(
            &project,
            &home,
            &["exec", "--offline", "--no-install", "--", command],
        )
        .await;
        assert!(
            output.status.success(),
            "{}: {}",
            case.name,
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            output.stdout,
            case.execution.stdout.as_bytes(),
            "{}",
            case.name
        );
        let which = run(&project, &home, &["tool", "which", command]).await;
        assert!(
            which.status.success(),
            "{}: {}",
            case.name,
            String::from_utf8_lossy(&which.stderr)
        );
        let relative = Path::new(&case.execution.selected_relative_path)
            .components()
            .map(|part| part.as_os_str())
            .collect::<Vec<_>>();
        let [version, bin, name] = relative.as_slice() else {
            panic!("unadmitted reference layout")
        };
        assert_eq!(*bin, OsStr::new("bin"));
        assert_eq!(*name, OsStr::new(&case.execution.command));
        let expected = store
            .root_path()
            .join("installs/uv")
            .join(*version)
            .join(command);
        let actual = String::from_utf8(which.stdout).unwrap();
        assert_eq!(
            Path::new(actual.trim()).canonicalize().unwrap(),
            expected.canonicalize().unwrap()
        );
        let shown = run(&project, &home, &["env", "show", "--json"]).await;
        assert!(shown.status.success());
        let shown: serde_json::Value = serde_json::from_slice(&shown.stdout).unwrap();
        assert_eq!(shown["tools"]["uv"], pin);
        assert_eq!(shown["vars"]["KEPT"], "literal value");
        if case.name == "use_forward" {
            for arguments in [
                vec!["env", "set", "CHANGED=value"],
                vec!["env", "unset", "CHANGED"],
            ] {
                assert!(run(&project, &home, &arguments).await.status.success());
            }
            let output = run(&project, &home, &["tool", "lock", "--offline", "--json"]).await;
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            let output = run(
                &project,
                &home,
                &["exec", "--locked", "--offline", "--", "uv"],
            )
            .await;
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(output.stdout, case.execution.stdout.as_bytes());
            let output = run(
                &project,
                &home,
                &["tool", "uninstall", "uv@1.0.0", "--unpin", "--json"],
            )
            .await;
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            let remaining = aster_use_tool_acquire::read_env_acl(&path)
                .unwrap()
                .unwrap();
            assert_eq!(remaining.tools.get("uv").map(String::as_str), Some("2.0.0"));
            assert_eq!(
                remaining.vars.get("KEPT").map(String::as_str),
                Some("literal value")
            );
            assert!(!remaining.vars.contains_key("CHANGED"));
        }
    }
}

#[cfg(unix)]
#[tokio::test]
async fn batch_install_and_use_selection_match_seven_frozen_native_mise_cases() {
    #[derive(serde::Deserialize)]
    struct Reference {
        initial_versions: Vec<String>,
        cases: Vec<Case>,
    }
    #[derive(serde::Deserialize)]
    struct Case {
        name: String,
        arguments: Vec<String>,
        exit: i32,
        config_unchanged: bool,
        pins: std::collections::BTreeMap<String, String>,
        installed: std::collections::BTreeMap<String, Vec<String>>,
        executions: std::collections::BTreeMap<String, std::collections::BTreeMap<String, String>>,
    }
    let reference: Reference = serde_json::from_str(include_str!(
        "../../../specs/031-cli-mise-replacement/contracts/mise-batch-install-reference.json"
    ))
    .unwrap();
    assert_eq!(reference.cases.len(), 7);
    let tools = [
        ("fixture", "uv", "github:astral-sh/uv"),
        ("other", "fd", "github:sharkdp/fd"),
    ];
    for case in reference.cases {
        let temporary = tempfile::tempdir().unwrap();
        let project = temporary.path().join("批量 project");
        let home = temporary.path().join("aster-home");
        let store = ToolStore::open(home.join("tools")).unwrap();
        let mut config = EnvConfig::default();
        for (upstream, name, spec) in tools {
            // Downloads are tested by the acquisition owner; these CLI cases use verified local payloads.
            for version in &case.installed[upstream] {
                install_tool_fixture(&store, temporary.path(), name, spec, version).await;
            }
            config.tools.insert(name.into(), "1.0.0".into());
        }
        config.vars.insert("KEPT".into(), "literal value".into());
        let path = project_env_path(&project);
        write_env_acl(&path, &config).unwrap();
        let before = std::fs::read(&path).unwrap();
        let mut arguments = vec!["tool".to_owned()];
        for argument in &case.arguments {
            if argument == "--pin" {
                continue;
            }
            let mapped = tools
                .iter()
                .find_map(|(upstream, name, _)| {
                    if argument == upstream {
                        Some((*name).to_owned())
                    } else {
                        argument
                            .strip_prefix(&format!("{upstream}@"))
                            .map(|v| format!("{name}@{v}"))
                    }
                })
                .unwrap_or_else(|| argument.replace("broken@", "hermes@"));
            arguments.push(mapped);
        }
        if case.arguments[0] == "install" {
            arguments.push("--offline".into());
        }
        arguments.push("--json".into());
        let result = run(&project, &home, &arguments).await;
        assert_eq!(
            result.status.code(),
            Some(case.exit),
            "{}: {}",
            case.name,
            String::from_utf8_lossy(&result.stderr)
        );
        let pinned = aster_use_tool_acquire::read_env_acl(&path)
            .unwrap()
            .unwrap();
        for (upstream, name, _) in tools {
            assert_eq!(
                pinned.tools[name], case.pins[upstream],
                "{}: {name}",
                case.name
            );
            assert_eq!(
                store
                    .installed_versions(name)
                    .unwrap()
                    .into_iter()
                    .map(|r| r.version)
                    .collect::<Vec<_>>(),
                case.installed[upstream],
                "{}: {name}",
                case.name
            );
            for (version, expected) in &case.executions[upstream] {
                let executable = store
                    .root_path()
                    .join("installs")
                    .join(name)
                    .join(version)
                    .join(name);
                let executed = tokio::time::timeout(
                    Duration::from_secs(20),
                    tokio::process::Command::new(executable).output(),
                )
                .await
                .unwrap()
                .unwrap();
                assert!(executed.status.success());
                assert_eq!(
                    executed.stdout,
                    expected.as_bytes(),
                    "{}: {name}@{version}",
                    case.name
                );
            }
        }
        assert_eq!(pinned.vars, config.vars);
        if case.config_unchanged {
            assert_eq!(std::fs::read(&path).unwrap(), before, "{}", case.name);
        }
        if !result.status.success() {
            continue;
        }
        #[derive(serde::Deserialize)]
        struct Record {
            name: String,
            version: String,
        }
        let records: Vec<Record> = serde_json::from_slice(&result.stdout).unwrap();
        let expected: Vec<_> = tools
            .iter()
            .flat_map(|(upstream, name, _)| {
                if case.arguments[0] == "install" {
                    case.installed[*upstream]
                        .iter()
                        .filter(|v| !reference.initial_versions.contains(v))
                        .map(|v| ((*name).to_owned(), v.clone()))
                        .collect::<Vec<_>>()
                } else {
                    vec![((*name).to_owned(), case.pins[*upstream].clone())]
                }
            })
            .collect();
        assert_eq!(
            records
                .into_iter()
                .map(|r| (r.name, r.version))
                .collect::<Vec<_>>(),
            expected,
            "{}",
            case.name
        );
    }
}

#[cfg(unix)]
#[tokio::test]
async fn batch_use_combines_managed_path_and_system_tools_and_preserves_other_values() {
    let temporary = tempfile::tempdir().unwrap();
    let project = temporary.path().join("项目 with spaces");
    let home = temporary.path().join("aster-home");
    std::fs::create_dir_all(&project).unwrap();
    let external = project.join("外部 SDK");
    write_context_node(&external.join("bin/node"), "external-node");
    let store = ToolStore::open(home.join("tools")).unwrap();
    install_uv_fixture(&store, temporary.path(), "2.0.0").await;
    install_uv_fixture(&store, temporary.path(), "1.0.0").await;
    let mut source = StaticSource::new();
    source.insert_versions(
        "github:astral-sh/uv",
        VersionListing::new(
            vec![
                ("1.0.0".into(), false),
                ("2.0.0".into(), false),
                ("3.0.0".into(), false),
            ],
            std::collections::BTreeMap::from([("latest".into(), "3.0.0".into())]),
        )
        .unwrap(),
    );
    store
        .remote_versions_with(
            &parse_tool_spec("github:astral-sh/uv").unwrap(),
            Some(&source),
        )
        .unwrap();
    let path = project_env_path(&project);
    let config = EnvConfig {
        tools: std::collections::BTreeMap::from([("gh".into(), "system".into())]),
        vars: std::collections::BTreeMap::from([("KEPT".into(), "literal value".into())]),
    };
    write_env_acl(&path, &config).unwrap();
    let result = run(
        &project,
        &home,
        &[
            "tool",
            "use",
            "uv",
            "node@path:./外部 SDK",
            "fd@system",
            "uv",
            "--json",
        ],
    )
    .await;
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let pinned = aster_use_tool_acquire::read_env_acl(&path)
        .unwrap()
        .unwrap();
    assert_eq!(
        pinned.tools,
        std::collections::BTreeMap::from([
            ("uv".into(), "2.0.0".into()),
            (
                "node".into(),
                format!(
                    "path:{}",
                    external.canonicalize().unwrap().to_str().unwrap()
                )
            ),
            ("fd".into(), "system".into()),
            ("gh".into(), "system".into()),
        ])
    );
    assert_eq!(pinned.vars, config.vars);
    let records: Vec<serde_json::Value> = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(
        records
            .iter()
            .map(|r| r["name"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["uv", "node", "fd"]
    );
    assert!(store.installed_versions("node").unwrap().is_empty());
    assert!(store.installed_versions("fd").unwrap().is_empty());
    assert_eq!(
        std::fs::read_to_string(store.root_path().join("current/uv")).unwrap(),
        "1.0.0\n"
    );
    let executed = run(
        &project,
        &home,
        &["exec", "--no-install", "--offline", "--", "node"],
    )
    .await;
    assert!(
        executed.status.success(),
        "{}",
        String::from_utf8_lossy(&executed.stderr)
    );
    assert_eq!(executed.stdout, b"external-node\n");
}

#[cfg(unix)]
#[tokio::test]
async fn batch_tool_requests_validate_all_arguments_before_mutating_project_configuration() {
    let temporary = tempfile::tempdir().unwrap();
    let project = temporary.path().join("project");
    let home = temporary.path().join("aster-home");
    let path = project_env_path(&project);
    write_env_acl(&path, &EnvConfig::default()).unwrap();
    let before = std::fs::read(&path).unwrap();
    for (arguments, rejected) in [
        (
            vec!["tool", "install", "uv@1.0.0", "unknown-tool", "--offline"],
            "unknown-tool",
        ),
        (
            vec!["tool", "use", "hermes@1.0.0", "unknown-tool"],
            "unknown-tool",
        ),
        (
            vec!["tool", "use", "hermes@1.0.0", "hermes@2.0.0"],
            "tool runtime is missing: uv",
        ),
    ] {
        let output = run(&project, &home, &arguments).await;
        assert_eq!(
            output.status.code(),
            Some(1),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(rejected),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(std::fs::read(&path).unwrap(), before);
        assert!(!home.join("tools/receipts").exists());
        assert!(!home.join("tools/current").exists());
    }
}

#[cfg(unix)]
#[tokio::test]
async fn multi_tool_uninstall_matches_the_five_frozen_native_mise_cases() {
    #[derive(serde::Deserialize)]
    struct Reference {
        installations: std::collections::BTreeMap<String, Vec<String>>,
        cases: Vec<Case>,
    }
    #[derive(serde::Deserialize)]
    struct Case {
        name: String,
        arguments: Vec<String>,
        exit: i32,
        retained: std::collections::BTreeMap<String, Vec<String>>,
    }
    let reference: Reference = serde_json::from_str(include_str!(
        "../../../specs/031-cli-mise-replacement/contracts/mise-multi-uninstall-reference.json"
    ))
    .unwrap();
    assert_eq!(reference.cases.len(), 5);
    let tools = [
        ("fixture", "uv", "github:astral-sh/uv"),
        ("other", "fd", "github:sharkdp/fd"),
    ];
    for case in reference.cases {
        let temporary = tempfile::tempdir().unwrap();
        let project = temporary.path().join("项目 with spaces");
        let home = temporary.path().join("home");
        std::fs::create_dir_all(&project).unwrap();
        let store = ToolStore::open(home.join("tools")).unwrap();
        let mut config = EnvConfig::default();
        for (upstream, name, spec) in tools {
            for version in &reference.installations[upstream] {
                install_tool_fixture(&store, temporary.path(), name, spec, version).await;
            }
            config.tools.insert(name.into(), "1.0.0".into());
        }
        config.vars.insert("KEPT".into(), "literal value".into());
        let path = project_env_path(&project);
        write_env_acl(&path, &config).unwrap();
        let before = std::fs::read(&path).unwrap();
        let mut args = vec!["tool".to_owned(), "uninstall".to_owned()];
        for arg in &case.arguments {
            let mapped = tools
                .iter()
                .find_map(|(upstream, name, _)| {
                    if arg.as_str() == *upstream {
                        Some((*name).to_owned())
                    } else {
                        arg.strip_prefix(&format!("{upstream}@"))
                            .map(|version| format!("{name}@{version}"))
                    }
                })
                .unwrap_or_else(|| arg.clone());
            args.push(mapped);
        }
        args.push("--json".into());
        let output = run(&project, &home, &args).await;
        assert_eq!(
            output.status.code(),
            Some(case.exit),
            "{}: {}",
            case.name,
            String::from_utf8_lossy(&output.stderr)
        );
        for (upstream, name, _) in tools {
            let actual = store
                .installed_versions(name)
                .unwrap()
                .into_iter()
                .map(|receipt| receipt.version)
                .collect::<Vec<_>>();
            assert_eq!(actual, case.retained[upstream], "{}: {name}", case.name);
            for version in &actual {
                let executed = tokio::time::timeout(
                    Duration::from_secs(20),
                    tokio::process::Command::new(
                        store
                            .root_path()
                            .join("installs")
                            .join(name)
                            .join(version)
                            .join(name),
                    )
                    .output(),
                )
                .await
                .unwrap()
                .unwrap();
                assert!(executed.status.success());
                assert_eq!(executed.stdout, format!("{version}\n").as_bytes());
            }
        }
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }
}

#[cfg(unix)]
#[tokio::test]
async fn global_uninstall_preview_and_unpin_include_broken_and_noncurrent_installs() {
    let temporary = tempfile::tempdir().unwrap();
    let project = temporary.path().join("project");
    let home = temporary.path().join("home");
    std::fs::create_dir_all(&project).unwrap();
    let store = ToolStore::open(home.join("tools")).unwrap();
    install_uv_fixture(&store, temporary.path(), "1.0.0").await;
    install_uv_fixture(&store, temporary.path(), "2.0.0").await;
    store.remove_version_sync("uv", "2.0.0").unwrap();
    install_tool_fixture(
        &store,
        temporary.path(),
        "fd",
        "github:sharkdp/fd",
        "10.0.0",
    )
    .await;
    std::fs::remove_file(store.root_path().join("installs/fd/10.0.0/fd")).unwrap();
    let path = project_env_path(&project);
    write_env_acl(
        &path,
        &EnvConfig {
            tools: [
                ("uv".into(), "1.0.0".into()),
                ("fd".into(), "10.0.0".into()),
                ("bun".into(), "latest".into()),
            ]
            .into(),
            vars: [("KEPT".into(), "literal value".into())].into(),
        },
    )
    .unwrap();
    let before = std::fs::read(&path).unwrap();
    let missing_selection = run(&project, &home, &["tool", "uninstall"]).await;
    assert_eq!(missing_selection.status.code(), Some(2));
    assert_eq!(store.all_uninstall_candidates().unwrap().len(), 2);
    let preview = run(
        &project,
        &home,
        &[
            "tool",
            "uninstall",
            "--all",
            "--dry-run-code",
            "--unpin",
            "--json",
        ],
    )
    .await;
    assert_eq!(
        preview.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&preview.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&preview.stdout).unwrap();
    assert_eq!(
        json["tools"],
        serde_json::json!([{"name":"fd","version":"10.0.0"},{"name":"uv","version":"1.0.0"}])
    );
    assert_eq!(store.all_uninstall_candidates().unwrap().len(), 2);
    assert_eq!(std::fs::read(&path).unwrap(), before);
    for args in [vec!["fd", "uv@path:outside"], vec!["fd", "../invalid"]] {
        let mut command = vec!["tool", "uninstall"];
        command.extend(args);
        let rejected = run(&project, &home, &command).await;
        assert!(!rejected.status.success());
        assert_eq!(store.all_uninstall_candidates().unwrap().len(), 2);
    }
    let removed = run(
        &project,
        &home,
        &["tool", "uninstall", "--all", "--unpin", "--json"],
    )
    .await;
    assert!(
        removed.status.success(),
        "{}",
        String::from_utf8_lossy(&removed.stderr)
    );
    assert!(store.all_uninstall_candidates().unwrap().is_empty());
    let config = aster_use_tool_acquire::read_env_acl(&path)
        .unwrap()
        .unwrap();
    assert_eq!(config.tools, [("bun".into(), "latest".into())].into());
    assert_eq!(config.vars.get("KEPT").unwrap(), "literal value");
    let empty = run(
        &project,
        &home,
        &["tool", "uninstall", "--all", "--dry-run-code", "--json"],
    )
    .await;
    assert!(empty.status.success());
}

#[cfg(unix)]
#[tokio::test]
async fn exact_uninstall_keeps_other_versions_and_an_unrelated_project_pin() {
    let temporary = tempfile::tempdir().unwrap();
    let project = temporary.path().join("项目 with spaces");
    let home = temporary.path().join("home");
    std::fs::create_dir_all(&project).unwrap();
    let store = ToolStore::open(home.join("tools")).unwrap();
    for version in ["1.0.0", "2.0.0"] {
        install_uv_fixture(&store, temporary.path(), version).await;
    }
    let path = project_env_path(&project);
    write_env_acl(
        &path,
        &EnvConfig {
            tools: [("uv".into(), "1.0.0".into())].into(),
            vars: [("KEPT".into(), "literal value".into())].into(),
        },
    )
    .unwrap();
    let before = std::fs::read(&path).unwrap();
    let output = run(
        &project,
        &home,
        &["tool", "uninstall", "uv@2.0.0", "--unpin", "--json"],
    )
    .await;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["removed"], "uv@2.0.0");
    assert!(
        store
            .installed_receipt("uv", Some("2.0.0"))
            .unwrap()
            .is_none()
    );
    assert!(
        store
            .installed_receipt("uv", Some("1.0.0"))
            .unwrap()
            .is_some()
    );
    assert_eq!(std::fs::read(&path).unwrap(), before);
    let executed = run(
        &project,
        &home,
        &["exec", "--offline", "--no-install", "--", "uv"],
    )
    .await;
    assert!(
        executed.status.success(),
        "{}",
        String::from_utf8_lossy(&executed.stderr)
    );
    assert_eq!(executed.stdout, b"1.0.0\n");
    let removed = run(
        &project,
        &home,
        &["tool", "uninstall", "uv@1.0.0", "--unpin"],
    )
    .await;
    assert!(
        removed.status.success(),
        "{}",
        String::from_utf8_lossy(&removed.stderr)
    );
    let config = aster_use_tool_acquire::read_env_acl(&path)
        .unwrap()
        .unwrap();
    assert!(config.tools.is_empty());
    assert_eq!(config.vars.get("KEPT").unwrap(), "literal value");
}

#[cfg(unix)]
#[tokio::test]
async fn uninstall_matches_the_eight_frozen_native_mise_cases() {
    #[derive(serde::Deserialize)]
    struct Reference {
        versions: Vec<String>,
        cases: Vec<Case>,
    }
    #[derive(serde::Deserialize)]
    struct Case {
        name: String,
        request: Option<String>,
        flags: Vec<String>,
        exit: i32,
        retained_versions: Vec<String>,
    }
    let reference: Reference = serde_json::from_str(include_str!(
        "../../../specs/031-cli-mise-replacement/contracts/mise-uninstall-reference.json"
    ))
    .unwrap();
    assert_eq!(reference.cases.len(), 8);
    for case in reference.cases {
        let temporary = tempfile::tempdir().unwrap();
        let project = temporary.path().join("项目 with spaces");
        let home = temporary.path().join("home");
        std::fs::create_dir_all(&project).unwrap();
        let store = ToolStore::open(home.join("tools")).unwrap();
        for version in &reference.versions {
            install_uv_fixture(&store, temporary.path(), version).await;
        }
        let spec = parse_tool_spec("github:astral-sh/uv").unwrap();
        let mut source = StaticSource::new();
        source.insert_versions(
            spec.raw(),
            VersionListing::new(
                reference
                    .versions
                    .iter()
                    .map(|version| (version.clone(), false))
                    .collect(),
                Default::default(),
            )
            .unwrap(),
        );
        store.remote_versions_with(&spec, Some(&source)).unwrap();
        let mut args = vec!["tool".to_owned(), "uninstall".to_owned()];
        args.extend(case.flags);
        args.push(
            case.request
                .map_or_else(|| "uv".into(), |request| format!("uv@{request}")),
        );
        let output = run(&project, &home, &args).await;
        assert_eq!(
            output.status.code(),
            Some(case.exit),
            "{}: {}",
            case.name,
            String::from_utf8_lossy(&output.stderr)
        );
        let retained = store.installed_versions("uv").unwrap();
        assert_eq!(
            retained
                .iter()
                .map(|receipt| receipt.version.as_str())
                .collect::<Vec<_>>(),
            case.retained_versions,
            "{}",
            case.name
        );
        assert!(!project_env_path(&project).exists(), "{}", case.name);
        for receipt in retained {
            let paths = store.installation_paths(&receipt).unwrap();
            let mut command = tokio::process::Command::new(paths.executable());
            command.env_clear().current_dir(&project).kill_on_drop(true);
            let executed = tokio::time::timeout(Duration::from_secs(20), command.output())
                .await
                .unwrap()
                .unwrap();
            assert!(executed.status.success(), "{}", case.name);
            assert_eq!(
                executed.stdout,
                format!("{}\n", receipt.version).as_bytes(),
                "{}",
                case.name
            );
        }
    }
}

#[cfg(unix)]
#[tokio::test]
async fn uninstall_rejects_ambiguity_and_previews_leave_payloads_and_pins_unchanged() {
    let temporary = tempfile::tempdir().unwrap();
    let project = temporary.path().join("project");
    let home = temporary.path().join("home");
    std::fs::create_dir_all(&project).unwrap();
    let store = ToolStore::open(home.join("tools")).unwrap();
    let versions = ["1.2", "1.2.3", "1.2.9", "1.20.0"];
    for version in versions {
        install_uv_fixture(&store, temporary.path(), version).await;
    }
    let spec = parse_tool_spec("github:astral-sh/uv").unwrap();
    let mut source = StaticSource::new();
    source.insert_versions(
        spec.raw(),
        VersionListing::new(
            versions
                .into_iter()
                .map(|version| (version.into(), false))
                .collect(),
            Default::default(),
        )
        .unwrap(),
    );
    store.remote_versions_with(&spec, Some(&source)).unwrap();
    let path = project_env_path(&project);
    write_env_acl(
        &path,
        &EnvConfig {
            tools: [("uv".into(), "1.2".into())].into(),
            vars: Default::default(),
        },
    )
    .unwrap();
    let before = std::fs::read(&path).unwrap();
    for query in ["uv", "uv@1"] {
        let output = run(&project, &home, &["tool", "uninstall", query]).await;
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("multiple installed versions match")
        );
    }
    let preview = run(
        &project,
        &home,
        &[
            "tool",
            "uninstall",
            "uv@prefix:1.2",
            "--all",
            "--dry-run",
            "--unpin",
            "--json",
        ],
    )
    .await;
    assert!(
        preview.status.success(),
        "{}",
        String::from_utf8_lossy(&preview.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&preview.stdout).unwrap();
    assert_eq!(json["wouldRemove"], serde_json::json!(["1.2.9"]));
    let preview_code = run(
        &project,
        &home,
        &[
            "tool",
            "uninstall",
            "uv@1.2.3",
            "--dry-run",
            "--dry-run-code",
            "--json",
        ],
    )
    .await;
    assert_eq!(preview_code.status.code(), Some(1));
    assert!(preview_code.stderr.is_empty());
    let json: serde_json::Value = serde_json::from_slice(&preview_code.stdout).unwrap();
    assert_eq!(json["wouldRemove"], serde_json::json!(["1.2.3"]));
    let no_match = run(
        &project,
        &home,
        &["tool", "uninstall", "uv@99", "--dry-run-code", "--json"],
    )
    .await;
    assert!(no_match.status.success());
    assert_eq!(
        store.installed_versions("uv").unwrap().len(),
        versions.len()
    );
    assert_eq!(std::fs::read(&path).unwrap(), before);
    for (query, flags) in [("uv@1.2", vec![]), ("uv@1.2", vec!["--all"])] {
        let mut args = vec!["tool", "uninstall", query];
        args.extend(flags);
        let removed = run(&project, &home, &args).await;
        assert!(
            removed.status.success(),
            "{}",
            String::from_utf8_lossy(&removed.stderr)
        );
    }
    assert_eq!(
        store
            .installed_versions("uv")
            .unwrap()
            .iter()
            .map(|receipt| receipt.version.as_str())
            .collect::<Vec<_>>(),
        ["1.20.0"]
    );
    let executed = run(
        &project,
        &home,
        &[
            "exec",
            "-t",
            "uv@1.20.0",
            "--offline",
            "--no-install",
            "--",
            "uv",
        ],
    )
    .await;
    assert!(
        executed.status.success(),
        "{}",
        String::from_utf8_lossy(&executed.stderr)
    );
    assert_eq!(executed.stdout, b"1.20.0\n");
    assert_eq!(std::fs::read(&path).unwrap(), before);
    let removed = run(&project, &home, &["tool", "uninstall", "uv", "--unpin"]).await;
    assert!(
        removed.status.success(),
        "{}",
        String::from_utf8_lossy(&removed.stderr)
    );
    assert!(
        aster_use_tool_acquire::read_env_acl(&path)
            .unwrap()
            .unwrap()
            .tools
            .is_empty()
    );
}

#[cfg(unix)]
#[tokio::test]
async fn uninstall_unpin_refuses_invalid_declarations_before_removing_payloads() {
    let temporary = tempfile::tempdir().unwrap();
    let project = temporary.path().join("project");
    let home = temporary.path().join("home");
    std::fs::create_dir_all(project.join(".aster")).unwrap();
    let store = ToolStore::open(home.join("tools")).unwrap();
    install_uv_fixture(&store, temporary.path(), "1.0.0").await;
    let receipt = store.installed_receipt("uv", None).unwrap().unwrap();
    let path = project_env_path(&project);
    let invalid = b"env { unknown {} }\n";
    std::fs::write(&path, invalid).unwrap();
    let output = run(&project, &home, &["tool", "uninstall", "uv", "--unpin"]).await;
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("unknown env child block: unknown"));
    assert_eq!(store.installed_receipt("uv", None).unwrap(), Some(receipt));
    assert_eq!(std::fs::read(&path).unwrap(), invalid);
    assert!(
        store
            .install_dir("uv")
            .unwrap()
            .unwrap()
            .join("uv")
            .is_file()
    );
}

#[cfg(unix)]
#[tokio::test]
async fn cli_exec_and_which_use_native_archive_layouts_and_uninstall_removes_all_entrypoints() {
    use aster_use_tool_acquire::{
        AcquireRequest, ArchiveFormat, ArchivePayload, InstallPayload, ToolAcquireError,
        ToolPayload, ToolQuery, ToolSource, ToolSpec,
    };
    use sha2::{Digest, Sha256};
    use std::collections::BTreeMap;

    struct Source(ArchivePayload);
    impl ToolSource for Source {
        fn fetch(&self, _: &ToolQuery) -> Result<ToolPayload, ToolAcquireError> {
            Err(ToolAcquireError::AssetMissing)
        }
        fn latest(&self, _: &ToolSpec) -> Result<String, ToolAcquireError> {
            Ok(self.0.version().into())
        }
        fn fetch_installation(&self, _: &ToolQuery) -> Result<InstallPayload, ToolAcquireError> {
            Ok(InstallPayload::Archive(self.0.clone()))
        }
    }
    let encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    let mut archive = tar::Builder::new(encoder);
    let script = b"#!/bin/sh\ncat \"$(dirname \"$0\")/../share/value\"\n";
    for (name, bytes, mode) in [
        ("runtime/bin/uv", script.as_slice(), 0o755),
        ("runtime/lib/entry.sh", script.as_slice(), 0o755),
        (
            "runtime/share/value",
            b"native archive executed\n".as_slice(),
            0o644,
        ),
    ] {
        let mut header = tar::Header::new_gnu();
        header.set_mode(mode);
        header.set_size(bytes.len() as u64);
        header.set_cksum();
        archive.append_data(&mut header, name, bytes).unwrap();
    }
    let mut link = tar::Header::new_gnu();
    link.set_entry_type(tar::EntryType::Symlink);
    link.set_link_name("../lib/entry.sh").unwrap();
    link.set_mode(0o755);
    link.set_size(0);
    link.set_cksum();
    archive
        .append_data(&mut link, "runtime/bin/alternate", std::io::empty())
        .unwrap();
    let bytes = archive.into_inner().unwrap().finish().unwrap();
    let hash = format!("{:x}", Sha256::digest(&bytes));
    let source = Source(
        ArchivePayload::new(
            "1.0.0".into(),
            &hash,
            bytes,
            ArchiveFormat::TarGz,
            "runtime".into(),
            BTreeMap::from([
                ("uv".into(), "bin/uv".into()),
                ("alternate".into(), "bin/alternate".into()),
            ]),
            vec!["bin".into()],
        )
        .unwrap(),
    );
    let temporary = tempfile::tempdir().unwrap();
    let project = temporary.path().join("项目 archive workspace");
    std::fs::create_dir(&project).unwrap();
    let home = temporary.path().join("home with spaces");
    let store = ToolStore::open(home.join("tools")).unwrap();
    store
        .acquire_with(
            AcquireRequest {
                name: "uv".into(),
                spec: "github:astral-sh/uv".into(),
                version: Some("1.0.0".into()),
                executable_name: "uv".into(),
            },
            &source,
        )
        .unwrap();
    write_env_acl(
        project_env_path(&project),
        &EnvConfig {
            tools: BTreeMap::from([("uv".into(), "1.0.0".into())]),
            vars: BTreeMap::new(),
        },
    )
    .unwrap();

    for command in ["uv", "alternate"] {
        let output = run(&project, &home, &["exec", "--offline", "--", command]).await;
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout, b"native archive executed\n");
    }
    let output = run(&project, &home, &["tool", "which", "uv"]).await;
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap().trim(),
        store
            .root_path()
            .join("installs/uv/1.0.0/bin/uv")
            .to_str()
            .unwrap()
    );
    assert!(!store.root_path().join("installs/uv/1.0.0/uv").exists());
    let output = run(&project, &home, &["tool", "lock", "--offline"]).await;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(project_env_lock_path(&project).exists());
    let output = run(
        &project,
        &home,
        &["exec", "--locked", "--offline", "--", "alternate"],
    )
    .await;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"native archive executed\n");
    std::fs::write(
        store.root_path().join("installs/uv/1.0.0/share/value"),
        b"changed",
    )
    .unwrap();
    let output = run(
        &project,
        &home,
        &["exec", "--locked", "--offline", "--", "uv"],
    )
    .await;
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("do not match env lock"));
    let output = run(&project, &home, &["tool", "uninstall", "uv", "--unpin"]).await;
    assert!(output.status.success());
    assert!(!store.shim_dir().join("alternate").exists());
}

#[cfg(unix)]
#[tokio::test]
async fn cli_version_requests_match_frozen_mise_executions_and_lock_without_metadata() {
    #[derive(serde::Deserialize)]
    struct Reference {
        versions: Vec<Row>,
        cases: Vec<Case>,
    }
    #[derive(serde::Deserialize)]
    struct Row {
        version: String,
        prerelease: bool,
    }
    #[derive(serde::Deserialize)]
    struct Case {
        request: String,
        exit: i32,
        stdout: String,
    }
    let reference: Reference = serde_json::from_str(include_str!(
        "../../../specs/031-cli-mise-replacement/contracts/mise-version-reference.json"
    ))
    .unwrap();
    assert_eq!(reference.cases.len(), 10);
    let temporary = tempfile::tempdir().unwrap();
    let project = temporary.path().join("版本 project");
    std::fs::create_dir_all(&project).unwrap();
    let home = temporary.path().join("aster-home");
    let store = ToolStore::open(home.join("tools")).unwrap();
    for row in &reference.versions {
        install_uv_fixture(&store, temporary.path(), &row.version).await;
    }
    let listing = VersionListing::new(
        reference
            .versions
            .into_iter()
            .map(|row| (row.version, row.prerelease))
            .collect(),
        std::collections::BTreeMap::new(),
    )
    .unwrap();
    let mut source = StaticSource::new();
    source.insert_versions("github:astral-sh/uv", listing);
    store
        .remote_versions_with(
            &parse_tool_spec("github:astral-sh/uv").unwrap(),
            Some(&source),
        )
        .unwrap();
    let remote = run(
        &project,
        &home,
        &["tool", "ls-remote", "uv", "--offline", "--json"],
    )
    .await;
    assert!(
        remote.status.success(),
        "{}",
        String::from_utf8_lossy(&remote.stderr)
    );
    let rows: serde_json::Value = serde_json::from_slice(&remote.stdout).unwrap();
    assert_eq!(rows["versions"].as_array().unwrap().len(), 11);
    for case in reference.cases {
        write_env_acl(
            project_env_path(&project),
            &EnvConfig {
                tools: std::collections::BTreeMap::from([("uv".into(), case.request.clone())]),
                vars: std::collections::BTreeMap::new(),
            },
        )
        .unwrap();
        let output = run(&project, &home, &["exec", "--offline", "--", "uv"]).await;
        if case.exit == 0 {
            assert!(
                output.status.success(),
                "{}: {}",
                case.request,
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(output.stdout, case.stdout.as_bytes(), "{}", case.request);
        } else {
            assert_eq!(output.status.code(), Some(1));
            assert!(output.stdout.is_empty());
        }
    }
    write_env_acl(
        project_env_path(&project),
        &EnvConfig {
            tools: std::collections::BTreeMap::from([("uv".into(), "sub-1:latest".into())]),
            vars: std::collections::BTreeMap::new(),
        },
    )
    .unwrap();
    let output = run(&project, &home, &["tool", "lock", "--offline"]).await;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    std::fs::remove_dir_all(store.root_path().join("cache/versions")).unwrap();
    let output = run(
        &project,
        &home,
        &["exec", "--locked", "--offline", "--", "uv"],
    )
    .await;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"2.79.0\n");
    assert_eq!(
        std::fs::read_to_string(store.root_path().join("current/uv")).unwrap(),
        "4.0.0-rc.1\n"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn cli_latest_can_execute_when_only_a_prerelease_is_installed() {
    let temporary = tempfile::tempdir().unwrap();
    let home = temporary.path().join("aster-home");
    let store = ToolStore::open(home.join("tools")).unwrap();
    install_uv_fixture(&store, temporary.path(), "2.0.0-rc.1").await;
    write_env_acl(
        project_env_path(temporary.path()),
        &EnvConfig {
            tools: std::collections::BTreeMap::from([("uv".into(), "latest".into())]),
            vars: std::collections::BTreeMap::new(),
        },
    )
    .unwrap();
    let output = run(temporary.path(), &home, &["exec", "--offline", "--", "uv"]).await;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"2.0.0-rc.1\n");
}

#[tokio::test]
async fn closed_stdout_returns_an_io_error_without_panicking() {
    let temporary = tempfile::tempdir().unwrap();
    let mut command = isolated_command(temporary.path(), &temporary.path().join("aster-home"));
    command
        .args(["env", "show", "--json"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let mut child = command.spawn().unwrap();
    drop(child.stdout.take().unwrap());

    let output = tokio::time::timeout(Duration::from_secs(20), child.wait_with_output())
        .await
        .unwrap()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    assert!(!String::from_utf8_lossy(&output.stderr).contains("panicked"));
}

#[cfg(unix)]
#[tokio::test]
async fn exec_preserves_non_utf8_process_arguments() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let temporary = tempfile::tempdir().unwrap();
    let raw = vec![b'a', 0xff, b'b'];
    let args = vec![
        OsString::from("exec"),
        OsString::from("--no-install"),
        OsString::from("--"),
        OsString::from("/bin/sh"),
        OsString::from("-c"),
        OsString::from("printf '%s' \"$1\""),
        OsString::from("sh"),
        OsString::from_vec(raw.clone()),
    ];

    let output = run(
        temporary.path(),
        &temporary.path().join("aster-home"),
        &args,
    )
    .await;

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, raw);
}

#[cfg(unix)]
#[tokio::test]
async fn exec_preserves_exit_codes_and_signal_termination() {
    use std::os::unix::process::ExitStatusExt;

    let temporary = tempfile::tempdir().unwrap();
    let home = temporary.path().join("aster-home");
    let failed = run(
        temporary.path(),
        &home,
        &["exec", "--no-install", "--", "/bin/sh", "-c", "exit 37"],
    )
    .await;
    assert_eq!(failed.status.code(), Some(37));

    let signaled = run(
        temporary.path(),
        &home,
        &[
            "exec",
            "--no-install",
            "--",
            "/bin/sh",
            "-c",
            "kill -TERM $$",
        ],
    )
    .await;
    assert_eq!(signaled.status.signal(), Some(libc::SIGTERM));
}

#[tokio::test]
async fn nested_cli_environment_inherits_user_and_ancestor_values() {
    let temporary = tempfile::tempdir().unwrap();
    let project = temporary.path().join("项目 with spaces");
    let nested = project.join("nested");
    let home = temporary.path().join("aster-home");
    std::fs::create_dir_all(&nested).unwrap();
    assert!(
        run(
            &project,
            &home,
            &["env", "set", "USER_VALUE=global", "--global"]
        )
        .await
        .status
        .success()
    );
    assert!(
        run(&project, &home, &["env", "set", "PROJECT_VALUE=root"])
            .await
            .status
            .success()
    );

    let output = run(&nested, &home, &["env", "show", "--json"]).await;

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["vars"]["USER_VALUE"], "global");
    assert_eq!(value["vars"]["PROJECT_VALUE"], "root");
    assert!(!home.join("docs/user-guide").exists());
}

#[tokio::test]
async fn failed_install_does_not_panic_or_replace_the_project_pin() {
    let temporary = tempfile::tempdir().unwrap();
    let project = temporary.path().join("project");
    let home = temporary.path().join("aster-home");
    let path = project_env_path(&project);
    write_env_acl(&path, &EnvConfig::default()).unwrap();
    let before = std::fs::read(&path).unwrap();

    let output = run(&project, &home, &["tool", "use", "hermes"]).await;

    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("tool runtime is missing: uv"));
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

#[tokio::test]
async fn invalid_variable_name_does_not_replace_the_project_file() {
    let temporary = tempfile::tempdir().unwrap();
    let project = temporary.path().join("project");
    let home = temporary.path().join("aster-home");
    let path = project_env_path(&project);
    write_env_acl(&path, &EnvConfig::default()).unwrap();
    let before = std::fs::read(&path).unwrap();

    let output = run(&project, &home, &["env", "set", "1INVALID=value"]).await;

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

#[tokio::test]
async fn unknown_shell_emits_no_activation_script() {
    let temporary = tempfile::tempdir().unwrap();
    let output = run(
        temporary.path(),
        &temporary.path().join("aster-home"),
        &["activate", "unknown-shell"],
    )
    .await;

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
}

#[cfg(unix)]
#[tokio::test]
async fn cli_exec_and_which_select_project_versions_without_network_or_pointer_changes() {
    let temporary = tempfile::tempdir().unwrap();
    let home = temporary.path().join("aster-home");
    let store = ToolStore::open(home.join("tools")).unwrap();
    let first = temporary.path().join("项目 A");
    let second = temporary.path().join("project B");
    for (project, version) in [(&first, "1.0.0"), (&second, "2.0.0")] {
        install_uv_fixture(&store, temporary.path(), version).await;
        let config = EnvConfig {
            tools: std::collections::BTreeMap::from([("uv".into(), version.into())]),
            vars: std::collections::BTreeMap::new(),
        };
        write_env_acl(project_env_path(project), &config).unwrap();
    }
    for (project, version) in [(&first, "1.0.0"), (&second, "2.0.0"), (&first, "1.0.0")] {
        for args in [
            vec!["exec", "--no-install", "--", "uv"],
            vec!["exec", "--", "uv"],
        ] {
            let output = run(project, &home, &args).await;
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), version);
        }
        let output = run(project, &home, &["tool", "which", "uv", "--json"]).await;
        assert!(output.status.success());
        let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        let expected = store
            .root_path()
            .join("installs")
            .join("uv")
            .join(version)
            .join("uv");
        assert_eq!(value["path"].as_str().unwrap(), expected.to_str().unwrap());
    }
    assert_eq!(
        std::fs::read_to_string(store.root_path().join("current/uv")).unwrap(),
        "2.0.0\n"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn cli_lock_keeps_latest_reproducible_and_refuses_drift_or_missing_companions() {
    let temporary = tempfile::tempdir().unwrap();
    let project = temporary.path().join("项目 with spaces");
    let nested = project.join("nested directory");
    let home = temporary.path().join("aster-home");
    std::fs::create_dir_all(&nested).unwrap();
    let store = ToolStore::open(home.join("tools")).unwrap();
    install_uv_fixture(&store, temporary.path(), "1.0.0").await;
    let mut config = EnvConfig {
        tools: std::collections::BTreeMap::from([("uv".into(), "latest".into())]),
        vars: std::collections::BTreeMap::from([("TOKEN".into(), "synthetic-lock-secret".into())]),
    };
    write_env_acl(project_env_path(&project), &config).unwrap();
    let output = run(&project, &home, &["tool", "lock", "--offline", "--json"]).await;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !std::fs::read_to_string(project_env_lock_path(&project))
            .unwrap()
            .contains("synthetic-lock-secret")
    );
    install_uv_fixture(&store, temporary.path(), "2.0.0").await;
    for args in [
        vec!["exec", "--locked", "--offline", "--", "uv"],
        vec!["exec", "--locked", "--no-install", "--", "uv"],
    ] {
        let output = run(&nested, &home, &args).await;
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout, b"1.0.0\n");
    }
    let output = run(
        &project,
        &home,
        &["tool", "install", "--locked", "--offline", "--json"],
    )
    .await;
    assert!(output.status.success());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap()[0]["version"],
        "1.0.0"
    );
    assert_eq!(
        std::fs::read_to_string(store.root_path().join("current/uv")).unwrap(),
        "2.0.0\n"
    );

    config.tools.insert("uv".into(), "2.0.0".into());
    write_env_acl(project_env_path(&project), &config).unwrap();
    let output = run(
        &nested,
        &home,
        &["exec", "--locked", "--offline", "--", "uv"],
    )
    .await;
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("does not match tool declarations"));
    config.tools.insert("uv".into(), "latest".into());
    write_env_acl(project_env_path(&project), &config).unwrap();
    std::fs::remove_file(store.root_path().join("installs/uv/1.0.0/lib/required.txt")).unwrap();
    let output = run(
        &nested,
        &home,
        &["exec", "--locked", "--offline", "--", "uv"],
    )
    .await;
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("do not match env lock"));
}

#[tokio::test]
async fn offline_install_and_missing_lock_refuse_without_network() {
    let temporary = tempfile::tempdir().unwrap();
    let home = temporary.path().join("aster-home");
    for args in [
        vec!["tool", "install", "hermes", "--offline"],
        vec!["tool", "install", "--locked", "--offline"],
    ] {
        let output = run(temporary.path(), &home, &args).await;
        assert_eq!(output.status.code(), Some(1));
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(!stderr.contains("panicked"));
        assert!(
            stderr.contains("downloads disabled") || stderr.contains("project tool lock missing"),
            "{stderr}"
        );
    }
}

#[cfg(unix)]
#[tokio::test]
async fn legacy_v1_literal_requests_execute_the_pinned_tool_and_keep_the_current_pointer() {
    use aster_use_tool_acquire::EnvLock;

    for version in ["system", "ref-main", "ref-"] {
        let temporary = tempfile::tempdir().unwrap();
        let project = temporary.path().join("旧项目 with spaces");
        let nested = project.join("nested");
        let home = temporary.path().join("home");
        std::fs::create_dir_all(&nested).unwrap();
        let store = ToolStore::open(home.join("tools")).unwrap();
        install_uv_fixture(&store, temporary.path(), version).await;
        let latest = EnvConfig {
            tools: std::collections::BTreeMap::from([("uv".into(), "latest".into())]),
            vars: Default::default(),
        };
        let legacy = EnvLock::capture(&latest, &store)
            .unwrap()
            .render()
            .replace("aster.env-lock.v3", "aster.env-lock.v1")
            .replace("request = \"latest\"", &format!("request = \"{version}\""))
            .lines()
            .filter(|line| !line.trim_start().starts_with("selector ="))
            .collect::<Vec<_>>()
            .join("\n");
        let config = EnvConfig {
            tools: std::collections::BTreeMap::from([("uv".into(), version.into())]),
            vars: Default::default(),
        };
        write_env_acl(project_env_path(&project), &config).unwrap();
        std::fs::write(project_env_lock_path(&project), legacy.as_bytes()).unwrap();
        install_uv_fixture(&store, temporary.path(), "9.9.9").await;
        for args in [
            vec!["exec", "--locked", "--offline", "--", "uv"],
            vec!["exec", "--locked", "--no-install", "--", "uv"],
        ] {
            let output = run(&nested, &home, &args).await;
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(output.stdout, format!("{version}\n").as_bytes());
        }
        let output = run(
            &nested,
            &home,
            &["tool", "install", "--locked", "--offline", "--json"],
        )
        .await;
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let reported: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(
            reported
                .pointer("/0/version")
                .and_then(serde_json::Value::as_str),
            Some(version)
        );
        assert_eq!(
            std::fs::read_to_string(store.root_path().join("current/uv")).unwrap(),
            "9.9.9\n"
        );
        assert_eq!(
            std::fs::read_to_string(project_env_lock_path(&project)).unwrap(),
            legacy
        );
    }
}

#[cfg(unix)]
fn write_context_node(path: &Path, word: &str) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, format!("#!/bin/sh\nprintf '%s\\n' '{word}'\n")).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

#[cfg(unix)]
async fn run_context_case(
    project: &Path,
    aster_home: &Path,
    user_home: &Path,
    path: &OsStr,
    args: &[&str],
) -> Output {
    tokio::time::timeout(
        Duration::from_secs(20),
        isolated_command(project, aster_home)
            .env("HOME", user_home)
            .env("PATH", path)
            .args(args)
            .output(),
    )
    .await
    .expect("context fixture command timed out")
    .expect("spawn Aster context command")
}

#[cfg(unix)]
#[tokio::test]
async fn package_commands_use_the_embedded_sdk_without_an_external_companion() {
    let temporary = tempfile::tempdir().unwrap();
    let project = temporary.path().join("项目 with spaces");
    let home = temporary.path().join("aster-home");
    std::fs::create_dir_all(&project).unwrap();
    let bin = temporary.path().join("isolated-bin");
    std::fs::create_dir(&bin).unwrap();
    let binary = bin.join("aster");
    // Isolate sibling lookup as well as PATH; the build directory can contain a standalone SDK.
    std::fs::copy(env!("CARGO_BIN_EXE_aster"), &binary).unwrap();
    assert!(!bin.join("aster-use").exists());
    let poison = temporary.path().join("bin/aster-use");
    write_context_node(&poison, "external-companion");
    let path = std::env::join_paths([
        poison.parent().unwrap(),
        Path::new("/usr/bin"),
        Path::new("/bin"),
    ])
    .unwrap();
    let output = isolated_binary_command(&binary, &project, &home)
        .env("ASTER_USE_HOME", home.join("use"))
        .env("HOME", temporary.path().join("user-home"))
        .env("PATH", &path)
        .args(["use", "capabilities", "--json"])
        .output();
    let output = tokio::time::timeout(Duration::from_secs(20), output)
        .await
        .unwrap()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["schemaVersion"], 1);
    assert_eq!(value["ok"], true);
    assert!(value["data"]["domains"].is_array());
    let output = isolated_binary_command(&binary, &project, &home)
        .env("ASTER_USE_HOME", home.join("use"))
        .env("HOME", temporary.path().join("user-home"))
        .env("PATH", &path)
        .args(["use", "unknown-route", "--json"])
        .output();
    let output = tokio::time::timeout(Duration::from_secs(20), output)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["error"]["code"], "use.route_unknown");
    assert!(!home.join("docs/user-guide").exists());
}

#[cfg(unix)]
#[tokio::test]
async fn an_explicit_package_command_override_preserves_native_arguments_and_exit_status() {
    use std::os::unix::ffi::OsStringExt;
    use std::os::unix::fs::PermissionsExt;
    let temporary = tempfile::tempdir().unwrap();
    let script = temporary.path().join("explicit package command");
    std::fs::write(&script, b"#!/bin/sh\nprintf '%s\\n' \"$1\"\nexit 37\n").unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    let argument = std::ffi::OsString::from_vec(b"native-\xff".to_vec());
    let output = isolated_command(temporary.path(), &temporary.path().join("aster-home"))
        .env("ASTER_USE_BIN", &script)
        .arg("use")
        .arg(argument)
        .output();
    let output = tokio::time::timeout(Duration::from_secs(20), output)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(output.status.code(), Some(37));
    assert_eq!(output.stdout, b"native-\xff\n");
}

#[cfg(unix)]
#[tokio::test]
async fn configured_path_and_system_requests_match_frozen_mise_context_results() {
    #[derive(serde::Deserialize)]
    struct Reference {
        cases: Vec<Case>,
    }
    #[derive(serde::Deserialize)]
    struct Case {
        name: String,
        source: Source,
        kind: Kind,
        managed_shim_in_path: bool,
        exit: i32,
        stdout: String,
    }
    #[derive(serde::Deserialize, PartialEq, Eq)]
    #[serde(rename_all = "lowercase")]
    enum Source {
        Argument,
        Project,
        Global,
    }
    #[derive(serde::Deserialize, PartialEq, Eq)]
    #[serde(rename_all = "lowercase")]
    enum Kind {
        Absolute,
        Relative,
        Home,
        System,
        Missing,
    }

    let reference: Reference = serde_json::from_str(include_str!(
        "../../../specs/031-cli-mise-replacement/contracts/mise-context-reference.json"
    ))
    .unwrap();
    assert_eq!(reference.cases.len(), 7);
    for case in reference.cases {
        let temporary = tempfile::tempdir().unwrap();
        let project = temporary.path().join("项目 with spaces");
        let cwd = project.join("nested");
        let aster_home = temporary.path().join("aster-home");
        let user_home = temporary.path().join("home");
        std::fs::create_dir_all(&cwd).unwrap();
        let external = if case.kind == Kind::Home || case.source == Source::Global {
            user_home.join("SDK node")
        } else if case.source == Source::Project {
            project.join("external node")
        } else {
            cwd.join("external node")
        };
        write_context_node(&external.join("bin/node"), "external-node");
        let system = temporary.path().join("system-bin");
        write_context_node(&system.join("node"), "system-node");
        let store = ToolStore::open(aster_home.join("tools")).unwrap();
        write_context_node(&store.root_path().join("shims/node"), "managed-shim");
        let mut paths = Vec::new();
        if case.managed_shim_in_path {
            paths.push(store.root_path().join("shims"));
        }
        paths.extend([
            system.clone(),
            Path::new("/usr/bin").into(),
            Path::new("/bin").into(),
        ]);
        let inherited = std::env::join_paths(paths).unwrap();
        let request = match case.kind {
            Kind::Absolute => format!("path:{}", external.to_str().unwrap()),
            Kind::Relative => format!("path:./{}", external.file_name().unwrap().to_str().unwrap()),
            Kind::Home => "path:~/SDK node".into(),
            Kind::System => "system".into(),
            Kind::Missing => "path:./missing node".into(),
        };
        let declaration = match case.source {
            Source::Argument => project_env_path(&cwd),
            Source::Project => project_env_path(&project),
            Source::Global => aster_use_tool_acquire::user_env_path(&aster_home),
        };
        let argument_pin = format!("node@{request}");
        write_env_acl(
            declaration,
            &EnvConfig {
                tools: std::collections::BTreeMap::from([("node".into(), request)]),
                vars: Default::default(),
            },
        )
        .unwrap();
        for args in [
            vec!["exec", "--", "node"],
            vec!["exec", "--offline", "--", "node"],
            vec!["exec", "--no-install", "--", "node"],
        ] {
            let mut variants = vec![args.clone()];
            if case.source == Source::Argument {
                let mut argument = args;
                argument.splice(1..1, ["--tool", argument_pin.as_str()]);
                variants.push(argument);
            }
            for args in variants {
                let output =
                    run_context_case(&cwd, &aster_home, &user_home, &inherited, &args).await;
                assert_eq!(
                    output.status.code(),
                    Some(case.exit),
                    "{}: {}",
                    case.name,
                    String::from_utf8_lossy(&output.stderr)
                );
                assert_eq!(output.stdout, case.stdout.as_bytes(), "{}", case.name);
            }
        }
        if case.exit == 0 {
            let output = run_context_case(
                &cwd,
                &aster_home,
                &user_home,
                &inherited,
                &["tool", "which", "node", "--json"],
            )
            .await;
            assert!(
                output.status.success(),
                "{}: {}",
                case.name,
                String::from_utf8_lossy(&output.stderr)
            );
            let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
            let selected = value
                .get("path")
                .and_then(serde_json::Value::as_str)
                .unwrap();
            let expected = if case.kind == Kind::System {
                system.join("node")
            } else {
                external.join("bin/node")
            };
            assert_eq!(
                std::fs::canonicalize(selected).unwrap(),
                expected.canonicalize().unwrap()
            );
            let output = run_context_case(
                &cwd,
                &aster_home,
                &user_home,
                &inherited,
                &["tool", "install", "--offline", "--json"],
            )
            .await;
            assert!(
                output.status.success(),
                "{}: {}",
                case.name,
                String::from_utf8_lossy(&output.stderr)
            );
            let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
            let origin = if case.kind == Kind::System {
                "system"
            } else {
                "path"
            };
            assert_eq!(
                value
                    .pointer("/0/origin")
                    .and_then(serde_json::Value::as_str),
                Some(origin)
            );
            let script = run_context_case(
                &cwd,
                &aster_home,
                &user_home,
                &inherited,
                &["activate", "sh"],
            )
            .await;
            assert!(script.status.success());
            let script = format!("{}\nnode", String::from_utf8(script.stdout).unwrap());
            let output = std::process::Command::new("/bin/sh")
                .arg("-c")
                .arg(script)
                .env_clear()
                .env("PATH", &inherited)
                .current_dir(&cwd)
                .output()
                .unwrap();
            assert!(output.status.success());
            assert_eq!(output.stdout, case.stdout.as_bytes());
        }
        assert!(
            !store.root_path().join("receipts").exists(),
            "{}",
            case.name
        );
        assert!(!store.root_path().join("current").exists(), "{}", case.name);
        assert!(
            !aster_home.join("docs/user-guide").exists(),
            "{}",
            case.name
        );
    }
}

#[cfg(unix)]
#[tokio::test]
async fn ordered_exec_overrides_match_frozen_mise_and_preserve_configuration() {
    #[derive(serde::Deserialize)]
    struct Reference {
        status: String,
        binary_sha256: String,
        cases: Vec<Case>,
    }
    #[derive(serde::Deserialize)]
    struct Case {
        name: String,
        tool: String,
        requests: Vec<String>,
        command: String,
        execution: Execution,
    }
    #[derive(serde::Deserialize)]
    struct Execution {
        exit: i32,
        stdout: String,
        path_order: Vec<String>,
    }
    let reference: Reference = serde_json::from_str(include_str!(
        "../../../specs/031-cli-mise-replacement/contracts/mise-exec-multiversion-reference.json"
    ))
    .unwrap();
    let baseline: serde_json::Value = serde_json::from_str(include_str!(
        "../../../specs/031-cli-mise-replacement/contracts/mise-cli-reference.json"
    ))
    .unwrap();
    assert_eq!(reference.status, "passed");
    assert_eq!(
        reference.binary_sha256,
        baseline["binary_sha256"].as_str().unwrap()
    );
    assert_eq!(reference.cases.len(), 9);
    for case in reference.cases {
        let name = &case.name;
        let (tool, program) = match (case.tool.as_str(), case.command.as_str()) {
            ("fixture", "fixture") => ("uv", "uv"),
            ("fixture", "fixture-extra") => ("uv", "uv-extra"),
            ("node", "node") => ("node", "node"),
            ("node", "node-extra") => ("node", "node-extra"),
            other => panic!("unadmitted reference target: {other:?}"),
        };
        let temporary = tempfile::tempdir().unwrap();
        let project = temporary.path().join("项目 with spaces");
        let cwd = project.join("nested");
        let home = temporary.path().join("home");
        let aster_home = temporary.path().join("aster-home");
        std::fs::create_dir_all(&cwd).unwrap();
        write_context_node(&cwd.join("one/bin/node"), "one");
        write_context_node(&cwd.join("two/bin/node"), "two");
        write_context_node(&cwd.join("two/bin/node-extra"), "extra:two");
        write_context_node(&project.join("inherited/bin/gh"), "kept-gh");
        let system = temporary.path().join("system-bin");
        write_context_node(&system.join("node"), "system");
        let inherited = std::env::join_paths([
            system.clone(),
            Path::new("/usr/bin").into(),
            Path::new("/bin").into(),
        ])
        .unwrap();
        let mut owned_dirs = std::collections::BTreeMap::from([
            (
                "one".to_owned(),
                cwd.join("one/bin").canonicalize().unwrap(),
            ),
            (
                "two".to_owned(),
                cwd.join("two/bin").canonicalize().unwrap(),
            ),
            ("system".to_owned(), system.canonicalize().unwrap()),
        ]);
        let store = ToolStore::open(aster_home.join("tools")).unwrap();
        if tool == "uv" {
            for version in ["1.0.0", "2.0.0"] {
                install_multiversion_uv_fixture(&store, temporary.path(), version).await;
                let receipt = store
                    .installed_receipt("uv", Some(version))
                    .unwrap()
                    .unwrap();
                let paths = store.installation_paths(&receipt).unwrap();
                assert_eq!(paths.bin_dirs().len(), 1);
                owned_dirs.insert(
                    version.to_owned(),
                    paths.bin_dirs()[0].canonicalize().unwrap(),
                );
            }
        }
        let declaration = project_env_path(&project);
        write_env_acl(
            &declaration,
            &EnvConfig {
                tools: std::collections::BTreeMap::from([
                    (tool.into(), "path:./overridden missing directory".into()),
                    ("gh".into(), "path:./inherited".into()),
                ]),
                vars: std::collections::BTreeMap::from([("KEPT".into(), "literal value".into())]),
            },
        )
        .unwrap();
        let before = std::fs::read(&declaration).unwrap();
        for mode in ["--offline", "--no-install"] {
            let mut arguments = vec!["exec".to_owned(), mode.to_owned()];
            for request in &case.requests {
                arguments.extend(["-t".into(), format!("{tool}@{request}")]);
            }
            arguments.extend(
                [
                    "--",
                    "/bin/sh",
                    "-c",
                    "set -eu; \"$1\"; gh; printf '%s\\n' \"$KEPT\" \"$PATH\"",
                    "aster-fixture",
                    program,
                ]
                .map(str::to_owned),
            );
            let arguments = arguments.iter().map(String::as_str).collect::<Vec<_>>();
            let output = run_context_case(&cwd, &aster_home, &home, &inherited, &arguments).await;
            assert_eq!(
                output.status.code(),
                Some(case.execution.exit),
                "{name}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            let stdout = String::from_utf8(output.stdout).unwrap();
            let lines = stdout.lines().collect::<Vec<_>>();
            assert_eq!(lines.len(), 4, "{name}: {stdout}");
            assert_eq!(format!("{}\n", lines[0]), case.execution.stdout, "{name}");
            assert_eq!(lines[1..3], ["kept-gh", "literal value"], "{name}");
            let order = std::env::split_paths(OsStr::new(lines[3]))
                .filter_map(|path| {
                    let path = path.canonicalize().unwrap();
                    owned_dirs
                        .iter()
                        .find_map(|(label, directory)| (directory == &path).then(|| label.clone()))
                })
                .collect::<Vec<_>>();
            assert_eq!(order, case.execution.path_order, "{name}");
        }
        if name == "native-forward" {
            let marker = cwd.join("must not run 文件");
            let output = run_context_case(
                &cwd,
                &aster_home,
                &home,
                &inherited,
                &[
                    "exec",
                    "--no-install",
                    "-t",
                    "node@path:./one",
                    "-t",
                    "node@path:./missing",
                    "--",
                    "/bin/sh",
                    "-c",
                    "printf effect > \"$1\"",
                    "aster-fixture",
                    marker.to_str().unwrap(),
                ],
            )
            .await;
            assert!(!output.status.success());
            assert!(!marker.exists());
        }
        assert_eq!(std::fs::read(&declaration).unwrap(), before, "{name}");
        assert!(!project_env_path(&cwd).exists());
        if tool == "uv" {
            assert_eq!(
                store
                    .installed_receipt("uv", None)
                    .unwrap()
                    .unwrap()
                    .version,
                "2.0.0"
            );
        } else {
            assert!(!store.root_path().join("receipts").exists());
            assert!(!store.root_path().join("current").exists());
        }
    }
}

#[cfg(unix)]
#[tokio::test]
async fn exec_tool_overrides_use_the_argument_root_and_preserve_configuration() {
    let temporary = tempfile::tempdir().unwrap();
    let project = temporary.path().join("项目 with spaces");
    let cwd = project.join("nested");
    let home = temporary.path().join("home");
    let aster_home = temporary.path().join("aster-home");
    std::fs::create_dir_all(&cwd).unwrap();
    write_context_node(&cwd.join("SDK node/bin/node"), "external-node");
    write_context_node(&project.join("SDK uv/bin/uv"), "parent-uv");
    let system = temporary.path().join("system-bin");
    write_context_node(&system.join("node"), "system-node");
    let inherited = std::env::join_paths([
        system,
        Path::new("/usr/bin").into(),
        Path::new("/bin").into(),
    ])
    .unwrap();
    let declaration = project_env_path(&project);
    write_env_acl(
        &declaration,
        &EnvConfig {
            tools: std::collections::BTreeMap::from([
                ("node".into(), "path:./missing declared node".into()),
                ("uv".into(), "path:./SDK uv".into()),
            ]),
            vars: Default::default(),
        },
    )
    .unwrap();
    let before = std::fs::read(&declaration).unwrap();
    for (request, expected) in [
        ("node@path:./SDK node", "external-node\nparent-uv\n"),
        ("node@system", "system-node\nparent-uv\n"),
    ] {
        for mode in ["--offline", "--no-install"] {
            let output = run_context_case(
                &cwd,
                &aster_home,
                &home,
                &inherited,
                &[
                    "exec", mode, "--tool", request, "--", "/bin/sh", "-c", "node; uv",
                ],
            )
            .await;
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(output.stdout, expected.as_bytes());
        }
    }
    let marker = cwd.join("must-not-run");
    let output = run_context_case(
        &cwd,
        &aster_home,
        &home,
        &inherited,
        &[
            "exec",
            "--locked",
            "--tool",
            "node@system",
            "--",
            "/usr/bin/touch",
            marker.to_str().unwrap(),
        ],
    )
    .await;
    assert_eq!(output.status.code(), Some(2));
    assert!(!marker.exists());
    assert_eq!(std::fs::read(&declaration).unwrap(), before);
    assert!(!project_env_path(&cwd).exists());
    assert!(!aster_home.join("tools/receipts").exists());
    assert!(!aster_home.join("tools/current").exists());
    assert!(!aster_home.join("docs/user-guide").exists());
}

#[cfg(unix)]
#[tokio::test]
async fn path_use_persists_the_argument_root_and_system_use_does_not_create_receipts() {
    let temporary = tempfile::tempdir().unwrap();
    let cwd = temporary.path().join("project");
    let home = temporary.path().join("home");
    let aster_home = temporary.path().join("aster-home");
    std::fs::create_dir_all(&cwd).unwrap();
    let external = cwd.join("SDK node");
    write_context_node(&external.join("bin/node"), "external-node");
    let inherited = OsStr::new("/usr/bin:/bin");
    let used = run_context_case(
        &cwd,
        &aster_home,
        &home,
        inherited,
        &["tool", "use", "node@path:./SDK node", "--global", "--json"],
    )
    .await;
    assert!(
        used.status.success(),
        "{}",
        String::from_utf8_lossy(&used.stderr)
    );
    let pinned =
        aster_use_tool_acquire::read_env_acl(aster_use_tool_acquire::user_env_path(&aster_home))
            .unwrap()
            .unwrap();
    let expected = format!(
        "path:{}",
        external.canonicalize().unwrap().to_str().unwrap()
    );
    assert_eq!(pinned.tools.get("node"), Some(&expected));
    let used = run_context_case(
        &cwd,
        &aster_home,
        &home,
        inherited,
        &["tool", "use", "node@system", "--json"],
    )
    .await;
    assert!(
        used.status.success(),
        "{}",
        String::from_utf8_lossy(&used.stderr)
    );
    let pinned = aster_use_tool_acquire::read_env_acl(project_env_path(&cwd))
        .unwrap()
        .unwrap();
    assert_eq!(pinned.tools.get("node").map(String::as_str), Some("system"));
    let status = run_context_case(
        &cwd,
        &aster_home,
        &home,
        inherited,
        &["tool", "install", "--offline"],
    )
    .await;
    assert!(status.status.success());
    assert_eq!(status.stdout, b"Using system node.\n");
    assert!(!aster_home.join("tools/receipts").exists());
}

#[cfg(unix)]
#[tokio::test]
async fn missing_pin_refuses_before_starting_the_requested_process() {
    let temporary = tempfile::tempdir().unwrap();
    let home = temporary.path().join("aster-home");
    let marker = temporary.path().join("must-not-run");
    let config = EnvConfig {
        tools: std::collections::BTreeMap::from([("uv".into(), "1.0.0".into())]),
        vars: std::collections::BTreeMap::new(),
    };
    write_env_acl(project_env_path(temporary.path()), &config).unwrap();

    let output = run(
        temporary.path(),
        &home,
        &[
            "exec",
            "--no-install",
            "--",
            "/bin/sh",
            "-c",
            "touch \"$1\"",
            "sh",
            marker.to_str().unwrap(),
        ],
    )
    .await;

    assert_eq!(output.status.code(), Some(1));
    assert!(!marker.exists());
}
