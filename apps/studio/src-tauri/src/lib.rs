mod agent_plane;

pub use agent_plane::{
    PrivateMemory, is_toboss_room_session, memory_directives, merge_memory_turn,
    merge_room_memory_turn, private_memory, room_memory_directives, room_retain, room_turn_ask,
    toboss_room_parts,
};
mod app_updater;
mod applet_product;
mod applet_runtime;
mod applet_window;

#[cfg(target_os = "macos")]
#[doc(hidden)]
pub use applet_window::probe_applet_titlebar;
mod agent_wiki_window;
mod aix_applet_window;
mod authority;
mod binary_tools;
mod boot_contract;
mod capabilities;

pub(crate) use boot_contract::{
    boot_initialization_script, is_main_renderer_document, main_renderer_resume_script,
};
mod channel_gateway;
mod code_git;
#[path = "../../../../crates/code/acp/src/connector_notes.rs"]
mod connector_notes;
mod data_sqlite;
mod desktop_settings;
mod document_result_handoff;
mod external_open;
mod file_editor_window;
mod folder_dialog;
mod host_family;
mod local_assets;
mod local_documents;
mod local_skills;
#[cfg(target_os = "macos")]
mod macos_app;

#[cfg(target_os = "macos")]
pub use macos_app::{AccessibilityPublish, publish_accessibility_windows};
mod mail_compose_window;
mod member_session;
mod message_attention;
mod notice_window;
mod office_tool;
mod okf_compile;
mod org_login;
mod paperloom;
mod pdf_tool_handoff;
mod plugin_okf;
mod preferences;
mod providers;
mod quark_suggestions;
mod review_plugin;
mod screenshot;
mod single_instance;
mod skill_links;
mod tenant;
mod window_drag;
mod workspace_browser;
mod workspace_document;
mod workspace_entries;
mod workspace_file;
mod workspace_search;
mod workspace_snapshot;
mod workspace_terminal;
mod workspace_watch;

use agent_wiki_window::open_agent_wiki_window;
use aix_applet_window::{
    close_aix_applet_window, open_aix_applet_window, protect_aix_applet_window,
    read_aix_applet_home_deck, report_aix_applet_home_deck, take_aix_applet_home_seed,
};
use applet_window::{deliver_workbench_review, open_workbench_applet_window};
use file_editor_window::{open_file_editor_window, prepare_message_file_preview};
use mail_compose_window::{open_mail_compose_window, publish_mail_compose_result};
use notice_window::open_notice_window;
use office_tool::office_tool;
use pdf_tool_handoff::{drop_pdf_tool_handoff, stage_pdf_tool_handoff, take_pdf_tool_handoff};
use window_drag::set_window_drag_regions;

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
#[cfg(unix)]
use std::sync::OnceLock;
#[cfg(target_os = "macos")]
use std::sync::atomic::{AtomicBool, Ordering};

use authority::StudioAuthority;
use tauri::menu::{AboutMetadata, Menu, PredefinedMenuItem, Submenu};
use tauri::{Manager, RunEvent, Theme, WebviewUrl, WebviewWindowBuilder, WindowEvent};

/// `just fork` on a platform that does not relaunch into an app bundle.
/// The macOS launcher picks the id before the bundle process starts.
fn claim_next_fork(brand_slug: &str) -> (String, single_instance::InstanceLock) {
    for number in 1..=10_000 {
        let id = format!("fork{number}");
        let Ok(storage) = tenant::storage_slug_for(brand_slug, Some(&id)) else {
            eprintln!("just fork: studio instance id is too long");
            std::process::exit(1);
        };
        if single_instance::is_held(&storage) {
            continue;
        }
        // Startup has one thread. Later reads of the instance id see this value.
        unsafe {
            std::env::set_var("A3S_STUDIO_INSTANCE", &id);
            std::env::set_var("A3S_STUDIO_FORK_ALLOC", "1");
            std::env::set_var(
                "STUDIO_RUNTIME_FILE",
                std::env::temp_dir().join(format!("a3s-studio-runtime-{brand_slug}-{id}.json")),
            );
        }
        match single_instance::claim_quiet(&storage) {
            single_instance::Claim::Owned(lock) => return (storage, lock),
            single_instance::Claim::Busy => continue,
            single_instance::Claim::Unavailable(error) => {
                eprintln!("just fork: single-instance lock for {storage}: {error}");
                std::process::exit(1);
            }
        }
    }
    eprintln!("just fork: no free window");
    std::process::exit(1);
}

fn smoke_line(text: &str) {
    println!("{text}");
    eprintln!("{text}");
    let _ = std::io::Write::flush(&mut std::io::stdout());
    let _ = std::io::Write::flush(&mut std::io::stderr());
}

#[cfg(target_os = "windows")]
#[link(name = "user32")]
unsafe extern "system" {
    fn GetSystemMetrics(n_index: i32) -> i32;
}

fn smoke_display_available() -> bool {
    #[cfg(target_os = "windows")]
    {
        // SM_CMONITORS. A session without an interactive desktop reports zero.
        const SM_CMONITORS: i32 = 80;
        return unsafe { GetSystemMetrics(SM_CMONITORS) > 0 };
    }
    #[cfg(target_os = "linux")]
    {
        return std::env::var_os("DISPLAY").is_some()
            || std::env::var_os("WAYLAND_DISPLAY").is_some();
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        true
    }
}

