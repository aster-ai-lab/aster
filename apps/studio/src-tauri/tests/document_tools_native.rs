#![cfg(test)]

// Compile the actual production modules in this harness-free test crate. The
// Office command and window/ticket composition are not replacement test engines.
// Keeping the host here adds no test launch API to the shipped library.
include!("../src/lib.rs");

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // This is the process main, before any thread starts. A fixed test instance
    // prevents the authority and WKWebView store from using the user's profile.
    unsafe { std::env::set_var("A3S_STUDIO_INSTANCE", "document-tools-e2e") };
    let mut authority = StudioAuthority::start()?;
    let storage = tenant::storage_slug(authority.slug())?;
    let product = authority.product().to_owned();
    let base = authority.api_base_url();
    let token = authority.token().to_owned();
    let access = authority.applet_access();
    let cleanup_access = access.clone();
    let app = tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            log_to_host,
            set_main_traffic_lights_hidden,
            set_window_drag_regions,
            office_tool,
            take_pdf_tool_handoff
        ])
        .setup(move |app| {
            app.handle().set_theme(Some(Theme::Light));
            applet_window::remember_host(app.handle(), &storage, &product, &base, &token, access);
            open_workbench_applet_window(
                app.handle().clone(),
                "pdf".to_owned(),
                "Document tools native test".to_owned(),
                None,
                None,
                None,
                None,
                None,
                Some(Vec::new()),
                Some("word-to-pdf".to_owned()),
            )?;
            Ok(())
        })
        .build(studio_context())?;
    app.run(move |handle, event| {
        if let tauri::RunEvent::WindowEvent {
            label,
            event: tauri::WindowEvent::Destroyed,
            ..
        } = event
        {
            cleanup_access.revoke_label(&label);
            handle.exit(0);
        }
    });
    authority.shutdown();
    Ok(())
}
