//! Open a page inside the main window.
//!
//! The workspace pane uses one child webview, labeled `studio-embed`.
//! A published notice uses a second child, labeled `studio-notice`.
//! A new address navigates that child. `window.open` stays in that child.
//! The renderer supplies the rectangle. A hidden pane hides the child.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use tauri::webview::{NewWindowResponse, WebviewBuilder};
use tauri::{
    AppHandle, LogicalPosition, LogicalSize, Manager, Position, Rect, Runtime, Size, WebviewUrl,
};

pub(crate) const EMBED_LABEL: &str = "studio-embed";
/// Latest workspace-page placement. An older show must not uncover a newer hide.
static EMBED_EPOCH: AtomicU64 = AtomicU64::new(0);

pub(crate) fn embed_epoch_applies(epoch: u64, latest: u64) -> bool {
    epoch == latest && epoch != 0
}
pub(crate) const NOTICE_LABEL: &str = "studio-notice";
const HOST_LABEL: &str = "main";
/// WKWebView omits the Safari token. Chinese sites then show a human check.
const PAGE_USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.6 Safari/605.1.15";
const MAX_EDGE: f64 = 8000.0;
const MIN_ORIGIN: f64 = -8000.0;
const MAX_ORIGIN: f64 = 16000.0;

#[derive(Clone, Debug)]
pub(crate) struct EmbedRequest {
    pub session_id: String,
    pub url: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub visible: bool,
    pub reload_key: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum EmbedPlan {
    Hide,
    Show {
        url: url::Url,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        reload_key: u64,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EmbedAction {
    Create,
    Move,
    Navigate,
    Reload,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Placed {
    url: String,
    reload_key: u64,
}

pub(crate) fn plan_embed(request: &EmbedRequest) -> Result<EmbedPlan, String> {
    sanitize_session_id(&request.session_id)?;
    let url = parse_page(&request.url)?;
    if !request.visible {
        return Ok(EmbedPlan::Hide);
    }
    if !request.x.is_finite()
        || !request.y.is_finite()
        || !request.width.is_finite()
        || !request.height.is_finite()
    {
        return Err("WebView bounds are invalid.".to_owned());
    }
    if request.width < 1.0 || request.height < 1.0 {
        return Ok(EmbedPlan::Hide);
    }
    if request.width > MAX_EDGE
        || request.height > MAX_EDGE
        || !(MIN_ORIGIN..=MAX_ORIGIN).contains(&request.x)
        || !(MIN_ORIGIN..=MAX_ORIGIN).contains(&request.y)
    {
        return Err("WebView bounds are out of range.".to_owned());
    }
    Ok(EmbedPlan::Show {
        url,
        x: request.x,
        y: request.y,
        width: request.width,
        height: request.height,
        reload_key: request.reload_key,
    })
}

fn embed_action(previous: Option<&Placed>, url: &str, reload_key: u64) -> EmbedAction {
    match previous {
        None => EmbedAction::Create,
        Some(state) if state.url != url => EmbedAction::Navigate,
        Some(state) if state.reload_key != reload_key => EmbedAction::Reload,
        Some(_) => EmbedAction::Move,
    }
}

fn page_allowed(url: &url::Url) -> bool {
    if !url.username().is_empty() || url.password().is_some() {
        return false;
    }
    matches!(url.scheme(), "http" | "https" | "file")
}

fn parse_page(value: &str) -> Result<url::Url, String> {
    let parsed = url::Url::parse(value.trim()).map_err(|_| "WebView URL is invalid.".to_owned())?;
    if !page_allowed(&parsed) {
        return Err("WebView URL scheme is not allowed.".to_owned());
    }
    Ok(parsed)
}

/// A local page and the directory WebKit may read while rendering it.
///
/// The directory is the file's parent. A page at the filesystem root may read
/// only itself, so a local document cannot reach the rest of the disk.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LocalFilePage {
    pub path: std::path::PathBuf,
    pub access: std::path::PathBuf,
}

pub(crate) fn local_file_page(url: &url::Url) -> Option<LocalFilePage> {
    if url.scheme() != "file" {
        return None;
    }
    let path = url.to_file_path().ok()?;
    if !path.is_absolute() {
        return None;
    }
    let access = match path.parent() {
        Some(directory) if !is_filesystem_root(directory) => directory.to_path_buf(),
        _ => path.clone(),
    };
    Some(LocalFilePage { path, access })
}

fn is_filesystem_root(path: &std::path::Path) -> bool {
    use std::path::Component;
    let mut parts = path
        .components()
        .filter(|component| !matches!(component, Component::Prefix(_)));
    matches!(parts.next(), Some(Component::RootDir)) && parts.next().is_none()
}

fn sanitize_session_id(session_id: &str) -> Result<String, String> {
    let trimmed = session_id.trim();
    if trimmed.is_empty()
        || trimmed.chars().count() > 200
        || trimmed.chars().any(|character| character.is_control())
    {
        return Err("WebView session is invalid.".to_owned());
    }
    Ok(trimmed.to_owned())
}

fn embed_state() -> &'static Mutex<Option<Placed>> {
    static STATE: OnceLock<Mutex<Option<Placed>>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(None))
}

/// Top-left `y` in the parent view. A non-flipped AppKit view grows upward.
pub(crate) fn child_frame_origin(parent_height: f64, flipped: bool, y: f64, height: f64) -> f64 {
    if flipped {
        y
    } else {
        parent_height - y - height
    }
}

fn notice_state() -> &'static Mutex<Option<Placed>> {
    static STATE: OnceLock<Mutex<Option<Placed>>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(None))
}

