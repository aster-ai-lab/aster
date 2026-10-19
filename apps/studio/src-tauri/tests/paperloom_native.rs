#![cfg(test)]

// Share production commands and transport without adding a test launcher to the shipped app.
include!("../src/lib.rs");

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(std::env::var("A3S_STUDIO_PAPERLOOM_E2E_ROOT")?);
    if !root.is_absolute() || !root.join(".owned").is_file() {
        return Err("an absolute owned test root is required".into());
    }
    let instance = tenant::instance_name()?.ok_or("an isolated test instance is required")?;
    if !instance.starts_with("paperloom-e2e-") {
        return Err("the test instance must use the paperloom-e2e prefix".into());
    }
    let renderer = url::Url::parse(&std::env::var("A3S_STUDIO_PAPERLOOM_E2E_RENDERER")?)?;
    if renderer.scheme() != "http"
        || !matches!(
            renderer.host_str(),
            Some("127.0.0.1" | "localhost" | "[::1]")
        )
        || renderer.path() != "/windows/main/index.html"
        || !renderer.username().is_empty()
        || renderer.password().is_some()
        || renderer.query().is_some()
        || renderer.fragment().is_some()
    {
        return Err("the test renderer must be a loopback main document".into());
    }
    let origin =
        std::env::var("A3S_ORG_ORIGIN").unwrap_or_else(|_| "http://127.0.0.1:9".to_owned());
    let mut authority = StudioAuthority::start_with_store(&origin, root.join("capabilities"))?;
    // This fixture proves native interactions, not production OS authentication.
    if root.join(".fixture-member").is_file() {
        let member = authority.preference_member_session();
        member.note_sign_in(
            "a3s_ms_paperloom.e2e.fixture.only",
            "33333333-3333-4333-8333-333333333333",
        );
        member.save(r#"{"token":"a3s_ms_paperloom.e2e.fixture.only","display_name":"论文精读测试成员","phone":"13800138000"}"#.as_bytes())
            .map_err(|status| format!("fixture member refused: {status}"))?;
    }
    let storage = tenant::storage_slug(authority.slug())?;
    let runtime = serde_json::json!({
        "apiBaseUrl":authority.api_base_url(), "token":authority.token(),
        "homePath":user_home(),
    });
    let initialization = boot_contract::boot_initialization_script(
        true,
        true,
        authority.slug(),
        authority.product(),
        &runtime,
        true,
    );
    let receipt = root.join("runtime.json");
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&receipt)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    }
    use std::io::Write;
    serde_json::to_writer(
        &mut file,
        &serde_json::json!({
            "runtime":runtime, "pid":std::process::id(), "instance":instance,
            "background":true, "profile":tenant::profile_dir(&storage),
        }),
    )?;
    file.flush()?;
    let member = authority.preference_member_session();
    let access = authority.applet_access();
    let product = authority.product().to_owned();
    let base = authority.api_base_url();
    let token = authority.token().to_owned();
    let mut context = studio_context();
    context.config_mut().build.dev_url = Some(renderer.join("/")?);
    let fixture_workspace = root
        .join(".fixture-member")
        .is_file()
        .then(|| root.join("workspace"));
    let builder = tauri::Builder::default();
    #[cfg(target_os = "macos")]
    let builder = builder.activate_ignoring_other_apps(false);
    let app = builder
        .invoke_handler(studio_invoke_handler())
        .setup(move |app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            app.handle().set_theme(Some(Theme::Light));
            let preferences = tenant::profile_dir(&storage).ok_or("test profile unavailable")?;
            if let Some(workspace) = fixture_workspace {
                let scope = member.member_id().ok_or("fixture member is unavailable")?;
                let store = preferences::PreferenceStore::new(preferences.join("preferences"));
                let snapshot = store.read(&scope)?;
                store.commit(&preferences::Commit {
                    scope,
                    expected_revision: snapshot.revision,
                    updates: std::collections::BTreeMap::from([(
                        "app.workspace.path".to_owned(),
                        serde_json::Value::String(
                            std::fs::canonicalize(workspace)?
                                .to_string_lossy()
                                .into_owned(),
                        ),
                    )]),
                })?;
            }
            app.manage(preferences::PreferenceHost::new(
                preferences.join("preferences"),
                member,
                token.clone(),
            ));
            app.manage(workspace_terminal::WorkspaceTerminalState::default());
            applet_window::remember_host(app.handle(), &storage, &product, &base, &token, access);
            WebviewWindowBuilder::new(app, "main", WebviewUrl::External(renderer))
                .initialization_script(initialization)
                .data_store_identifier(applet_window::data_store_id(&storage, "paperloom-e2e"))
                .background_throttling(tauri::utils::config::BackgroundThrottlingPolicy::Disabled)
                .title("论文精读 E2E")
                .inner_size(1280.0, 800.0)
                .focused(false)
                .visible(true)
                .build()?;
            Ok(())
        })
        .build(context)?;
    let handle = app.handle().clone();
    let stopped = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let done = stopped.clone();
    let watchdog = std::thread::spawn(move || {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3600);
        while !done.load(std::sync::atomic::Ordering::Acquire) {
            if root.join("stop").exists() || std::time::Instant::now() >= deadline {
                handle.exit(0);
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
    });
    let code = app.run_return(|handle, event| {
        if matches!(
            event,
            RunEvent::WindowEvent {
                event: WindowEvent::Destroyed,
                ..
            }
        ) {
            handle.exit(0);
        }
    });
    stopped.store(true, std::sync::atomic::Ordering::Release);
    watchdog.join().map_err(|_| "test watchdog failed")?;
    authority.shutdown();
    std::fs::remove_file(receipt)?;
    if code != 0 {
        return Err(format!("the native test host exited with {code}").into());
    }
    Ok(())
}
