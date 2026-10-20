//! System windows for workbench office applets.
//!
//! The frame matches Work's plugin popup: a native overlay window, hidden
//! system traffic lights, and a page that paints close / minimize / zoom.

use std::sync::Arc;
#[cfg(target_os = "macos")]
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine;
use sha2::{Digest, Sha256};
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

use crate::applet_runtime::AppletAccess;

const APPLET_IDS: &[&str] = &[
    "approval",
    "leave",
    "travel",
    "expense",
    "meeting",
    "seal",
    "notice",
    "pdf",
    "inbox",
    "plan",
    "probation",
    "clock",
    "correction",
    "ai-resource",
];

/// Same canvas as Work's applet / file-editor chrome windows.
const APPLET_WINDOW_WIDTH: f64 = 1280.0;
const APPLET_WINDOW_HEIGHT: f64 = 860.0;

const PERMISSIONS: [&str; 3] = ["user", "model", "knowledge"];

struct WindowBounds {
    width: f64,
    height: f64,
    min_width: f64,
    min_height: f64,
    max_width: Option<f64>,
    max_height: Option<f64>,
}

/// A package declaration can replace the shared applet canvas and cap the window.
fn window_bounds(
    _id: &str,
    default_width: Option<f64>,
    default_height: Option<f64>,
    max_width: Option<f64>,
    max_height: Option<f64>,
) -> Result<WindowBounds, String> {
    if let Some(bounds) = declared_bounds(default_width, default_height, max_width, max_height)? {
        return Ok(bounds);
    }
    Ok(WindowBounds {
        width: APPLET_WINDOW_WIDTH,
        height: APPLET_WINDOW_HEIGHT,
        min_width: 720.0,
        min_height: 520.0,
        max_width: None,
        max_height: None,
    })
}

fn declared_bounds(
    default_width: Option<f64>,
    default_height: Option<f64>,
    max_width: Option<f64>,
    max_height: Option<f64>,
) -> Result<Option<WindowBounds>, String> {
    if default_width.is_none()
        && default_height.is_none()
        && max_width.is_none()
        && max_height.is_none()
    {
        return Ok(None);
    }
    let (Some(width), Some(height), Some(max_width), Some(max_height)) =
        (default_width, default_height, max_width, max_height)
    else {
        return Err("Applet window size is incomplete.".to_owned());
    };
    let width = css_length(width, 320.0, 3840.0, "default width")?;
    let height = css_length(height, 400.0, 2160.0, "default height")?;
    let max_width = css_length(max_width, 320.0, 3840.0, "max width")?;
    let max_height = css_length(max_height, 400.0, 2160.0, "max height")?;
    if width > max_width || height > max_height {
        return Err("Applet window default size is larger than the maximum.".to_owned());
    }
    Ok(Some(WindowBounds {
        width,
        height,
        min_width: 320.0_f64.min(width),
        min_height: 400.0_f64.min(height),
        max_width: Some(max_width),
        max_height: Some(max_height),
    }))
}

fn css_length(value: f64, min: f64, max: f64, name: &str) -> Result<f64, String> {
    if !value.is_finite() || value.fract() != 0.0 || value < min || value > max {
        return Err(format!("Applet window {name} is out of range."));
    }
    Ok(value)
}

fn clean_permissions(values: Option<Vec<String>>) -> Result<Vec<String>, String> {
    let Some(values) = values else {
        return Ok(Vec::new());
    };
    let mut permissions = Vec::new();
    for value in values {
        if !PERMISSIONS.contains(&value.as_str()) {
            return Err(format!(
                "Unknown applet permission '{value}'; expected one of: {}.",
                PERMISSIONS.join(", ")
            ));
        }
        if permissions.iter().any(|kept: &String| kept == &value) {
            return Err(format!("Applet permission '{value}' is duplicated."));
        }
        permissions.push(value);
    }
    permissions.sort_by_key(|name| {
        PERMISSIONS
            .iter()
            .position(|item| item == name)
            .unwrap_or(PERMISSIONS.len())
    });
    Ok(permissions)
}