fn smoke_wait_until_visible(handle: tauri::AppHandle) {
    let started = std::time::Instant::now();
    loop {
        let (sender, receiver) = std::sync::mpsc::channel();
        let probe = handle.clone();
        let queued = handle.run_on_main_thread(move || {
            let visible = probe
                .get_webview_window("main")
                .and_then(|window| window.is_visible().ok())
                .unwrap_or(false);
            if visible {
                if let Some(window) = probe.get_webview_window("main") {
                    let _ = window.close();
                }
            }
            let _ = sender.send(visible);
        });
        if queued.is_err() {
            eprintln!("smoke-window main thread unavailable");
            handle.exit(1);
            return;
        }
        match receiver.recv_timeout(std::time::Duration::from_secs(2)) {
            Ok(true) => {
                smoke_line("smoke-window closed");
                handle.exit(0);
                return;
            }
            Ok(false) | Err(_) if started.elapsed() < std::time::Duration::from_secs(20) => {
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            _ => {
                eprintln!("smoke-window timed out waiting for visibility");
                handle.exit(1);
                return;
            }
        }
    }
}

/// Reads `A3S_STUDIO_DESKTOP_SMOKE=1` and returns without opening the product shell.
/// Directory selection prints a result and does not open a panel.
fn run_desktop_smoke() {
    let page = main_renderer_url(packaged_shell());
    print_studio_url("webview-url", page);
    smoke_line(&host_family::desktop_capabilities().to_string());
    let launch =
        host_family::open_http_launch(host_family::current_family(), "https://example.com/guide");
    smoke_line(&format!(
        "open_http_launch {} {}",
        launch.program,
        launch.args.join(" ")
    ));
    smoke_line("cancelled");
    if !smoke_display_available() {
        smoke_line("skipped-no-display");
        return;
    }
    let page = page.to_owned();
    let app = tauri::Builder::default()
        .setup(move |app| {
            let parsed = page.parse::<url::Url>()?;
            WebviewWindowBuilder::new(app, "main", WebviewUrl::External(parsed))
                .title("Studio")
                .inner_size(960.0, 640.0)
                .visible(true)
                .build()?;
            Ok(())
        })
        .build(studio_context())
        .unwrap_or_else(|error| {
            eprintln!("smoke-window {error}");
            std::process::exit(1);
        });
    let handle = app.handle().clone();
    std::thread::spawn(move || smoke_wait_until_visible(handle));
    app.run(|_app, _event| {});
}

fn studio_invoke_handler() -> impl Fn(tauri::ipc::Invoke<tauri::Wry>) -> bool + Send + Sync + 'static
{
    tauri::generate_handler![
        preferences::preference_read,
        preferences::preference_commit,
        log_to_host,
        close_aix_applet_window,
        protect_aix_applet_window,
        set_main_traffic_lights_hidden,
        set_window_drag_regions,
        open_workbench_applet_window,
        deliver_workbench_review,
        office_tool,
        document_result_handoff::stage_document_result_save,
        document_result_handoff::take_document_result_save,
        document_result_handoff::finish_document_result_save,
        document_result_handoff::document_result_save_status,
        stage_pdf_tool_handoff,
        take_pdf_tool_handoff,
        drop_pdf_tool_handoff,
        open_file_editor_window,
        open_agent_wiki_window,
        open_aix_applet_window,
        take_aix_applet_home_seed,
        report_aix_applet_home_deck,
        read_aix_applet_home_deck,
        data_sqlite::data_sqlite_query,
        okf_compile::compile_personal_okf,
        okf_compile::accept_personal_okf,
        okf_compile::plan_personal_okf,
        okf_compile::pin_personal_okf,
        okf_compile::release_personal_okf,
        okf_compile::remember_personal_okf,
        okf_compile::compile_workspace_okf,
        okf_compile::search_workspace_okf,
        prepare_message_file_preview,
        open_mail_compose_window,
        publish_mail_compose_result,
        open_notice_window,
        workspace_terminal::workspace_terminal_create,
        code_git::code_worktree_add,
        code_git::code_branches,
        code_git::code_pull_request,
        code_git::git_worktrees,
        workspace_terminal::workspace_terminal_drain,
        workspace_terminal::workspace_terminal_write,
        workspace_terminal::workspace_terminal_resize,
        workspace_terminal::workspace_terminal_kill,
        workspace_browser::launch_workspace_browser,
        workspace_browser::launch_notice_browser,
        workspace_browser::workspace_browser_navigate,
        quark_suggestions::quark_search_suggestions,
        binary_tools::binary_ipc,
        screenshot::capture_interactive_screenshot,
        message_attention::set_message_attention,
        message_attention::publish_message_notification,
        message_attention::withdraw_message_notification,
        screenshot::open_screen_capture_settings,
        app_updater::app_updater_check,
        app_updater::app_updater_install,
        app_updater::app_updater_notes
    ]
}

pub fn run() {
    if std::env::var("A3S_STUDIO_DESKTOP_SMOKE").ok().as_deref() == Some("1") {
        run_desktop_smoke();
        return;
    }
    #[cfg(target_os = "macos")]
    macos_app::relaunch_into_tenant_bundle();
    let tenant_slug = tenant::load()
        .map(|brand| brand.slug)
        .unwrap_or_else(|error| {
            eprintln!("just studio: {error}");
            std::process::exit(1);
        });
    let fork_request = tenant::instance_name()
        .unwrap_or_else(|error| {
            eprintln!("just studio: {error}");
            std::process::exit(1);
        })
        .is_some_and(|name| tenant::is_fork_request(Some(&name)));
    let (_storage_slug, instance_lock) = if fork_request {
        claim_next_fork(&tenant_slug)
    } else {
        let storage_slug = tenant::storage_slug(&tenant_slug).unwrap_or_else(|error| {
            eprintln!("just studio: {error}");
            std::process::exit(1);
        });
        let allocating_fork = std::env::var("A3S_STUDIO_FORK_ALLOC").ok().as_deref() == Some("1");
        let claimed = if allocating_fork {
            single_instance::claim_quiet(&storage_slug)
        } else {
            single_instance::claim(&storage_slug)
        };
        let instance_lock = match claimed {
            single_instance::Claim::Owned(lock) => lock,
            single_instance::Claim::Busy => {
                std::process::exit(if allocating_fork {
                    single_instance::FORK_CLAIM_LOST
                } else {
                    0
                });
            }
            single_instance::Claim::Unavailable(error) => {
                eprintln!("just studio: single-instance lock for {storage_slug}: {error}");
                std::process::exit(1);
            }
        };
        (storage_slug, instance_lock)
    };
    let authority = StudioAuthority::start().unwrap_or_else(|error| {
        eprintln!("just studio: {error}");
        std::process::exit(1);
    });
    let runtime_file = match RuntimeFile::create(&authority) {
        Ok(file) => file,
        Err(error) => {
            eprintln!("just studio: {error}");
            std::process::exit(1);
        }
    };
    let api_base_url = authority.api_base_url();
    let token = authority.token().to_owned();
    let app_name = authority.product().to_owned();
    let slug = authority.slug().to_owned();
    let session_slug = tenant::storage_slug(&slug).unwrap_or_else(|error| {
        eprintln!("just studio: {error}");
        std::process::exit(1);
    });
    let logo_path = authority.logo_path().to_owned();
    let dock_name = app_name.clone();
    let theme_light = authority.theme_light();
    let applet_access = authority.applet_access();
    let preference_member = authority.preference_member_session();
    let app = tauri::Builder::default()
        .invoke_handler(studio_invoke_handler())
        .setup(move |app| {
            #[cfg(target_os = "macos")]
            macos_app::register_bundle_with_window_server();
            let preference_root = match crate::tenant::profile_dir(&session_slug) {
                Some(dir) => dir,
                None => app.path().app_data_dir()?.join(&session_slug),
            };
            app.manage(preferences::PreferenceHost::new(
                preference_root.join("preferences"),
                preference_member,
                token.clone(),
            ));
            // Knowledge and the task form ship with the app. Link each when that name is still free.
            let _ = crate::capabilities::prepare_builtin_skill_at_home("stc-answer");
            let _ = crate::capabilities::prepare_builtin_skill_at_home("ask-form");
            crate::capabilities::retire_removed_builtin_skills();
            app.manage(workspace_terminal::WorkspaceTerminalState::default());
            app.handle().set_theme(Some(Theme::Light));
            applet_window::remember_host(
                app.handle(),
                &slug,
                &app_name,
                &api_base_url,
                &token,
                applet_access,
            );
            create_main_window(
                app,
                &api_base_url,
                &token,
                &slug,
                &session_slug,
                &app_name,
                &logo_path,
                theme_light,
            )?;
            desktop_settings::attach(app.handle().clone());
            Ok(())
        })
        .on_window_event(|window, event| {
            if aix_applet_window::is_aix_applet_label(window.label())
                && aix_applet_window::protects_recovery(window.label())
            {
                if let WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    if let Some(webview) = window.app_handle().get_webview_window(window.label()) {
                        if let Err(error) =
                            webview.eval("window.dispatchEvent(new Event('a3s:aix-editor-close'))")
                        {
                            log_to_host(
                                "error".into(),
                                "AIX close request failed".into(),
                                serde_json::json!({"module":"AixAppletWindow"}),
                                serde_json::json!({"reason":error.to_string()}),
                            );
                        }
                    }
                }
            }
            if aix_applet_window::is_aix_applet_label(window.label())
                && matches!(event, WindowEvent::Destroyed)
            {
                if let Err(error) = aix_applet_window::retire_home_window(window.label()) {
                    log_to_host(
                        "error".to_owned(),
                        "AIX window cleanup failed".to_owned(),
                        serde_json::json!({ "module": "AixAppletWindow" }),
                        serde_json::json!({ "reason": error }),
                    );
                }
            }
            // AppKit puts the buttons back on its own baseline after resize,
            // scale, and focus. Place them again from the content top so the
            // offset does not accumulate.
            #[cfg(target_os = "macos")]
            if window.label() == "main"
                && matches!(
                    event,
                    WindowEvent::Resized(_)
                        | WindowEvent::ScaleFactorChanged { .. }
                        | WindowEvent::Focused(true)
                )
            {
                if let Some(webview) = window.app_handle().get_webview_window("main") {
                    schedule_macos_traffic_lights(webview);
                }
            }
            // AppKit puts the hidden overlay buttons back after resize and focus.
            // Hide now, then again after AppKit's own deferred layout.
            #[cfg(target_os = "macos")]
            if (applet_window::is_applet_label(window.label())
                || file_editor_window::is_file_editor_label(window.label())
                || agent_wiki_window::is_agent_wiki_label(window.label())
                || aix_applet_window::is_aix_applet_label(window.label())
                || mail_compose_window::is_mail_compose_label(window.label()))
                && matches!(
                    event,
                    WindowEvent::Resized(_)
                        | WindowEvent::ScaleFactorChanged { .. }
                        | WindowEvent::Focused(true)
                )
            {
                if let Some(webview) = window.app_handle().get_webview_window(window.label()) {
                    applet_window::hide_applet_traffic_lights(&webview);
                    applet_window::schedule_hide_applet_traffic_lights(webview);
                }
            }
            if applet_window::is_applet_label(window.label()) {
                if let WindowEvent::Destroyed = event {
                    if let Some(host) = window
                        .app_handle()
                        .try_state::<applet_window::AppletWindowHost>()
                    {
                        host.revoke(window.label());
                    }
                }
            }
            if window.label() == "main" {
                if let WindowEvent::CloseRequested { api, .. } = event {
                    if desktop_settings::hide_on_close() {
                        api.prevent_close();
                        let _ = window.hide();
                    } else {
                        window.app_handle().exit(0);
                    }
                }
            }
        })
        .build(studio_context())
        .unwrap_or_else(|error| {
            eprintln!("just studio: {error}");
            std::process::exit(1);
        });
    let activate_handle = app.handle().clone();
    instance_lock.listen(move || {
        let handle = activate_handle.clone();
        let _ = activate_handle.run_on_main_thread(move || desktop_settings::show_main(&handle));
    });
    app.run(move |_app, event| {
        if let RunEvent::Ready = event {
            // AppKit applies the embedded Cherry icon when the run loop starts.
            // Clearing it restores the bundle icon, which the Dock masks.
            #[cfg(target_os = "macos")]
            present_macos_identity(&dock_name);
        }
    });
    drop(runtime_file);
    drop(authority);
}