pub(crate) fn place_embed<R: Runtime>(app: &AppHandle<R>, plan: EmbedPlan) -> Result<(), String> {
    let epoch = EMBED_EPOCH.fetch_add(1, Ordering::AcqRel) + 1;
    place_child(app, EMBED_LABEL, embed_state(), plan, Some(epoch))
}

pub(crate) fn place_notice<R: Runtime>(app: &AppHandle<R>, plan: EmbedPlan) -> Result<(), String> {
    place_child(app, NOTICE_LABEL, notice_state(), plan, None)
}

fn place_child<R: Runtime>(
    app: &AppHandle<R>,
    label: &'static str,
    state: &Mutex<Option<Placed>>,
    plan: EmbedPlan,
    epoch: Option<u64>,
) -> Result<(), String> {
    let Some(window) = app.get_window(HOST_LABEL) else {
        return Err("Workspace window is missing.".to_owned());
    };
    let pin: Result<Option<(f64, f64, f64, f64, Option<LocalFilePage>)>, String> = {
        let mut placed = state
            .lock()
            .map_err(|_| "WebView embed lock is poisoned.".to_owned())?;
        match plan {
            EmbedPlan::Hide => {
                if let Some(child) = app.get_webview(label) {
                    set_webview_visible(&child, false, epoch)?;
                }
                Ok(None)
            }
            EmbedPlan::Show {
                url,
                x,
                y,
                width,
                height,
                reload_key,
            } => {
                let local = local_file_page(&url);
                let url_string = url.to_string();
                let position = LogicalPosition::new(x, y);
                let size = LogicalSize::new(width, height);
                let bounds = Rect {
                    position: Position::Logical(position),
                    size: Size::Logical(size),
                };
                // WKWebView loadRequest leaves a file page blank. Load it after the lock drops.
                let file_page = cfg!(target_os = "macos") && local.is_some();
                let mut open_local = false;
                if let Some(child) = app.get_webview(label) {
                    match embed_action(placed.as_ref(), &url_string, reload_key) {
                        EmbedAction::Create | EmbedAction::Navigate => {
                            if !file_page {
                                child.navigate(url).map_err(|error| {
                                    format!("Could not open the workspace page: {error}")
                                })?;
                            }
                            open_local = file_page;
                        }
                        EmbedAction::Reload => {
                            if !file_page {
                                child.reload().map_err(|error| {
                                    format!("Could not reload the workspace page: {error}")
                                })?;
                            }
                            open_local = file_page;
                        }
                        EmbedAction::Move => {}
                    }
                    // One bounds message. A separate position keeps the old height and covers the toolbar.
                    child
                        .set_bounds(bounds)
                        .map_err(|error| format!("Could not place the workspace page: {error}"))?;
                } else {
                    let handle = app.clone();
                    let builder = WebviewBuilder::new(label, WebviewUrl::External(url))
                        .focused(false)
                        .background_color(tauri::webview::Color(255, 255, 255, 255))
                        .on_navigation(page_allowed)
                        .on_new_window(move |target, _features| {
                            if page_allowed(&target) {
                                if let Some(webview) = handle.get_webview(label) {
                                    let opened =
                                        present_local_page(&webview, &target).unwrap_or(false);
                                    if !opened {
                                        let _ = webview.navigate(target);
                                    }
                                }
                            }
                            NewWindowResponse::Deny
                        });
                    // WKWebView has no Safari token until this value is set at creation.
                    #[cfg(target_os = "macos")]
                    let builder = builder.user_agent(PAGE_USER_AGENT);
                    window
                        .add_child(builder, position, size)
                        .map_err(|error| format!("Could not open the workspace page: {error}"))?;
                    open_local = file_page;
                }
                *placed = Some(Placed {
                    url: url_string,
                    reload_key,
                });
                let local = if open_local { local } else { None };
                Ok(Some((x, y, width, height, local)))
            }
        }
    };
    // Drop the embed lock before touching the view. with_webview waits on the main thread.
    if let Some((x, y, width, height, local)) = pin? {
        #[cfg(target_os = "macos")]
        pin_embed_frame(app, label, x, y, width, height)?;
        #[cfg(not(target_os = "macos"))]
        let _ = (x, y, width, height);
        if let Some(child) = app.get_webview(label) {
            set_webview_visible(&child, true, epoch)?;
        }
        if let Some(page) = local {
            #[cfg(target_os = "macos")]
            if let Some(child) = app.get_webview(label) {
                // Creation used loadRequest. Replace it with a file load that can read the page.
                load_macos_file(&child, &page)?;
            }
            #[cfg(not(target_os = "macos"))]
            let _ = page;
        }
    }
    Ok(())
}