pub struct AppletWindowHost {
    slug: String,
    product: String,
    api_base_url: String,
    shell_token: String,
    access: Arc<AppletAccess>,
}

impl AppletWindowHost {
    pub fn new(
        slug: &str,
        product: &str,
        api_base_url: &str,
        shell_token: &str,
        access: Arc<AppletAccess>,
    ) -> Self {
        Self {
            slug: slug.to_owned(),
            product: product.to_owned(),
            api_base_url: api_base_url.to_owned(),
            shell_token: shell_token.to_owned(),
            access,
        }
    }

    pub fn revoke(&self, label: &str) {
        self.access.revoke_label(label);
    }

    /// Tenant name and the signed-in shell runtime for a sibling system window.
    pub fn boot_context(&self) -> (String, String, serde_json::Value) {
        (
            self.slug.clone(),
            self.product.clone(),
            serde_json::json!({
                "apiBaseUrl": self.api_base_url,
                "token": self.shell_token,
                "homePath": crate::user_home(),
            }),
        )
    }

    /// The page receives an applet ticket. The shell bearer stays in this process.
    fn initialization_script(&self, app_id: &str, ticket: &str, permissions: &[String]) -> String {
        let slug = serde_json::to_string(&self.slug).unwrap_or_else(|_| "\"\"".to_owned());
        let product = serde_json::to_string(&self.product).unwrap_or_else(|_| "\"\"".to_owned());
        let runtime = serde_json::json!({
            "apiBaseUrl": self.api_base_url,
            "appletTicket": ticket,
            "appId": app_id,
            "tenant": self.slug,
            "permissions": permissions,
        });
        format!(
            "document.documentElement.dataset.overlayTitlebar='true';\
             document.documentElement.dataset.workbenchApplet='true';\
             document.documentElement.dataset.tenant={slug};\
             document.documentElement.dataset.a3sProduct={product};\
             (function(){{var runtime=Object.freeze({runtime});try{{\
             Object.defineProperty(globalThis,'__A3S_APPLET_RUNTIME__',{{value:runtime,configurable:false,enumerable:false,writable:false}});\
             }}catch(e){{}}}})();"
        )
    }
}

pub fn data_store_id(slug: &str, app_id: &str) -> [u8; 16] {
    let digest = Sha256::new()
        .chain_update(slug.as_bytes())
        .chain_update([0])
        .chain_update(app_id.as_bytes())
        .finalize();
    let mut id = [0u8; 16];
    id.copy_from_slice(&digest[..16]);
    id
}

pub fn remember_host(
    app: &tauri::AppHandle,
    slug: &str,
    product: &str,
    api_base_url: &str,
    shell_token: &str,
    access: Arc<AppletAccess>,
) {
    app.manage(AppletWindowHost::new(
        slug,
        product,
        api_base_url,
        shell_token,
        access,
    ));
    crate::review_plugin::remember_app(app.clone());
}

pub fn is_applet_label(label: &str) -> bool {
    label
        .strip_prefix("applet-")
        .is_some_and(|id| APPLET_IDS.contains(&id))
}

const PDF_TOOLS: &[&str] = &[
    "extract",
    "rotate",
    "filter",
    "handwriting",
    "watermark",
    "erase",
    "merge",
    "pdf-to-word",
    "pdf-to-excel",
    "pdf-to-image",
    "pdf-to-long",
    "word-to-pdf",
    "word-to-image",
    "excel-to-pdf",
    "excel-to-image",
    "image-to-word",
    "image-to-excel",
    "image-to-pdf",
    "audio",
    "pdf-edit",
    "scan",
];

pub(crate) fn is_pdf_tool(tool: &str) -> bool {
    PDF_TOOLS.contains(&tool)
}