/// The main window keeps its traffic lights. A page that covers the whole
/// window asks for them to be hidden, then shown again when that page closes.
#[tauri::command]
fn set_main_traffic_lights_hidden(window: tauri::Window, hidden: bool) {
    #[cfg(target_os = "macos")]
    {
        if window.label() != "main" {
            return;
        }
        MAIN_TRAFFIC_SUPPRESSED.store(hidden, Ordering::Release);
        let Some(webview) = window.app_handle().get_webview_window("main") else {
            return;
        };
        if hidden {
            hide_main_traffic_buttons(&webview);
        } else {
            show_main_traffic_buttons(&webview);
        }
        schedule_macos_traffic_lights(webview);
    }
    #[cfg(not(target_os = "macos"))]
    let _ = (window, hidden);
}

/// One renderer log line. The page calls this instead of an Electron IPC bridge.
#[tauri::command]
fn log_to_host(level: String, message: String, source: serde_json::Value, data: serde_json::Value) {
    let source = serde_json::to_string(&source).unwrap_or_else(|_| "{}".to_owned());
    let data = serde_json::to_string(&data).unwrap_or_else(|_| "null".to_owned());
    eprintln!("studio-host level={level} message={message} source={source} data={data}");
}

/// Window-base origins for the three system traffic lights.
/// `y` is the bottom of each button. The buttons keep AppKit's own size.
/// A 4px gap and a 15px top inset center that native cluster in the 60px rail.
fn traffic_light_window_origins(
    content_left: f64,
    content_top: f64,
    button_width: f64,
    button_height: f64,
) -> [(f64, f64); 3] {
    const RAIL_WIDTH: f64 = 60.0;
    const GAP: f64 = 4.0;
    const TOP: f64 = 15.0;
    let cluster = button_width * 3.0 + GAP * 2.0;
    let start = content_left + (RAIL_WIDTH - cluster) / 2.0;
    let y = content_top - TOP - button_height;
    let step = button_width + GAP;
    [(start, y), (start + step, y), (start + step * 2.0, y)]
}

pub(crate) fn packaged_shell() -> bool {
    !cfg!(debug_assertions)
}

/// Document path passed to `WebviewUrl::App` for a signed-in packaged window.
pub(crate) const PACKAGED_MAIN_DOCUMENT: &str = "windows/main/index.html";

pub(crate) fn main_renderer_url(packaged: bool) -> &'static str {
    if packaged {
        "tauri://localhost/windows/main/index.html"
    } else {
        static URL: std::sync::OnceLock<String> = std::sync::OnceLock::new();
        URL.get_or_init(|| {
            format!(
                "{}/windows/main/index.html",
                crate::boot_contract::debug_renderer_origin()
            )
        })
        .as_str()
    }
}

fn webview_url_label(url: &WebviewUrl) -> String {
    match url {
        WebviewUrl::App(path) => path.display().to_string(),
        WebviewUrl::External(url) => url.as_str().to_owned(),
        WebviewUrl::CustomProtocol(url) => url.as_str().to_owned(),
        _ => String::new(),
    }
}

fn print_studio_url(kind: &str, url: &str) {
    println!("studio-{kind} {url}");
    eprintln!("studio-{kind} {url}");
    let _ = std::io::Write::flush(&mut std::io::stdout());
    let _ = std::io::Write::flush(&mut std::io::stderr());
}

/// A saved member session opens the main renderer immediately. The shell is a
/// new loopback origin on every launch, so loading it first paints the login
/// card before the session file can be read.
pub(crate) fn user_home() -> String {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_default()
}

fn initial_window_url(api_base_url: &str, signed_in: bool, packaged: bool) -> String {
    window_url(api_base_url, signed_in, packaged, false)
}

fn window_url(api_base_url: &str, signed_in: bool, packaged: bool, slides: bool) -> String {
    if signed_in {
        main_renderer_url(packaged).to_owned()
    } else if slides {
        format!("{}/slides", api_base_url.trim_end_matches('/'))
    } else {
        format!("{api_base_url}/")
    }
}

fn create_main_window(
    app: &tauri::App,
    api_base_url: &str,
    token: &str,
    slug: &str,
    session_slug: &str,
    app_name: &str,
    logo_path: &Path,
    theme_light: (u8, u8, u8),
) -> Result<(), Box<dyn std::error::Error>> {
    let runtime = serde_json::json!({
        "apiBaseUrl": api_base_url,
        "token": token,
        "homePath": user_home(),
    });
    let initialization_script = boot_initialization_script(
        !cfg!(target_os = "windows"),
        cfg!(target_os = "macos"),
        slug,
        app_name,
        &runtime,
        cfg!(target_os = "macos"),
    );
    let signed_in = member_session::MemberSessionStore::open(session_slug).has_session();
    let resume_script = main_renderer_resume_script(&runtime, slug, app_name);
    let packaged = packaged_shell();
    let slides = !signed_in
        && authority::presentation_deck_page(
            std::env::var("A3S_PRESENTATION_DECK").ok().as_deref(),
        )
        .is_some();
    let chosen = if slides {
        window_url(api_base_url, false, packaged, true)
    } else {
        initial_window_url(api_base_url, signed_in, packaged)
    };
    let webview_url = if signed_in && packaged {
        WebviewUrl::App(PathBuf::from(PACKAGED_MAIN_DOCUMENT))
    } else {
        WebviewUrl::External(chosen.parse()?)
    };
    print_studio_url("webview-url", &webview_url_label(&webview_url));
    let (red, green, blue) = theme_light;
    // A saved session must not paint the shell. The window stays hidden until
    // the committed page is the main renderer.
    let shell_origin = api_base_url.trim_end_matches('/').to_owned();
    let builder = WebviewWindowBuilder::new(app, "main", webview_url)
        .initialization_script(initialization_script)
        .title(app_name)
        .theme(Some(Theme::Light))
        .inner_size(1440.0, 920.0)
        .min_inner_size(960.0, 640.0)
        .resizable(true)
        .visible(!signed_in)
        .background_color(tauri::webview::Color(red, green, blue, 255))
        // The default handler accepts the drop and does not call through to the
        // web view, so the homepage and message composers never receive the file.
        .disable_drag_drop_handler();
    #[cfg(target_os = "macos")]
    let builder = builder
        .hidden_title(true)
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .accept_first_mouse(true)
        .on_page_load(move |window, payload| {
            // Login navigation needs guarded runtime and drag restoration;
            // document-start injection does not run again on that transition.
            if payload.event() != tauri::webview::PageLoadEvent::Finished {
                return;
            }
            print_studio_url("page-load", payload.url().as_str());
            let loaded = payload.url().as_str();
            let main_renderer = is_main_renderer_document(loaded);
            if loaded.starts_with(&shell_origin) || !main_renderer {
                return;
            }
            let _ = window.eval(&resume_script);
            if desktop_settings::take_launch_hidden() {
                schedule_macos_traffic_lights(window.clone());
                return;
            }
            let _ = window.show();
            let _ = window.set_focus();
            if objc2::MainThreadMarker::new().is_some() {
                macos_app::publish_accessibility_windows();
            } else {
                let _ = window.run_on_main_thread(|| {
                    macos_app::publish_accessibility_windows();
                });
            }
            schedule_macos_traffic_lights(window.clone());
        });
    let window = builder.build()?;
    #[cfg(target_os = "macos")]
    window_drag::install(&window);
    install_app_menu(app, app_name, logo_path)?;
    #[cfg(target_os = "macos")]
    present_macos_identity(app_name);
    #[cfg(not(target_os = "macos"))]
    apply_app_icon(&window, logo_path)?;
    if !signed_in {
        window.show()?;
        window.set_focus()?;
        #[cfg(target_os = "macos")]
        macos_app::publish_accessibility_windows();
    }
    #[cfg(target_os = "macos")]
    {
        place_macos_traffic_lights(&window);
        schedule_macos_traffic_lights(window.clone());
    }
    Ok(())
}