/// A stale show hides the page. A stale hide leaves the newer placement alone.
fn set_webview_visible<R: Runtime>(
    child: &tauri::Webview<R>,
    visible: bool,
    epoch: Option<u64>,
) -> Result<(), String> {
    let current = match epoch {
        Some(epoch) => embed_epoch_applies(epoch, EMBED_EPOCH.load(Ordering::Acquire)),
        None => true,
    };
    let hidden = if current { !visible } else { visible };
    if !current && !visible {
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    {
        child
            .with_webview(move |platform| {
                use objc2_app_kit::NSView;
                let pointer = platform.inner();
                if pointer.is_null() {
                    return;
                }
                unsafe {
                    let view = &*pointer.cast::<NSView>();
                    view.setHidden(hidden);
                }
            })
            .map_err(|_| "Could not hide the workspace page.".to_owned())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let result = if hidden { child.hide() } else { child.show() };
        result.map_err(|error| format!("Could not hide the workspace page: {error}"))
    }
}

/// `loadRequest` on a file URL paints a blank page. This grants read access to
/// the page and its directory, then loads that file.
fn present_local_page<R: Runtime>(
    webview: &tauri::Webview<R>,
    url: &url::Url,
) -> Result<bool, String> {
    let Some(page) = local_file_page(url) else {
        return Ok(false);
    };
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (webview, page);
        return Ok(false);
    }
    #[cfg(target_os = "macos")]
    {
        load_macos_file(webview, &page)?;
        Ok(true)
    }
}

#[cfg(target_os = "macos")]
fn load_macos_file<R: Runtime>(
    webview: &tauri::Webview<R>,
    page: &LocalFilePage,
) -> Result<(), String> {
    let path = page
        .path
        .to_str()
        .ok_or_else(|| "Could not open the local page.".to_owned())?
        .to_owned();
    let access = page
        .access
        .to_str()
        .ok_or_else(|| "Could not open the local page.".to_owned())?
        .to_owned();
    let directory = page.access != page.path;
    webview
        .with_webview(move |platform| {
            use objc2_foundation::{NSString, NSURL};
            use objc2_web_kit::WKWebView;

            let pointer = platform.inner();
            if pointer.is_null() {
                return;
            }
            unsafe {
                let view = &*pointer.cast::<WKWebView>();
                let file = NSURL::fileURLWithPath(&NSString::from_str(&path));
                let read = if directory {
                    NSURL::fileURLWithPath_isDirectory(&NSString::from_str(&access), true)
                } else {
                    NSURL::fileURLWithPath(&NSString::from_str(&access))
                };
                let _ = view.loadFileURL_allowingReadAccessToURL(&file, &read);
            }
        })
        .map_err(|_| "Could not open the local page.".to_owned())
}