pub fn applet_page_url(
    id: &str,
    channel: &str,
    opened_at: u128,
    permissions: &[String],
    tool: Option<&str>,
) -> Result<String, String> {
    if !APPLET_IDS.contains(&id) {
        return Err("Unknown workbench applet.".to_owned());
    }
    if channel != "release" && channel != "trial" {
        return Err("Unknown applet channel.".to_owned());
    }
    let renderer = crate::main_renderer_url(crate::packaged_shell());
    let mut url = format!(
        "{renderer}?workbench-applet={id}&channel={channel}&open={opened_at}&permissions={}",
        permissions.join(",")
    );
    if let Some(tool) = tool.map(str::trim).filter(|value| !value.is_empty()) {
        if id != "pdf" || !PDF_TOOLS.contains(&tool) {
            return Err(format!("Unknown format tool '{tool}'."));
        }
        url.push_str("&tool=");
        url.push_str(tool);
    }
    Ok(url)
}

fn clean_channel(channel: Option<String>) -> Result<&'static str, String> {
    match channel.as_deref().map(str::trim) {
        None | Some("") | Some("release") => Ok("release"),
        Some("trial") => Ok("trial"),
        _ => Err("Unknown applet channel.".to_owned()),
    }
}

fn normalize_title(title: &str, id: &str) -> String {
    let cleaned: String = title
        .chars()
        .filter(|character| !character.is_control())
        .collect();
    let trimmed = cleaned.trim();
    if trimmed.is_empty() || trimmed.chars().count() > 120 {
        return id.to_owned();
    }
    trimmed.to_owned()
}

fn opened_at() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

const REVIEW_BYTES_LIMIT: usize = 8 * 1024 * 1024;

fn review_file_name(name: &str) -> Result<&str, String> {
    let name = name.trim();
    if name.is_empty()
        || name.len() > 180
        || name.contains('/')
        || name.contains('\\')
        || name.contains('\0')
        || !name.to_ascii_lowercase().ends_with(".docx")
    {
        return Err("Only Word documents can be reviewed.".to_owned());
    }
    Ok(name)
}

/// Hand one explicit source to the main window's review composer.
#[tauri::command]
pub fn deliver_workbench_review(
    app: tauri::AppHandle,
    file_name: Option<String>,
    bytes_base64: Option<String>,
    text: Option<String>,
    task: Option<String>,
) -> Result<(), String> {
    if let Some(text) = text {
        if file_name.is_some() || bytes_base64.is_some() {
            return Err("Choose one review source.".into());
        }
        let source = crate::agent_plane::ReviewTextSource::Text { text };
        let text = source.text().map_err(str::to_owned)?;
        let task = task.as_deref().unwrap_or("").trim();
        if task.is_empty() || task.len() > 4096 || task.contains('\0') {
            return Err("Review instructions are required and must be at most 4096 bytes.".into());
        }
        return dispatch_review(
            &app,
            serde_json::json!({"kind":"text","text":text,"task":task}),
        );
    }
    if task.is_some() {
        return Err("Review text is missing.".into());
    }
    let file_name = file_name.ok_or("A Word document is required.")?;
    let bytes_base64 = bytes_base64.ok_or("A Word document is required.")?;
    let name = review_file_name(&file_name)?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(bytes_base64.trim())
        .map_err(|_| "The Word document could not be read.".to_owned())?;
    if bytes.is_empty() {
        return Err("An empty file cannot be reviewed.".to_owned());
    }
    if bytes.len() > REVIEW_BYTES_LIMIT {
        return Err("The file is over 8 MB and cannot be reviewed.".to_owned());
    }
    let stamp = opened_at();
    let mut path = std::env::temp_dir();
    path.push("a3s-studio-review");
    std::fs::create_dir_all(&path).map_err(|_| "Could not store the Word document.".to_owned())?;
    path.push(format!("{stamp}-{name}"));
    std::fs::write(&path, &bytes).map_err(|_| "Could not store the Word document.".to_owned())?;
    let path = path.to_string_lossy();
    let detail = serde_json::json!({ "path": path, "fileName": name });
    dispatch_review(&app, detail)
}