#[cfg(target_os = "macos")]
static TRAFFIC_LIGHT_PLACEMENT_QUEUED: AtomicBool = AtomicBool::new(false);

/// Set while a full-page overlay in the main window is open.
#[cfg(target_os = "macos")]
static MAIN_TRAFFIC_SUPPRESSED: AtomicBool = AtomicBool::new(false);

/// The suppress path gets two deferred passes. This marks the second one.
#[cfg(target_os = "macos")]
static MAIN_TRAFFIC_EXTRA_HIDE: AtomicBool = AtomicBool::new(false);

#[cfg(target_os = "macos")]
#[link(name = "System")]
unsafe extern "C" {
    // `dispatch_get_main_queue()` is a header macro for the address of this queue.
    static _dispatch_main_q: u8;
    fn dispatch_async_f(
        queue: *mut std::ffi::c_void,
        context: *mut std::ffi::c_void,
        work: unsafe extern "C" fn(*mut std::ffi::c_void),
    );
}

#[cfg(target_os = "macos")]
unsafe extern "C" fn dispatch_place_traffic_lights(context: *mut std::ffi::c_void) {
    TRAFFIC_LIGHT_PLACEMENT_QUEUED.store(false, Ordering::Release);
    if context.is_null() {
        return;
    }
    let window = unsafe { Box::from_raw(context.cast::<tauri::WebviewWindow>()) };
    if MAIN_TRAFFIC_SUPPRESSED.load(Ordering::Acquire) {
        hide_main_traffic_buttons(&window);
        if MAIN_TRAFFIC_EXTRA_HIDE.swap(true, Ordering::AcqRel) {
            MAIN_TRAFFIC_EXTRA_HIDE.store(false, Ordering::Release);
            return;
        }
        schedule_macos_traffic_lights(window.as_ref().clone());
        return;
    }
    MAIN_TRAFFIC_EXTRA_HIDE.store(false, Ordering::Release);
    place_macos_traffic_lights(&window);
}

#[cfg(target_os = "macos")]
fn hide_main_traffic_buttons(window: &tauri::WebviewWindow) {
    set_main_traffic_buttons_visible(window, false);
}

#[cfg(target_os = "macos")]
fn show_main_traffic_buttons(window: &tauri::WebviewWindow) {
    set_main_traffic_buttons_visible(window, true);
}

#[cfg(target_os = "macos")]
fn set_main_traffic_buttons_visible(window: &tauri::WebviewWindow, visible: bool) {
    use objc2_app_kit::{NSWindow, NSWindowButton};

    let Ok(native) = window.ns_window() else {
        return;
    };
    let native = unsafe { &*native.cast::<NSWindow>() };
    for kind in [
        NSWindowButton::CloseButton,
        NSWindowButton::MiniaturizeButton,
        NSWindowButton::ZoomButton,
    ] {
        if let Some(button) = native.standardWindowButton(kind) {
            button.setHidden(!visible);
            button.setAlphaValue(if visible { 1.0 } else { 0.0 });
        }
    }
}

/// AppKit lays the buttons out again after the resize event. One deferred
/// pass on the main queue runs after that layout and reads the live frames.
#[cfg(target_os = "macos")]
fn schedule_macos_traffic_lights(window: tauri::WebviewWindow) {
    if TRAFFIC_LIGHT_PLACEMENT_QUEUED.swap(true, Ordering::AcqRel) {
        return;
    }
    let context = Box::into_raw(Box::new(window)).cast::<std::ffi::c_void>();
    unsafe {
        let queue = std::ptr::addr_of!(_dispatch_main_q).cast_mut().cast();
        dispatch_async_f(queue, context, dispatch_place_traffic_lights);
    }
}

/// Close, minimize, and zoom stay at AppKit's size and are centered in the
/// 60px icon rail. Vertical position is recomputed from the content top on
/// every call so a resize cannot walk the buttons downward.
#[cfg(target_os = "macos")]
fn place_macos_traffic_lights(window: &tauri::WebviewWindow) {
    use objc2_app_kit::{NSWindow, NSWindowButton};
    use objc2_foundation::NSPoint;

    if window.is_fullscreen().unwrap_or(false) {
        return;
    }
    let Ok(native) = window.ns_window() else {
        return;
    };
    let native = unsafe { &*native.cast::<NSWindow>() };
    let Some(content) = native.contentView() else {
        return;
    };
    let content_frame = content.frame();
    let content_left = content_frame.origin.x;
    let content_top = content_frame.origin.y + content_frame.size.height;
    let kinds = [
        NSWindowButton::CloseButton,
        NSWindowButton::MiniaturizeButton,
        NSWindowButton::ZoomButton,
    ];
    let mut placed = Vec::new();
    for kind in kinds {
        let Some(button) = native.standardWindowButton(kind) else {
            continue;
        };
        // Leave the system button's size alone. Scaling its unit square
        // enlarges the bounds, and the next layout copies that into a wider frame.
        let window_rect = button.convertRect_toView(button.bounds(), None);
        if window_rect.size.width < 1.0 || window_rect.size.height < 1.0 {
            continue;
        }
        placed.push((button, window_rect.size.width, window_rect.size.height));
    }
    let Some((_, button_width, button_height)) = placed.first() else {
        return;
    };
    let origins =
        traffic_light_window_origins(content_left, content_top, *button_width, *button_height);
    for (index, (button, _, _)) in placed.into_iter().enumerate() {
        let Some((x, y)) = origins.get(index).copied() else {
            continue;
        };
        // The button stays in the hierarchy for this call, so its superview
        // remains alive while the origin is converted and assigned.
        let Some(superview) = (unsafe { button.superview() }) else {
            continue;
        };
        let origin = superview.convertPoint_fromView(NSPoint::new(x, y), None);
        button.setFrameOrigin(origin);
    }
}