/// wry pins a child with `ViewMinYMargin`. AppKit then moves that view onto the toolbar.
#[cfg(target_os = "macos")]
fn pin_embed_frame<R: Runtime>(
    app: &AppHandle<R>,
    label: &str,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> Result<(), String> {
    let Some(child) = app.get_webview(label) else {
        return Ok(());
    };
    child
        .with_webview(move |platform| {
            use objc2_app_kit::{NSAutoresizingMaskOptions, NSView};
            use objc2_foundation::{NSPoint, NSRect, NSSize};

            let pointer = platform.inner();
            if pointer.is_null() {
                return;
            }
            unsafe {
                let view = &*pointer.cast::<NSView>();
                let Some(parent) = view.superview() else {
                    return;
                };
                let origin_y =
                    child_frame_origin(parent.frame().size.height, parent.isFlipped(), y, height);
                view.setAutoresizingMask(NSAutoresizingMaskOptions::ViewNotSizable);
                view.setFrame(NSRect {
                    origin: NSPoint::new(x, origin_y),
                    size: NSSize::new(width, height),
                });
            }
        })
        .map_err(|_| "Could not place the workspace page.".to_owned())
}

/// Place or hide the workspace page.
///
/// The command is async so `add_child` is not called from a synchronous command.
/// Windows WebView2 deadlocks when a synchronous command creates a webview.
#[tauri::command]
pub async fn launch_workspace_browser(
    app: AppHandle,
    session_id: String,
    url: String,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    visible: bool,
    reload_key: u64,
) -> Result<(), String> {
    let plan = plan_embed(&EmbedRequest {
        session_id,
        url,
        x,
        y,
        width,
        height,
        visible,
        reload_key,
    })?;
    place_embed(&app, plan)?;
    #[cfg(target_os = "macos")]
    note_embed(&app);
    Ok(())
}

/// Place or hide the notice page over a dialog in the main window.
///
/// This child is not the workspace pane. Hiding it leaves `studio-embed` where it was.
#[tauri::command]
pub async fn launch_notice_browser(
    app: AppHandle,
    url: String,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    visible: bool,
    reload_key: u64,
) -> Result<(), String> {
    let plan = plan_embed(&EmbedRequest {
        session_id: "about-notice".to_owned(),
        url,
        x,
        y,
        width,
        height,
        visible,
        reload_key,
    })?;
    place_notice(&app, plan)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NavAction {
    State,
    Back,
    Forward,
    Stop,
}

fn parse_nav_action(value: &str) -> Result<NavAction, String> {
    match value.trim() {
        "state" => Ok(NavAction::State),
        "back" => Ok(NavAction::Back),
        "forward" => Ok(NavAction::Forward),
        "stop" => Ok(NavAction::Stop),
        _ => Err(
            "Workspace browser navigation is not in the closed set: state, back, forward, stop."
                .to_owned(),
        ),
    }
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceBrowserNav {
    url: String,
    can_go_back: bool,
    can_go_forward: bool,
    loading: bool,
    title: String,
}

fn clip_title(value: String) -> String {
    value
        .chars()
        .filter(|character| !character.is_control())
        .take(300)
        .collect()
}

#[cfg(not(target_os = "macos"))]
fn current_page_url(webview: &tauri::Webview) -> String {
    webview.url().map(|url| url.to_string()).unwrap_or_default()
}

/// `with_webview` runs its closure immediately only on the main thread.
/// This command runs on the async pool, so an unread slot is always empty.
fn recv_nav(
    rx: std::sync::mpsc::Receiver<Result<WorkspaceBrowserNav, String>>,
    timeout: std::time::Duration,
) -> Result<WorkspaceBrowserNav, String> {
    rx.recv_timeout(timeout)
        .map_err(|_| "The workspace page is unavailable.".to_owned())?
}

#[cfg(target_os = "macos")]
fn macos_nav(
    pointer: *mut std::ffi::c_void,
    action: NavAction,
) -> Result<WorkspaceBrowserNav, String> {
    use objc2_web_kit::WKWebView;

    if pointer.is_null() {
        return Err("The workspace page is unavailable.".to_owned());
    }
    let view = unsafe { &*pointer.cast::<WKWebView>() };
    match action {
        NavAction::Back => {
            let _ = unsafe { view.goBack() };
        }
        NavAction::Forward => {
            let _ = unsafe { view.goForward() };
        }
        NavAction::Stop => unsafe { view.stopLoading() },
        NavAction::State => {}
    }
    let url = unsafe { view.URL() }
        .and_then(|value| value.absoluteString())
        .map(|value| value.to_string())
        .unwrap_or_default();
    let title = unsafe { view.title() }
        .map(|value| clip_title(value.to_string()))
        .unwrap_or_default();
    Ok(WorkspaceBrowserNav {
        url,
        can_go_back: unsafe { view.canGoBack() },
        can_go_forward: unsafe { view.canGoForward() },
        loading: unsafe { view.isLoading() },
        title,
    })
}

#[cfg(target_os = "macos")]
fn move_history(
    webview: &tauri::Webview,
    action: NavAction,
) -> Result<WorkspaceBrowserNav, String> {
    let (tx, rx) = std::sync::mpsc::channel();
    webview
        .with_webview(move |platform| {
            let _ = tx.send(macos_nav(platform.inner(), action));
        })
        .map_err(|_| "The workspace page is unavailable.".to_owned())?;
    recv_nav(rx, std::time::Duration::from_secs(1))
}

#[cfg(not(target_os = "macos"))]
fn move_history(
    webview: &tauri::Webview,
    action: NavAction,
) -> Result<WorkspaceBrowserNav, String> {
    let script = match action {
        NavAction::Back => "history.back()",
        NavAction::Forward => "history.forward()",
        NavAction::Stop => "window.stop()",
        NavAction::State => "",
    };
    if !script.is_empty() {
        webview
            .eval(script)
            .map_err(|_| "The workspace page is unavailable.".to_owned())?;
    }
    Ok(WorkspaceBrowserNav {
        url: current_page_url(webview),
        can_go_back: false,
        can_go_forward: false,
        loading: false,
        title: String::new(),
    })
}

/// Read or move the embedded page history.
///
/// The address bar does not own the child webview, so back, forward, and the
/// current address come from that child. The main thread applies the action.
/// Waiting here keeps that read off the UI thread.
#[tauri::command]
pub async fn workspace_browser_navigate(
    app: AppHandle,
    action: String,
) -> Result<WorkspaceBrowserNav, String> {
    let action = parse_nav_action(&action)?;
    let webview = app
        .get_webview(EMBED_LABEL)
        .ok_or_else(|| "The workspace page is unavailable.".to_owned())?;
    tauri::async_runtime::spawn_blocking(move || move_history(&webview, action))
        .await
        .map_err(|_| "The workspace page is unavailable.".to_owned())?
}

#[cfg(target_os = "macos")]
static PAGE: std::sync::atomic::AtomicPtr<std::ffi::c_void> =
    std::sync::atomic::AtomicPtr::new(std::ptr::null_mut());

#[cfg(target_os = "macos")]
fn note_embed(app: &AppHandle) {
    use std::sync::atomic::Ordering;

    if let Some(child) = app.get_webview(EMBED_LABEL) {
        let _ = child.with_webview(|platform| {
            PAGE.store(platform.inner(), Ordering::Release);
        });
    }
    // The monitor is type-checked in unit tests. Tests do not install it.
    #[cfg(not(test))]
    install_page_monitor(app);
}

#[cfg(target_os = "macos")]
#[cfg_attr(test, allow(dead_code))]
fn install_page_monitor(app: &AppHandle) {
    use std::sync::atomic::{AtomicBool, Ordering};

    static MONITOR: AtomicBool = AtomicBool::new(false);
    if MONITOR.swap(true, Ordering::AcqRel) {
        return;
    }
    let app = app.clone();
    let _ = app.clone().run_on_main_thread(move || {
        use block2::RcBlock;
        use objc2::MainThreadMarker;
        use objc2_app_kit::{NSEvent, NSEventMask, NSEventModifierFlags, NSView};
        use objc2_web_kit::WKWebView;
        use std::ptr::NonNull;

        let block = RcBlock::new(move |event: NonNull<NSEvent>| -> *mut NSEvent {
            let keep = event.as_ptr();
            let event_ref = unsafe { event.as_ref() };
            let Some(marker) = MainThreadMarker::new() else {
                return keep;
            };
            let Some(window) = event_ref.window(marker) else {
                return keep;
            };
            let Some(responder) = window.firstResponder() else {
                return keep;
            };
            let Ok(view) = responder.downcast::<NSView>() else {
                return keep;
            };
            let page = PAGE.load(Ordering::Acquire);
            if page.is_null() {
                return keep;
            }
            let page_view = unsafe { &*page.cast::<NSView>() };
            if page_view.isHiddenOrHasHiddenAncestor() || !view.isDescendantOf(page_view) {
                return keep;
            }
            let page_web = unsafe { &*page.cast::<WKWebView>() };
            let text = event_ref
                .charactersIgnoringModifiers()
                .map(|value| value.to_string())
                .unwrap_or_default();
            let key = text.to_lowercase();
            if key == "\u{1b}" || event_ref.keyCode() == 53 {
                if unsafe { page_web.isLoading() } {
                    unsafe { page_web.stopLoading() };
                    return std::ptr::null_mut();
                }
                return keep;
            }
            let flags =
                event_ref.modifierFlags() & NSEventModifierFlags::DeviceIndependentFlagsMask;
            let typed = flags.difference(NSEventModifierFlags::CapsLock);
            if !typed.contains(NSEventModifierFlags::Command)
                || typed.contains(NSEventModifierFlags::Shift)
                || typed.contains(NSEventModifierFlags::Option)
                || typed.contains(NSEventModifierFlags::Control)
            {
                return keep;
            }
            match key.as_str() {
                "l" => focus_address(&app),
                "[" => {
                    let _ = unsafe { page_web.goBack() };
                }
                "]" => {
                    let _ = unsafe { page_web.goForward() };
                }
                "r" => {
                    let _ = unsafe { page_web.reload() };
                }
                _ => return keep,
            }
            std::ptr::null_mut()
        });
        let block = Box::leak(Box::new(block));
        unsafe {
            let _ =
                NSEvent::addLocalMonitorForEventsMatchingMask_handler(NSEventMask::KeyDown, block);
        }
    });
}

#[cfg(target_os = "macos")]
fn focus_address(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        let Some(main) = app.get_webview(HOST_LABEL) else {
            return;
        };
        let _ = main.eval(
            "window.dispatchEvent(new CustomEvent('studio-browser-shortcut',{detail:'focus-address'}))",
        );
    });
}

#[cfg(test)]
mod tests {
    use super::{
        EmbedAction, EmbedPlan, EmbedRequest, NavAction, PAGE_USER_AGENT, Placed,
        WorkspaceBrowserNav, child_frame_origin, embed_action, embed_epoch_applies,
        local_file_page, parse_nav_action, plan_embed, recv_nav,
    };

    #[test]
    fn older_embed_placement_does_not_apply() {
        assert!(embed_epoch_applies(2, 2));
        assert!(!embed_epoch_applies(1, 2));
        assert!(!embed_epoch_applies(0, 0));
    }

    fn request(
        session_id: &str,
        url: &str,
        visible: bool,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        reload_key: u64,
    ) -> EmbedRequest {
        EmbedRequest {
            session_id: session_id.to_owned(),
            url: url.to_owned(),
            x,
            y,
            width,
            height,
            visible,
            reload_key,
        }
    }

    fn sample_nav(can_go_back: bool) -> WorkspaceBrowserNav {
        WorkspaceBrowserNav {
            url: "https://example.com/next".to_owned(),
            can_go_back,
            can_go_forward: false,
            loading: false,
            title: "Next".to_owned(),
        }
    }

    #[test]
    fn history_answer_waits_for_the_page() {
        let (tx, rx) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(40));
            let _ = tx.send(Ok(sample_nav(true)));
        });
        let state = recv_nav(rx, std::time::Duration::from_millis(500)).expect("page answered");
        assert!(state.can_go_back);
        assert_eq!(state.url, "https://example.com/next");
        worker.join().expect("worker");
    }

    #[test]
    fn history_answer_fails_closed_without_a_page() {
        let (tx, rx) = std::sync::mpsc::channel::<Result<WorkspaceBrowserNav, String>>();
        let Err(error) = recv_nav(rx, std::time::Duration::from_millis(30)) else {
            panic!("no page");
        };
        assert_eq!(error, "The workspace page is unavailable.");
        drop(tx);
    }

    #[test]
    fn accepts_only_the_history_actions() {
        assert_eq!(parse_nav_action("state"), Ok(NavAction::State));
        assert_eq!(parse_nav_action(" back "), Ok(NavAction::Back));
        assert_eq!(parse_nav_action("forward"), Ok(NavAction::Forward));
        assert_eq!(parse_nav_action("stop"), Ok(NavAction::Stop));
        assert!(parse_nav_action("reload").is_err());
        assert!(parse_nav_action("").is_err());
    }

    #[test]
    fn page_user_agent_identifies_as_safari() {
        assert!(PAGE_USER_AGENT.contains("Safari/"));
        assert!(!PAGE_USER_AGENT.contains("Tauri"));
    }

    #[test]
    fn keeps_the_page_below_the_toolbar() {
        assert_eq!(child_frame_origin(900.0, false, 120.0, 480.0), 300.0);
        assert_eq!(child_frame_origin(900.0, true, 120.0, 480.0), 120.0);
    }

    #[test]
    fn plans_a_child_webview_for_an_http_page() {
        let plan = plan_embed(&request(
            "session-a",
            "https://example.com/docs",
            true,
            20.0,
            40.0,
            640.0,
            480.0,
            3,
        ))
        .unwrap();
        match plan {
            EmbedPlan::Show {
                url,
                x,
                y,
                width,
                height,
                reload_key,
            } => {
                assert_eq!(url.as_str(), "https://example.com/docs");
                assert_eq!(
                    (x, y, width, height, reload_key),
                    (20.0, 40.0, 640.0, 480.0, 3)
                );
            }
            EmbedPlan::Hide => panic!("expected the page to stay inside the pane"),
        }
    }

    #[test]
    fn accepts_a_published_notice_address() {
        let plan = plan_embed(&request(
            "about-notice",
            "http://127.0.0.1:3000/v1/api/legal/terms",
            true,
            80.0,
            120.0,
            900.0,
            640.0,
            1,
        ))
        .unwrap();
        match plan {
            EmbedPlan::Show {
                url, reload_key, ..
            } => {
                assert_eq!(url.as_str(), "http://127.0.0.1:3000/v1/api/legal/terms");
                assert_eq!(reload_key, 1);
            }
            EmbedPlan::Hide => panic!("expected the notice page"),
        }
    }

    #[test]
    fn accepts_a_file_page() {
        let plan = plan_embed(&request(
            "session-a",
            "file:///workspace/index.html",
            true,
            0.0,
            0.0,
            320.0,
            240.0,
            0,
        ))
        .unwrap();
        match plan {
            EmbedPlan::Show { url, .. } => assert_eq!(url.scheme(), "file"),
            EmbedPlan::Hide => panic!("expected a file page"),
        }
    }

    #[cfg(unix)]
    #[test]
    fn local_html_reads_its_own_directory() {
        let url = url::Url::parse("file:///private/var/pages/note.html").unwrap();
        let page = local_file_page(&url).expect("local page");
        assert_eq!(
            page.path,
            std::path::PathBuf::from("/private/var/pages/note.html")
        );
        assert_eq!(page.access, std::path::PathBuf::from("/private/var/pages"));
    }

    #[cfg(unix)]
    #[test]
    fn local_html_keeps_a_unicode_file_name() {
        let url = url::Url::parse("file:///private/var/pages/.answer-me-with-html/%E9%A1%B5.html")
            .unwrap();
        let page = local_file_page(&url).expect("local page");
        assert_eq!(
            page.path,
            std::path::PathBuf::from("/private/var/pages/.answer-me-with-html/页.html")
        );
        assert_eq!(
            page.access,
            std::path::PathBuf::from("/private/var/pages/.answer-me-with-html")
        );
    }

    #[cfg(unix)]
    #[test]
    fn local_html_at_the_filesystem_root_reads_only_that_file() {
        let url = url::Url::parse("file:///index.html").unwrap();
        let page = local_file_page(&url).expect("local page");
        assert_eq!(page.path, page.access);
        assert_eq!(page.access, std::path::PathBuf::from("/index.html"));
    }

    #[test]
    fn remote_page_is_not_a_local_file() {
        let url = url::Url::parse("https://example.com/index.html").unwrap();
        assert!(local_file_page(&url).is_none());
    }

    #[test]
    fn hides_a_valid_page_when_the_pane_is_not_presented() {
        let hidden = plan_embed(&request(
            "session-a",
            "https://example.com/docs",
            false,
            20.0,
            40.0,
            640.0,
            480.0,
            1,
        ))
        .unwrap();
        assert_eq!(hidden, EmbedPlan::Hide);
        let collapsed = plan_embed(&request(
            "session-a",
            "https://example.com/docs",
            true,
            20.0,
            40.0,
            0.0,
            480.0,
            1,
        ))
        .unwrap();
        assert_eq!(collapsed, EmbedPlan::Hide);
    }

    #[test]
    fn rejects_pages_the_workspace_must_not_open() {
        assert!(
            plan_embed(&request(
                "session-a",
                "javascript:alert(1)",
                true,
                0.0,
                0.0,
                640.0,
                480.0,
                0
            ))
            .is_err()
        );
        assert!(
            plan_embed(&request(
                "session-a",
                "javascript:alert(1)",
                false,
                0.0,
                0.0,
                640.0,
                480.0,
                0
            ))
            .is_err()
        );
        assert!(
            plan_embed(&request(
                "session-a",
                "https://user:secret@example.com/",
                true,
                0.0,
                0.0,
                640.0,
                480.0,
                0
            ))
            .is_err()
        );
        assert!(
            plan_embed(&request(
                "session-a",
                "about:blank",
                true,
                0.0,
                0.0,
                640.0,
                480.0,
                0
            ))
            .is_err()
        );
        assert!(
            plan_embed(&request(
                "session-a",
                "not a url",
                true,
                0.0,
                0.0,
                640.0,
                480.0,
                0
            ))
            .is_err()
        );
        assert!(
            plan_embed(&request(
                " ",
                "https://example.com/",
                true,
                0.0,
                0.0,
                640.0,
                480.0,
                0
            ))
            .is_err()
        );
        assert!(
            plan_embed(&request(
                "session-a",
                "https://example.com/",
                true,
                0.0,
                0.0,
                9000.0,
                480.0,
                0
            ))
            .is_err()
        );
        assert!(
            plan_embed(&request(
                "session-a",
                "https://example.com/",
                true,
                -9000.0,
                0.0,
                640.0,
                480.0,
                0
            ))
            .is_err()
        );
        assert!(
            plan_embed(&request(
                "session-a",
                "https://example.com/",
                true,
                f64::NAN,
                0.0,
                640.0,
                480.0,
                0
            ))
            .is_err()
        );
    }

    #[test]
    fn keeps_one_child_when_only_the_rectangle_changes() {
        let placed = Placed {
            url: "https://example.com/docs".to_owned(),
            reload_key: 1,
        };
        assert_eq!(embed_action(None, &placed.url, 1), EmbedAction::Create);
        assert_eq!(
            embed_action(Some(&placed), "https://example.com/next", 1),
            EmbedAction::Navigate
        );
        assert_eq!(
            embed_action(Some(&placed), &placed.url, 2),
            EmbedAction::Reload
        );
        assert_eq!(
            embed_action(Some(&placed), &placed.url, 1),
            EmbedAction::Move
        );
    }
}