fn dispatch_review(app: &tauri::AppHandle, detail: serde_json::Value) -> Result<(), String> {
    let script = format!(
        "window.dispatchEvent(new CustomEvent(\"a3s:workbench-review\",{{detail:{detail}}}))"
    );
    let main = app
        .get_webview_window("main")
        .ok_or_else(|| "Main window is not open.".to_owned())?;
    main.eval(script)
        .map_err(|_| "Could not start the review.".to_owned())
}

#[tauri::command]
pub fn open_workbench_applet_window(
    app: tauri::AppHandle,
    applet_id: String,
    title: String,
    channel: Option<String>,
    default_width: Option<f64>,
    default_height: Option<f64>,
    max_width: Option<f64>,
    max_height: Option<f64>,
    permissions: Option<Vec<String>>,
    tool: Option<String>,
) -> Result<(), String> {
    let id = applet_id.trim();
    if !APPLET_IDS.contains(&id) {
        return Err("Unknown workbench applet.".to_owned());
    }
    let channel = clean_channel(channel)?;
    let title = normalize_title(&title, id);
    let permissions = clean_permissions(permissions)?;
    let bounds = window_bounds(id, default_width, default_height, max_width, max_height)?;
    let label = format!("applet-{id}");
    let host = app
        .try_state::<AppletWindowHost>()
        .ok_or_else(|| "Applet window host is missing.".to_owned())?;
    let page = applet_page_url(id, channel, opened_at(), &permissions, tool.as_deref())?;
    let url = page
        .parse::<url::Url>()
        .map_err(|_| "Applet window URL is invalid.".to_owned())?;
    let ticket = host.access.mint(id, &label, &host.shell_token)?;
    let script = host.initialization_script(id, &ticket, &permissions);
    let store_id = data_store_id(&host.slug, id);
    drop(host);

    if let Some(existing) = app.get_webview_window(&label) {
        let _ = existing.set_title(&title);
        if let (Some(max_width), Some(max_height)) = (bounds.max_width, bounds.max_height) {
            let _ = existing.set_max_size(Some(tauri::LogicalSize::new(max_width, max_height)));
        }
        let _ = existing.navigate(url);
        let _ = existing.unminimize();
        reveal(&existing);
        return Ok(());
    }

    let mut builder = crate::tenant::isolate_webview(
        WebviewWindowBuilder::new(&app, &label, WebviewUrl::External(url))
            .initialization_script(script)
            .data_store_identifier(store_id),
        store_id,
    )
    .map_err(|error| error.to_string())?
    .title(title)
    .inner_size(bounds.width, bounds.height)
    .min_inner_size(bounds.min_width, bounds.min_height);
    if let (Some(max_width), Some(max_height)) = (bounds.max_width, bounds.max_height) {
        builder = builder.max_inner_size(max_width, max_height);
    }
    let builder = builder
        .resizable(true)
        .maximizable(true)
        .minimizable(true)
        .closable(true)
        .visible(false)
        .focused(false)
        .center()
        .background_color(tauri::webview::Color(255, 255, 255, 255))
        .on_page_load(|window, payload| {
            if payload.event() != tauri::webview::PageLoadEvent::Finished {
                return;
            }
            if !is_applet_label(window.label()) {
                return;
            }
            reveal(&window);
        });
    #[cfg(target_os = "macos")]
    let builder = builder
        .hidden_title(true)
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .accept_first_mouse(true);
    let window = builder
        .build()
        .map_err(|error| format!("Could not open the applet window: {error}"))?;
    // Show after the native traffic lights are hidden. A load that never
    // finishes would otherwise leave the window invisible.
    reveal(&window);
    Ok(())
}

/// Whether opening an applet may take the keyboard focus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AppletPresent {
    /// This app is already front, so the new window may become key.
    Focus,
    /// Another process is front. Show the window and leave that process front.
    ShowOnly,
}