fn install_app_menu(app: &tauri::App, name: &str, logo_path: &Path) -> tauri::Result<()> {
    let handle = app.handle();
    let labels =
        desktop_settings::menu_labels::MenuLabels::load(&desktop_settings::menu_language())?;
    let icon = tauri::image::Image::from_path(logo_path).ok();
    let about = PredefinedMenuItem::about(
        handle,
        Some(&labels.about.replace("{{product}}", name)),
        Some(AboutMetadata {
            name: Some(name.to_owned()),
            icon,
            ..Default::default()
        }),
    )?;
    let separator = PredefinedMenuItem::separator(handle)?;
    let quit = PredefinedMenuItem::quit(handle, Some(&labels.quit.replace("{{product}}", name)))?;
    #[cfg(target_os = "macos")]
    let app_menu = {
        let hide =
            PredefinedMenuItem::hide(handle, Some(&labels.hide.replace("{{product}}", name)))?;
        let hide_others = PredefinedMenuItem::hide_others(handle, Some(&labels.hide_others))?;
        let separator_b = PredefinedMenuItem::separator(handle)?;
        Submenu::with_items(
            handle,
            name,
            true,
            &[&about, &separator, &hide, &hide_others, &separator_b, &quit],
        )?
    };
    #[cfg(not(target_os = "macos"))]
    let app_menu = Submenu::with_items(handle, name, true, &[&about, &separator, &quit])?;
    let undo = PredefinedMenuItem::undo(handle, Some(&labels.undo))?;
    let redo = PredefinedMenuItem::redo(handle, Some(&labels.redo))?;
    let separator_c = PredefinedMenuItem::separator(handle)?;
    let cut = PredefinedMenuItem::cut(handle, Some(&labels.cut))?;
    let copy = PredefinedMenuItem::copy(handle, Some(&labels.copy))?;
    let paste = PredefinedMenuItem::paste(handle, Some(&labels.paste))?;
    let select_all = PredefinedMenuItem::select_all(handle, Some(&labels.select_all))?;
    let edit = Submenu::with_items(
        handle,
        &labels.edit,
        true,
        &[&undo, &redo, &separator_c, &cut, &copy, &paste, &select_all],
    )?;
    let menu = Menu::with_items(handle, &[&app_menu, &edit])?;
    app.handle().set_menu(menu)?;
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn apply_app_icon(
    window: &tauri::WebviewWindow,
    logo_path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let image = tauri::image::Image::from_path(logo_path)?;
    window.set_icon(image)?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn present_macos_identity(name: &str) {
    // setApplicationIconImage draws the bitmap raw. The square light-surface
    // plate then fills the Dock slot with sharp corners. nil uses the bundle icon,
    // and the Dock applies the same squircle mask as every other app.
    clear_macos_dock_icon_override();
    set_macos_dock_name(name);
}

#[cfg(target_os = "macos")]
fn set_macos_dock_name(name: &str) {
    use objc2_foundation::{NSProcessInfo, NSString};

    // The Dock label comes from the app bundle's CFBundleDisplayName.
    // CPSSetProcessName faults inside SkyLight when handed this string.
    let label = NSString::from_str(name);
    NSProcessInfo::processInfo().setProcessName(&label);
}

#[cfg(target_os = "macos")]
fn clear_macos_dock_icon_override() {
    use objc2::MainThreadMarker;
    use objc2_app_kit::NSApplication;

    let Some(marker) = MainThreadMarker::new() else {
        return;
    };
    let application = NSApplication::sharedApplication(marker);
    unsafe { application.setApplicationIconImage(None) };
}

struct RuntimeFile {
    path: PathBuf,
}

impl RuntimeFile {
    fn create(authority: &StudioAuthority) -> io::Result<Self> {
        let path = runtime_path();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)?;
            }
        }
        let mut bytes = serde_json::to_vec(&serde_json::json!({
            "apiBaseUrl": authority.api_base_url(),
            "token": authority.token(),
            "homePath": user_home(),
            "pid": std::process::id(),
        }))
        .map_err(|err| io::Error::other(err))?;
        bytes.push(b'\n');
        let temporary = path.with_extension("tmp");
        fs::write(&temporary, &bytes)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&temporary, fs::Permissions::from_mode(0o600))?;
        }
        fs::rename(&temporary, &path)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
        }
        arm_runtime_cleanup(&path);
        Ok(Self { path })
    }
}

impl Drop for RuntimeFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

#[cfg(unix)]
static RUNTIME_FILE: OnceLock<std::ffi::CString> = OnceLock::new();