/// An inactive app must not activate itself to reveal an applet.
pub(crate) fn applet_present(app_active: bool) -> AppletPresent {
    if app_active {
        AppletPresent::Focus
    } else {
        AppletPresent::ShowOnly
    }
}

pub(crate) fn reveal(window: &tauri::WebviewWindow) {
    #[cfg(target_os = "macos")]
    {
        hide_applet_traffic_lights(window);
        match applet_present(app_is_active()) {
            AppletPresent::Focus => {
                present_applet_window(window, true);
                // show() and set_focus() both activate the application.
                // That is only allowed while this app is already front.
                let _ = window.show();
                let _ = window.set_focus();
            }
            AppletPresent::ShowOnly => present_applet_window(window, false),
        }
        crate::macos_app::publish_accessibility_windows();
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = window.show();
        let _ = window.set_focus();
    }
    // AppKit puts the overlay buttons back after order-front and focus.
    // The deferred pass runs after that layout.
    #[cfg(target_os = "macos")]
    schedule_hide_applet_traffic_lights(window.clone());
}

#[cfg(target_os = "macos")]
fn app_is_active() -> bool {
    use objc2::MainThreadMarker;
    use objc2_app_kit::NSApplication;

    let Some(marker) = MainThreadMarker::new() else {
        return false;
    };
    NSApplication::sharedApplication(marker).isActive()
}

#[cfg(target_os = "macos")]
pub fn hide_applet_traffic_lights(window: &tauri::WebviewWindow) {
    use objc2_app_kit::{NSWindow, NSWindowStyleMask};

    let Ok(native) = window.ns_window() else {
        return;
    };
    // SAFETY: Tauri returns the NSWindow owned by this webview window.
    let native = unsafe { &*native.cast::<NSWindow>() };
    hide_native_titlebar(native);
    // Close and minimize exist only while those style bits are set.
    // Zoom stays with Resizable, so the titlebar hide still covers it.
    let mut mask = native.styleMask();
    mask.remove(NSWindowStyleMask::Closable | NSWindowStyleMask::Miniaturizable);
    native.setStyleMask(mask);
}

/// AppKit lays the overlay buttons out again after show, focus, and resize.
/// One deferred pass is not enough: AppKit's own layout is also deferred.
/// The second pass runs after that layout. The flag is not the main window's.
#[cfg(target_os = "macos")]
static APPLET_TRAFFIC_HIDE_QUEUED: AtomicBool = AtomicBool::new(false);

#[cfg(target_os = "macos")]
struct AppletTrafficHide {
    window: tauri::WebviewWindow,
    pass: u8,
}

#[cfg(target_os = "macos")]
#[link(name = "System")]
unsafe extern "C" {
    static _dispatch_main_q: u8;
    fn dispatch_async_f(
        queue: *mut std::ffi::c_void,
        context: *mut std::ffi::c_void,
        work: unsafe extern "C" fn(*mut std::ffi::c_void),
    );
}

#[cfg(target_os = "macos")]
fn applet_main_queue() -> *mut std::ffi::c_void {
    std::ptr::addr_of!(_dispatch_main_q).cast_mut().cast()
}

#[cfg(target_os = "macos")]
unsafe extern "C" fn dispatch_hide_applet_traffic_lights(context: *mut std::ffi::c_void) {
    if context.is_null() {
        APPLET_TRAFFIC_HIDE_QUEUED.store(false, Ordering::Release);
        return;
    }
    let request = unsafe { Box::from_raw(context.cast::<AppletTrafficHide>()) };
    hide_applet_traffic_lights(&request.window);
    if request.pass == 0 {
        let again = Box::into_raw(Box::new(AppletTrafficHide {
            window: request.window.clone(),
            pass: 1,
        }));
        unsafe {
            dispatch_async_f(
                applet_main_queue(),
                again.cast(),
                dispatch_hide_applet_traffic_lights,
            )
        };
        return;
    }
    APPLET_TRAFFIC_HIDE_QUEUED.store(false, Ordering::Release);
}

#[cfg(target_os = "macos")]
pub(crate) fn schedule_hide_applet_traffic_lights(window: tauri::WebviewWindow) {
    if APPLET_TRAFFIC_HIDE_QUEUED.swap(true, Ordering::AcqRel) {
        return;
    }
    let context = Box::into_raw(Box::new(AppletTrafficHide { window, pass: 0 })).cast();
    unsafe {
        dispatch_async_f(
            applet_main_queue(),
            context,
            dispatch_hide_applet_traffic_lights,
        )
    };
}

/// Hide the system traffic lights and let the page bar receive clicks.
/// `setIgnoresMouseEvents:` is not on the titlebar view; sending it aborts.
#[cfg(target_os = "macos")]
fn hide_native_titlebar(native: &objc2_app_kit::NSWindow) {
    use objc2::runtime::NSObjectProtocol;
    use objc2_app_kit::{NSView, NSWindowButton};

    for kind in [
        NSWindowButton::CloseButton,
        NSWindowButton::MiniaturizeButton,
        NSWindowButton::ZoomButton,
    ] {
        if let Some(button) = native.standardWindowButton(kind) {
            button.setHidden(true);
            button.setAlphaValue(0.0);
        }
    }
    let Some(close) = native.standardWindowButton(NSWindowButton::CloseButton) else {
        return;
    };
    let theme = native
        .contentView()
        .and_then(|content| unsafe { content.superview() });
    let selector = objc2::sel!(setIgnoresMouseEvents:);
    let mut current = unsafe { close.superview() };
    let mut titlebar: Option<objc2::rc::Retained<NSView>> = None;
    while let Some(view) = current {
        if theme.as_ref().is_some_and(|frame| same_view(frame, &view)) {
            break;
        }
        // Do not return here. A view that ignores clicks can still show the buttons.
        if view.respondsToSelector(selector) {
            let _: () = unsafe { objc2::msg_send![&*view, setIgnoresMouseEvents: true] };
        }
        let parent = unsafe { view.superview() };
        if parent
            .as_ref()
            .is_some_and(|parent| theme.as_ref().is_some_and(|frame| same_view(frame, parent)))
        {
            titlebar = Some(view);
            break;
        }
        current = parent;
    }
    // The theme frame is the window. Hiding it would hide the page.
    // The titlebar container is only the traffic-light strip.
    if let Some(container) = titlebar_container(&close) {
        if theme
            .as_ref()
            .is_none_or(|frame| !same_view(frame, &container))
        {
            container.setHidden(true);
            container.setAlphaValue(0.0);
        }
    } else if let Some(titlebar) = titlebar {
        titlebar.setHidden(true);
    }
}

#[cfg(target_os = "macos")]
fn same_view(
    left: &objc2::rc::Retained<objc2_app_kit::NSView>,
    right: &objc2_app_kit::NSView,
) -> bool {
    std::ptr::eq(left.as_ref() as &objc2_app_kit::NSView, right)
}