fn arm_runtime_cleanup(path: &Path) {
    #[cfg(unix)]
    {
        let Some(text) = path.to_str() else {
            return;
        };
        let Ok(bytes) = std::ffi::CString::new(text) else {
            return;
        };
        let _ = RUNTIME_FILE.set(bytes);
        unsafe {
            signal(2, shutdown_signal as *const ());
            signal(15, shutdown_signal as *const ());
        }
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
}

#[cfg(unix)]
extern "C" fn shutdown_signal(signal_number: i32) {
    if let Some(path) = RUNTIME_FILE.get() {
        unsafe {
            unlink(path.as_ptr());
        }
    }
    unsafe {
        signal(signal_number, std::ptr::null());
        raise(signal_number);
    }
}

#[cfg(unix)]
unsafe extern "C" {
    fn signal(sig: i32, handler: *const ()) -> *const ();
    fn raise(sig: i32) -> i32;
    fn unlink(path: *const i8) -> i32;
}

fn studio_context<R: tauri::Runtime>() -> tauri::Context<R> {
    tauri::generate_context!()
}

fn runtime_path() -> PathBuf {
    std::env::var("STUDIO_RUNTIME_FILE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir().join("a3s-studio-runtime.json"))
}

#[cfg(test)]
mod embed_capability {
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    use tauri::Manager;
    use tauri::ipc::Origin;

    fn authority_from_shipped_capability_file() -> tauri::ipc::RuntimeAuthority {
        let capability_path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("capabilities/default.json");
        let capability_bytes = std::fs::read(&capability_path).unwrap_or_else(|error| {
            panic!(
                "read shipped capability {}: {error}",
                capability_path.display()
            )
        });
        let file: tauri::utils::acl::capability::CapabilityFile =
            serde_json::from_slice(&capability_bytes).unwrap_or_else(|error| {
                panic!(
                    "parse shipped capability {}: {error}",
                    capability_path.display()
                )
            });
        let mut capabilities = BTreeMap::new();
        match file {
            tauri::utils::acl::capability::CapabilityFile::Capability(capability) => {
                assert!(!capability.local, "loopback page is remote");
                let remote = capability
                    .remote
                    .as_ref()
                    .expect("shipped capability remote.urls");
                assert_eq!(remote.urls, ["http://127.0.0.1:*"]);
                capabilities.insert(capability.identifier.clone(), capability);
            }
            tauri::utils::acl::capability::CapabilityFile::List(_)
            | tauri::utils::acl::capability::CapabilityFile::NamedList { .. } => {
                panic!("capabilities/default.json must be the single main capability");
            }
        }

        let manifest_path =
            PathBuf::from(env!("OUT_DIR")).join(tauri::utils::acl::ACL_MANIFESTS_FILE_NAME);
        let manifest_bytes = std::fs::read(&manifest_path).unwrap_or_else(|error| {
            panic!(
                "read plugin acl manifests {}: {error}",
                manifest_path.display()
            )
        });
        let manifests: BTreeMap<String, tauri::utils::acl::manifest::Manifest> =
            serde_json::from_slice(&manifest_bytes).unwrap_or_else(|error| {
                panic!(
                    "parse plugin acl manifests {}: {error}",
                    manifest_path.display()
                )
            });
        let create_webview = manifests
            .get("core:webview")
            .and_then(|manifest| manifest.permissions.get("allow-create-webview"))
            .expect("core:webview allow-create-webview from the shipped acl manifests");
        assert_eq!(create_webview.commands.allow.as_slice(), ["create_webview"]);

        let resolved = tauri::utils::acl::resolved::Resolved::resolve(
            &manifests,
            capabilities,
            tauri::utils::platform::Target::current(),
        )
        .unwrap_or_else(|error| panic!("resolve shipped capability: {error}"));

        #[cfg(debug_assertions)]
        {
            tauri::ipc::RuntimeAuthority::new(manifests, resolved)
        }
        #[cfg(not(debug_assertions))]
        {
            let _ = manifests;
            tauri::ipc::RuntimeAuthority::new(resolved)
        }
    }

    fn allows(page: &str) -> bool {
        let mut context = super::studio_context::<tauri::test::MockRuntime>();
        let authority = context.runtime_authority_mut();
        authority
            .resolve_access(
                "plugin:webview|create_webview",
                "main",
                "main",
                &Origin::Remote {
                    url: page.parse().expect("page url"),
                },
            )
            .is_some()
    }

    #[test]
    fn applet_window_can_minimize_on_the_loopback_renderer() {
        let mut context = super::studio_context::<tauri::test::MockRuntime>();
        let authority = context.runtime_authority_mut();
        let page = Origin::Remote {
            url: "http://127.0.0.1:4318/windows/main/index.html"
                .parse()
                .unwrap(),
        };
        assert!(
            authority
                .resolve_access(
                    "plugin:window|minimize",
                    "applet-leave",
                    "applet-leave",
                    &page
                )
                .is_some(),
            "applet windows need native minimize"
        );
        assert!(
            authority
                .resolve_access("plugin:window|close", "applet-leave", "applet-leave", &page)
                .is_some()
        );
        assert!(
            authority
                .resolve_access("open_workbench_applet_window", "main", "main", &page)
                .is_some(),
            "the main window opens applet windows"
        );
        assert!(
            authority
                .resolve_access("plugin:window|minimize", "other", "other", &page)
                .is_none()
        );
        // devUrl http://127.0.0.1:4318 is classified as local, not remote.
        let local = Origin::Local;
        assert!(
            authority
                .resolve_access("open_workbench_applet_window", "main", "main", &local)
                .is_some(),
            "the renderer origin is local and must open applet windows"
        );
        for command in [
            "plugin:window|minimize",
            "plugin:window|toggle_maximize",
            "plugin:window|close",
            "plugin:window|start_dragging",
        ] {
            assert!(
                authority
                    .resolve_access(command, "applet-leave", "applet-leave", &local)
                    .is_some(),
                "{command} on the local applet origin"
            );
        }
        assert!(
            authority
                .resolve_access("plugin:webview|create_webview", "main", "main", &local)
                .is_none(),
            "local renderer access does not grant child webviews"
        );
        for command in [
            "plugin:window|close",
            "plugin:window|set_fullscreen",
            "plugin:window|is_fullscreen",
            "plugin:window|maximize",
            "plugin:window|unmaximize",
            "plugin:window|is_maximized",
            "plugin:webview|set_webview_zoom",
        ] {
            assert!(
                authority
                    .resolve_access(command, "main", "main", &local)
                    .is_some(),
                "{command} on the local main window"
            );
            assert!(
                authority
                    .resolve_access(command, "main", "main", &page)
                    .is_some(),
                "{command} on the loopback shell"
            );
        }
        assert!(
            authority
                .resolve_access(
                    "open_workbench_applet_window",
                    "applet-leave",
                    "applet-leave",
                    &page
                )
                .is_none(),
            "an applet window cannot open another applet window"
        );
        assert!(
            authority
                .resolve_access(
                    "deliver_workbench_review",
                    "applet-review",
                    "applet-review",
                    &page
                )
                .is_some(),
            "the review applet can hand a Word document to the main window"
        );
        assert!(
            authority
                .resolve_access("deliver_workbench_review", "main", "main", &page)
                .is_none(),
            "the main window does not need the applet handoff command"
        );
        assert!(
            authority
                .resolve_access("stage_pdf_tool_handoff", "main", "main", &local)
                .is_some(),
            "the homepage can store a format job"
        );
        assert!(
            authority
                .resolve_access("drop_pdf_tool_handoff", "main", "main", &local)
                .is_some(),
            "the homepage can drop a format job when the window does not open"
        );
        assert!(
            authority
                .resolve_access("take_pdf_tool_handoff", "main", "main", &local)
                .is_none(),
            "the main window does not take the format job"
        );
        assert!(
            authority
                .resolve_access("take_pdf_tool_handoff", "applet-pdf", "applet-pdf", &page)
                .is_some(),
            "the PDF window can take the format job"
        );
        assert!(
            authority
                .resolve_access("stage_pdf_tool_handoff", "applet-pdf", "applet-pdf", &page)
                .is_none(),
            "the PDF window cannot store a new format job"
        );
        assert!(
            authority
                .resolve_access("open_file_editor_window", "main", "main", &local)
                .is_some(),
            "the renderer origin opens file editor windows"
        );
        assert!(
            authority
                .resolve_access("open_agent_wiki_window", "main", "main", &local)
                .is_some(),
            "the renderer origin opens agent encyclopedia windows"
        );
        assert!(
            authority
                .resolve_access(
                    "plugin:window|close",
                    "agent-wiki-0123456789abcdef",
                    "agent-wiki-0123456789abcdef",
                    &local
                )
                .is_some(),
            "an agent encyclopedia window can close itself"
        );
        assert!(
            authority
                .resolve_access("open_aix_applet_window", "main", "main", &local)
                .is_some(),
            "the renderer origin opens AIX mini app windows"
        );
        assert!(
            authority
                .resolve_access("read_aix_applet_home_deck", "main", "main", &local)
                .is_some(),
            "the renderer origin reads whether an AIX mini app still holds a deck"
        );
        for command in [
            "take_aix_applet_home_seed",
            "report_aix_applet_home_deck",
            "protect_aix_applet_window",
            "close_aix_applet_window",
        ] {
            assert!(
                authority
                    .resolve_access(command, "main", "main", &local)
                    .is_none(),
                "the main window cannot {command}"
            );
            assert!(
                authority
                    .resolve_access(
                        command,
                        "aix-applet-0123456789abcdef",
                        "aix-applet-0123456789abcdef",
                        &local
                    )
                    .is_some(),
                "an AIX mini app window can {command}"
            );
        }
        assert!(
            authority
                .resolve_access(
                    "read_aix_applet_home_deck",
                    "aix-applet-0123456789abcdef",
                    "aix-applet-0123456789abcdef",
                    &local
                )
                .is_none(),
            "an AIX mini app window cannot read the homepage deck"
        );
        assert!(
            authority
                .resolve_access(
                    "plugin:window|close",
                    "aix-applet-0123456789abcdef",
                    "aix-applet-0123456789abcdef",
                    &local
                )
                .is_some(),
            "an AIX mini app window can close itself"
        );
        for command in ["open_aix_applet_window", "preference_commit", "office_tool"] {
            assert!(
                authority
                    .resolve_access(
                        command,
                        "aix-applet-0123456789abcdef",
                        "aix-applet-0123456789abcdef",
                        &local
                    )
                    .is_none(),
                "an AIX mini app window cannot call {command}"
            );
        }
        assert!(
            authority
                .resolve_access(
                    "plugin:window|close",
                    "file-editor-note",
                    "file-editor-note",
                    &local
                )
                .is_some(),
            "a file editor window can close itself"
        );
        assert!(
            authority
                .resolve_access(
                    "open_file_editor_window",
                    "file-editor-note",
                    "file-editor-note",
                    &local
                )
                .is_none(),
            "a file editor window cannot open another file editor window"
        );
        assert!(
            authority
                .resolve_access("open_notice_window", "main", "main", &local)
                .is_some(),
            "the renderer origin opens notice windows"
        );
        assert!(
            authority
                .resolve_access(
                    "plugin:window|close",
                    "notice-terms",
                    "notice-terms",
                    &local
                )
                .is_some(),
            "a notice window can close itself"
        );
        assert!(
            authority
                .resolve_access("open_notice_window", "notice-terms", "notice-terms", &local)
                .is_none(),
            "a notice window cannot open another notice window"
        );
        assert!(
            authority
                .resolve_access("open_mail_compose_window", "main", "main", &local)
                .is_some(),
            "the renderer origin opens the mail compose window"
        );
        assert!(
            authority
                .resolve_access(
                    "plugin:window|close",
                    "mail-compose",
                    "mail-compose",
                    &local
                )
                .is_some(),
            "the mail compose window can close itself"
        );
        assert!(
            authority
                .resolve_access(
                    "publish_mail_compose_result",
                    "mail-compose",
                    "mail-compose",
                    &local
                )
                .is_some(),
            "the mail compose window can tell the main window to reload"
        );
        assert!(
            authority
                .resolve_access(
                    "open_mail_compose_window",
                    "mail-compose",
                    "mail-compose",
                    &local
                )
                .is_none(),
            "the mail compose window cannot open another compose window"
        );
        assert!(
            authority
                .resolve_access(
                    "plugin:webview|create_webview",
                    "notice-terms",
                    "notice-terms",
                    &local
                )
                .is_none(),
            "a notice window cannot create a child webview"
        );
        assert!(
            authority
                .resolve_access(
                    "plugin:webview|create_webview",
                    "applet-leave",
                    "applet-leave",
                    &page
                )
                .is_none(),
            "an applet window cannot create a child webview"
        );
        assert!(
            authority
                .resolve_access(
                    "set_window_drag_regions",
                    "applet-leave",
                    "applet-leave",
                    &local
                )
                .is_none(),
            "an applet window cannot call shell commands"
        );
    }

    #[test]
    fn shipped_capability_allows_create_webview_only_on_loopback() {
        let mut context = super::studio_context::<tauri::test::MockRuntime>();
        let authority = context.runtime_authority_mut();
        assert!(
            authority
                .resolve_access(
                    "plugin:webview|create_webview",
                    "main",
                    "main",
                    &Origin::Local,
                )
                .is_none(),
            "the shell page is remote; local context must not grant create_webview"
        );
        for page in [
            "http://127.0.0.1:1/",
            "http://127.0.0.1:62544/",
            "http://127.0.0.1:65535/",
            "http://127.0.0.1:62544/index.html",
        ] {
            assert!(allows(page), "{page}");
        }
        assert!(
            authority
                .resolve_access(
                    "set_window_drag_regions",
                    "main",
                    "main",
                    &Origin::Remote {
                        url: "http://127.0.0.1:62544/".parse().unwrap(),
                    },
                )
                .is_some(),
            "loopback chrome must be allowed to start a window drag"
        );
        for page in [
            "https://example.com/",
            "http://example.com/",
            "https://127.0.0.1:62544/",
            "http://localhost:62544/",
            "http://127.0.0.1.evil.example/",
        ] {
            assert!(!allows(page), "{page}");
        }
        assert!(
            authority
                .resolve_access(
                    "plugin:webview|create_webview",
                    "other",
                    "other",
                    &Origin::Remote {
                        url: "http://127.0.0.1:62544/".parse().unwrap(),
                    },
                )
                .is_none()
        );
    }

    #[test]
    fn preference_read_follows_the_renderer_document() {
        let mut context = super::studio_context::<tauri::test::MockRuntime>();
        let authority = context.runtime_authority_mut();
        let allows = |page: &str, window: &str, webview: &str| {
            authority
                .resolve_access(
                    "preference_read",
                    window,
                    webview,
                    &Origin::Remote {
                        url: page.parse().expect("page url"),
                    },
                )
                .is_some()
        };
        assert!(
            authority
                .resolve_access("preference_read", "main", "main", &Origin::Local)
                .is_some(),
            "the devUrl renderer stays local"
        );
        for page in [
            "http://127.0.0.1:4318/windows/main/index.html",
            "http://127.0.0.1:4541/windows/main/index.html",
            "http://127.0.0.1:1/windows/main/index.html",
            "http://127.0.0.1:65535/windows/main/index.html?boot=1#/",
        ] {
            assert!(allows(page, "main", "main"), "{page}");
            assert!(
                authority
                    .resolve_access(
                        "preference_commit",
                        "main",
                        "main",
                        &Origin::Remote {
                            url: page.parse().expect("page url"),
                        },
                    )
                    .is_some(),
                "{page}"
            );
        }
        for page in [
            "http://127.0.0.1:62544/",
            "http://127.0.0.1:62544/index.html",
            "http://127.0.0.1:4541/",
            "http://127.0.0.1:4541/index.html",
            "http://127.0.0.1:4541/windows/main/other.html",
            "https://127.0.0.1:4541/windows/main/index.html",
            "http://localhost:4541/windows/main/index.html",
            "http://127.0.0.1.evil.example/windows/main/index.html",
        ] {
            assert!(!allows(page, "main", "main"), "{page}");
        }
        assert!(
            !allows(
                "http://127.0.0.1:4541/windows/main/index.html",
                "other",
                "other"
            ),
            "another window does not read preferences"
        );
    }

    #[test]
    fn shipped_file_allows_create_webview_only_from_loopback_page() {
        let authority = authority_from_shipped_capability_file();
        let command = "plugin:webview|create_webview";
        assert!(
            authority
                .resolve_access(command, "main", "main", &Origin::Local)
                .is_none(),
            "the shell page is remote; local context must not grant create_webview"
        );
        for page in [
            "http://127.0.0.1:1/",
            "http://127.0.0.1:62544/",
            "http://127.0.0.1:65535/",
            "http://127.0.0.1:62544/index.html",
        ] {
            assert!(
                authority
                    .resolve_access(
                        command,
                        "main",
                        "main",
                        &Origin::Remote {
                            url: page.parse().expect("page url"),
                        },
                    )
                    .is_some(),
                "{page}"
            );
        }
        for page in [
            "https://example.com/",
            "http://example.com/",
            "https://127.0.0.1:62544/",
            "http://localhost:62544/",
            "http://127.0.0.1.evil.example/",
        ] {
            assert!(
                authority
                    .resolve_access(
                        command,
                        "main",
                        "main",
                        &Origin::Remote {
                            url: page.parse().expect("page url"),
                        },
                    )
                    .is_none(),
                "{page}"
            );
        }
        assert!(
            authority
                .resolve_access(
                    command,
                    "other",
                    "other",
                    &Origin::Remote {
                        url: "http://127.0.0.1:62544/".parse().unwrap(),
                    },
                )
                .is_none()
        );
        let remote = Origin::Remote {
            url: "http://127.0.0.1:62544/".parse().unwrap(),
        };
        assert!(
            authority
                .resolve_access("read_aix_applet_home_deck", "main", "main", &remote)
                .is_some(),
            "the loopback shell can read whether an AIX mini app still holds a deck"
        );
        for command in [
            "take_aix_applet_home_seed",
            "report_aix_applet_home_deck",
            "protect_aix_applet_window",
            "close_aix_applet_window",
        ] {
            assert!(
                authority
                    .resolve_access(command, "main", "main", &remote)
                    .is_none(),
                "the main capability does not grant {command}"
            );
        }
    }

    #[test]
    fn loopback_page_creates_a_same_window_child_webview() {
        let app = tauri::test::mock_builder()
            .build(super::studio_context())
            .expect("studio context");
        let window = tauri::WebviewWindowBuilder::new(
            &app,
            "main",
            tauri::WebviewUrl::External("http://127.0.0.1:62544/".parse().unwrap()),
        )
        .build()
        .expect("main window");

        let denied = invoke(&window, "https://example.com/", "foreign-embed");
        let denied = denied.expect_err("non-loopback page must be rejected");
        let denied = denied.to_string();
        assert!(
            denied.contains("not allowed"),
            "expected an ACL denial, got {denied}"
        );
        assert!(app.get_webview("foreign-embed").is_none());

        invoke(&window, "http://127.0.0.1:62544/", "studio-embed")
            .expect("loopback page creates the child webview");
        let child = app
            .get_webview("studio-embed")
            .expect("studio-embed child webview");
        assert_eq!(child.label(), "studio-embed");
        assert!(
            child
                .url()
                .expect("child url")
                .as_str()
                .starts_with("https://example.com"),
            "child url"
        );
        let host = app.get_window("main").expect("main window");
        let labels: Vec<String> = host
            .webviews()
            .iter()
            .map(|webview| webview.label().to_string())
            .collect();
        assert!(labels.iter().any(|label| label == "main"));
        assert!(
            labels.iter().any(|label| label == "studio-embed"),
            "studio-embed must be a child of the main window, labels={labels:?}"
        );
        assert!(app.get_webview_window("studio-embed").is_none());
    }

    #[test]
    fn places_the_workspace_page_inside_the_main_window() {
        let app = tauri::test::mock_builder()
            .build(super::studio_context())
            .expect("studio context");
        let _window = tauri::WebviewWindowBuilder::new(
            &app,
            "main",
            tauri::WebviewUrl::External("http://127.0.0.1:62544/".parse().unwrap()),
        )
        .build()
        .expect("main window");

        let show = super::workspace_browser::plan_embed(&super::workspace_browser::EmbedRequest {
            session_id: "session-a".to_owned(),
            url: "https://example.com/docs".to_owned(),
            x: 20.0,
            y: 40.0,
            width: 640.0,
            height: 480.0,
            visible: true,
            reload_key: 1,
        })
        .expect("plan");
        super::workspace_browser::place_embed(app.handle(), show).expect("place");
        let child = app
            .get_webview(super::workspace_browser::EMBED_LABEL)
            .expect("studio-embed child");
        assert!(
            child
                .url()
                .expect("child url")
                .as_str()
                .starts_with("https://example.com/docs"),
            "child url"
        );
        let host = app.get_window("main").expect("main window");
        let labels: Vec<String> = host
            .webviews()
            .iter()
            .map(|webview| webview.label().to_string())
            .collect();
        assert!(labels.iter().any(|label| label == "main"));
        assert!(
            labels.iter().any(|label| label == "studio-embed"),
            "studio-embed must be a child of the main window, labels={labels:?}"
        );
        assert!(app.get_webview_window("studio-embed").is_none());

        let next = super::workspace_browser::plan_embed(&super::workspace_browser::EmbedRequest {
            session_id: "session-a".to_owned(),
            url: "https://example.com/next".to_owned(),
            x: 24.0,
            y: 48.0,
            width: 600.0,
            height: 400.0,
            visible: true,
            reload_key: 1,
        })
        .expect("next plan");
        super::workspace_browser::place_embed(app.handle(), next).expect("navigate");
        let child = app.get_webview("studio-embed").expect("the same child");
        assert!(
            child
                .url()
                .expect("next url")
                .as_str()
                .starts_with("https://example.com/next")
        );
        let embed_count = app
            .get_window("main")
            .expect("main window")
            .webviews()
            .iter()
            .filter(|webview| webview.label() == "studio-embed")
            .count();
        assert_eq!(embed_count, 1);
        assert!(app.get_webview_window("studio-embed").is_none());

        assert!(
            super::workspace_browser::plan_embed(&super::workspace_browser::EmbedRequest {
                session_id: "session-a".to_owned(),
                url: "javascript:alert(1)".to_owned(),
                x: 24.0,
                y: 48.0,
                width: 600.0,
                height: 400.0,
                visible: true,
                reload_key: 1,
            })
            .is_err()
        );
        assert!(
            app.get_webview("studio-embed")
                .expect("child remains")
                .url()
                .expect("url")
                .as_str()
                .starts_with("https://example.com/next")
        );

        let hidden =
            super::workspace_browser::plan_embed(&super::workspace_browser::EmbedRequest {
                session_id: "session-a".to_owned(),
                url: "https://example.com/next".to_owned(),
                x: 24.0,
                y: 48.0,
                width: 600.0,
                height: 400.0,
                visible: false,
                reload_key: 1,
            })
            .expect("hide plan");
        assert!(matches!(hidden, super::workspace_browser::EmbedPlan::Hide));
        super::workspace_browser::place_embed(app.handle(), hidden).expect("hide");
        assert!(app.get_webview("studio-embed").is_some());
        assert!(app.get_webview_window("studio-embed").is_none());
    }

    fn invoke(
        window: &tauri::WebviewWindow<tauri::test::MockRuntime>,
        page: &str,
        label: &str,
    ) -> Result<tauri::ipc::InvokeResponseBody, serde_json::Value> {
        tauri::test::get_ipc_response(
            window,
            tauri::webview::InvokeRequest {
                cmd: "plugin:webview|create_webview".into(),
                callback: tauri::ipc::CallbackFn(1),
                error: tauri::ipc::CallbackFn(2),
                url: page.parse().expect("page url"),
                body: tauri::ipc::InvokeBody::Json(serde_json::json!({
                    "windowLabel": "main",
                    "options": {
                        "label": label,
                        "url": "https://example.com",
                        "x": 48,
                        "y": 140,
                        "width": 960,
                        "height": 640
                    }
                })),
                headers: Default::default(),
                invoke_key: tauri::test::INVOKE_KEY.to_string(),
            },
        )
    }
}

#[cfg(test)]
#[path = "boot_script_tests.rs"]
mod boot_script;

#[cfg(test)]
mod initial_window {
    use super::initial_window_url;

    #[test]
    fn signed_in_window_opens_the_main_renderer() {
        let packaged = super::packaged_shell();
        let start = initial_window_url("http://127.0.0.1:54913", true, packaged);
        assert_eq!(start, super::main_renderer_url(packaged));
        let bundled = initial_window_url("http://127.0.0.1:54913", true, true);
        assert_eq!(bundled, super::main_renderer_url(true));
        assert_eq!(
            bundled,
            format!("tauri://localhost/{}", super::PACKAGED_MAIN_DOCUMENT)
        );
        assert_eq!(bundled, "tauri://localhost/windows/main/index.html");
        assert_eq!(
            initial_window_url("http://127.0.0.1:54913", false, packaged),
            "http://127.0.0.1:54913/"
        );
        assert_eq!(
            super::window_url("http://127.0.0.1:54913", false, packaged, true),
            "http://127.0.0.1:54913/slides"
        );
        assert_eq!(
            super::window_url("http://127.0.0.1:54913/", false, packaged, true),
            "http://127.0.0.1:54913/slides"
        );
        assert_eq!(
            initial_window_url("http://127.0.0.1:54913", true, false),
            super::main_renderer_url(false)
        );
        println!("packaged-main-window {bundled}");
        let builder_url =
            tauri::WebviewUrl::App(std::path::PathBuf::from(super::PACKAGED_MAIN_DOCUMENT));
        assert_eq!(
            super::webview_url_label(&builder_url),
            super::PACKAGED_MAIN_DOCUMENT
        );
        println!(
            "studio-webview-url {}",
            super::webview_url_label(&builder_url)
        );
        assert!(super::is_main_renderer_document(
            "tauri://localhost/windows/main/index.html"
        ));
        assert!(super::is_main_renderer_document(
            "http://tauri.localhost/windows/main/index.html"
        ));
        assert!(super::is_main_renderer_document(
            "https://tauri.localhost/windows/main/index.html"
        ));
        assert_eq!(
            super::is_main_renderer_document("http://127.0.0.1:4318/windows/main/index.html"),
            cfg!(debug_assertions)
        );
        assert!(!super::is_main_renderer_document("http://127.0.0.1:54913/"));
    }
}

#[cfg(test)]
mod traffic_lights {
    use super::traffic_light_window_origins;

    #[test]
    fn centers_native_lights_in_the_icon_rail() {
        let origins = traffic_light_window_origins(0.0, 920.0, 14.0, 14.0);
        assert_eq!(origins[0], (5.0, 891.0));
        assert_eq!(origins[1], (23.0, 891.0));
        assert_eq!(origins[2], (41.0, 891.0));
        let cluster_end = origins[2].0 + 14.0;
        assert!((60.0 - cluster_end) - origins[0].0 < 0.01);
    }
}