/// AppKit check for the titlebar crash. Called from the harness-free test,
/// because a window created off the main thread aborts.
#[cfg(target_os = "macos")]
#[doc(hidden)]
pub fn probe_applet_titlebar() -> Result<(), &'static str> {
    use objc2::{MainThreadMarker, MainThreadOnly, msg_send};
    use objc2_app_kit::{NSWindow, NSWindowButton, NSWindowStyleMask};
    use objc2_foundation::{NSPoint, NSRect, NSSize};

    let mtm = MainThreadMarker::new().ok_or("not the main thread")?;
    let rect = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(480.0, 320.0));
    let style = NSWindowStyleMask::Titled
        | NSWindowStyleMask::Closable
        | NSWindowStyleMask::Miniaturizable
        | NSWindowStyleMask::Resizable
        | NSWindowStyleMask::FullSizeContentView;
    let window: objc2::rc::Retained<NSWindow> = unsafe {
        msg_send![
            NSWindow::alloc(mtm),
            initWithContentRect: rect,
            styleMask: style,
            backing: 2usize,
            defer: false,
        ]
    };
    window.setTitlebarAppearsTransparent(true);
    let close = window
        .standardWindowButton(NSWindowButton::CloseButton)
        .ok_or("close button")?;
    let container = titlebar_container(&close).ok_or("titlebar container")?;
    let theme = window
        .contentView()
        .and_then(|content| unsafe { content.superview() })
        .ok_or("theme frame")?;
    if theme.isHidden() || container.isHidden() {
        return Err("titlebar started hidden");
    }
    hide_native_titlebar(&window);
    if !close.isHidden() {
        return Err("traffic light stayed visible");
    }
    if theme.isHidden() {
        return Err("theme frame was hidden");
    }
    let content = window.contentView().ok_or("content")?;
    if content.isHidden() {
        return Err("page was hidden");
    }
    if !titlebar_releases_clicks(&container) {
        return Err("titlebar still covers the page bar");
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn view_class_name(view: &objc2_app_kit::NSView) -> &str {
    use objc2::runtime::AnyObject;
    let object = unsafe { &*(std::ptr::from_ref(view) as *const AnyObject) };
    object.class().name().to_str().unwrap_or("")
}

#[cfg(target_os = "macos")]
fn titlebar_container(
    close: &objc2_app_kit::NSView,
) -> Option<objc2::rc::Retained<objc2_app_kit::NSView>> {
    let mut current = unsafe { close.superview() };
    while let Some(view) = current {
        if view_class_name(&view) == "NSTitlebarContainerView" {
            return Some(view);
        }
        current = unsafe { view.superview() };
    }
    None
}

#[cfg(target_os = "macos")]
fn titlebar_releases_clicks(view: &objc2_app_kit::NSView) -> bool {
    use objc2::runtime::NSObjectProtocol;
    if view.isHidden() {
        return true;
    }
    let setter = objc2::sel!(setIgnoresMouseEvents:);
    if !view.respondsToSelector(setter) {
        return false;
    }
    let ignores: bool = unsafe { objc2::msg_send![view, ignoresMouseEvents] };
    ignores
}

#[cfg(target_os = "macos")]
fn present_applet_window(window: &tauri::WebviewWindow, focus: bool) {
    use objc2_app_kit::{NSWindow, NSWindowCollectionBehavior};

    let Ok(native) = window.ns_window() else {
        return;
    };
    let native = unsafe { &*native.cast::<NSWindow>() };
    native.setCollectionBehavior(
        native.collectionBehavior() | NSWindowCollectionBehavior::MoveToActiveSpace,
    );
    if focus {
        native.makeKeyAndOrderFront(None);
    } else {
        // orderFrontRegardless shows the window and does not activate the app.
        native.orderFrontRegardless();
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{
        AppletPresent, AppletWindowHost, applet_page_url, applet_present, clean_permissions,
        data_store_id, is_applet_label, normalize_title, window_bounds,
    };

    #[test]
    fn an_inactive_app_shows_an_applet_without_taking_focus() {
        assert_eq!(applet_present(false), AppletPresent::ShowOnly);
        assert_eq!(applet_present(true), AppletPresent::Focus);
    }

    #[test]
    fn page_url_stays_on_the_main_renderer() {
        let url = applet_page_url("leave", "release", 42, &[], None).unwrap();
        assert_eq!(
            url,
            "http://127.0.0.1:4318/windows/main/index.html?workbench-applet=leave&channel=release&open=42&permissions="
        );
        let granted = applet_page_url(
            "leave",
            "release",
            42,
            &["user".to_owned(), "knowledge".to_owned()],
            None,
        )
        .unwrap();
        assert!(granted.ends_with("&permissions=user,knowledge"));
        assert!(applet_page_url("../leave", "release", 1, &[], None).is_err());
        assert!(applet_page_url("add", "release", 1, &[], None).is_err());
        assert!(applet_page_url("leave", "development", 1, &[], None).is_err());
        assert!(applet_page_url("review", "release", 7, &[], None).is_err());
        assert!(
            applet_page_url("plan", "release", 7, &[], None)
                .unwrap()
                .contains("workbench-applet=plan")
        );
        let tool = applet_page_url("pdf", "release", 7, &[], Some("pdf-to-word")).unwrap();
        assert!(tool.ends_with("&tool=pdf-to-word"));
        assert!(applet_page_url("pdf", "release", 7, &[], Some("translate")).is_err());
        assert!(applet_page_url("leave", "release", 7, &[], Some("pdf-to-word")).is_err());
        assert!(super::review_file_name("合同.docx").is_ok());
        assert!(super::review_file_name("scan.pdf").is_err());
        assert!(super::review_file_name("../合同.docx").is_err());
    }

    #[test]
    fn script_keeps_the_shell_token_out_of_the_page() {
        let access = Arc::new(crate::applet_runtime::AppletAccess::open("toyou"));
        let shell = "ab".repeat(32);
        let host = AppletWindowHost::new(
            "toyou",
            "ToWork",
            "http://127.0.0.1:9",
            &shell,
            Arc::clone(&access),
        );
        let ticket = access.mint("leave", "applet-leave", &shell).unwrap();
        let script = host.initialization_script("leave", &ticket, &["user".to_owned()]);
        assert!(script.contains("\"permissions\":[\"user\"]"));
        assert!(script.contains("__A3S_APPLET_RUNTIME__"));
        assert!(script.contains("appletTicket"));
        assert!(!script.contains("__A3S_STUDIO_RUNTIME__"));
        assert!(!script.contains(&shell));
        assert_ne!(
            data_store_id("toyou", "leave"),
            data_store_id("toyou", "travel")
        );
        assert_eq!(
            data_store_id("toyou", "leave"),
            data_store_id("toyou", "leave")
        );
    }

    #[test]
    fn labels_match_office_applets_only() {
        assert!(is_applet_label("applet-leave"));
        assert!(is_applet_label("applet-pdf"));
        assert!(is_applet_label("applet-plan"));
        assert!(!is_applet_label("applet-add"));
        assert!(!is_applet_label("main"));
        assert!(!is_applet_label("applet-leave-extra"));
    }

    #[test]
    fn review_has_no_standalone_window_url_or_label() {
        assert!(applet_page_url("review", "release", 1, &[], None).is_err());
        assert!(!is_applet_label("applet-review"));
    }

    #[test]
    fn a_package_window_replaces_the_host_default_and_caps_the_frame() {
        let bounds =
            window_bounds("leave", Some(480.0), Some(720.0), Some(840.0), Some(960.0)).unwrap();
        assert_eq!((bounds.width, bounds.height), (480.0, 720.0));
        assert_eq!(
            (bounds.max_width, bounds.max_height),
            (Some(840.0), Some(960.0))
        );
        assert!(bounds.min_width <= bounds.width);
        assert!(bounds.min_height <= bounds.height);
        assert!(
            window_bounds("leave", Some(900.0), Some(720.0), Some(840.0), Some(960.0)).is_err()
        );
        assert!(window_bounds("leave", Some(480.0), None, None, None).is_err());
        assert!(clean_permissions(Some(vec!["phone".to_owned()])).is_err());
        assert_eq!(
            clean_permissions(Some(vec!["knowledge".to_owned(), "user".to_owned()])).unwrap(),
            vec!["user".to_owned(), "knowledge".to_owned()]
        );
    }

    #[test]
    fn title_drops_control_characters() {
        assert_eq!(normalize_title("  请假\n ", "leave"), "请假");
        assert_eq!(normalize_title("   ", "leave"), "leave");
    }
}
