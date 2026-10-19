//! macOS overlay titlebars have no native drag strip. WKWebView ignores
//! `-webkit-app-region`. `performWindowDragWithEvent` only moves the window
//! when the view under the pointer calls it from `mouseDown:`. A transparent
//! strip above the web view owns the top 44px, skips the page's controls,
//! and starts the drag there. A real click is handled in `sendEvent:` before
//! the web view can take it, and the window frame follows the pointer until
//! the button comes up. The hook is on the window class, so every later
//! window receives it. Only the main window that installed the strip may
//! consume the click. Applet windows paint their own minimize, zoom, and
//! close buttons in that band.

#[derive(Clone, Copy)]
pub(crate) struct CssRect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

pub(crate) const TITLEBAR_CSS_HEIGHT: f64 = 44.0;

pub(crate) fn hit_window_drag(x: f64, y: f64, drag: &[CssRect], block: &[CssRect]) -> bool {
    if block.iter().any(|rect| contains(x, y, rect)) {
        return false;
    }
    drag.iter().any(|rect| contains(x, y, rect))
}

/// `published` is true only when the rectangles belong to the document that
/// is currently loaded. Otherwise the top band drags and stale rectangles
/// from the previous document are ignored.
pub(crate) fn hit_window_chrome(
    x: f64,
    y: f64,
    drag: &[CssRect],
    block: &[CssRect],
    published: bool,
) -> bool {
    if published {
        hit_window_drag(x, y, drag, block)
    } else {
        y >= 0.0 && y < TITLEBAR_CSS_HEIGHT
    }
}

/// The top band always drags. Published control rectangles pass the click
/// through. Rectangles from another document are ignored.
/// The class-wide `sendEvent:` hook must not drag an applet window. A null
/// installed window consumes nothing.
pub(crate) fn owns_titlebar_drag(event_window: usize, installed: usize) -> bool {
    installed != 0 && event_window == installed
}

pub(crate) fn claim_titlebar_drag(x: f64, y: f64, block: &[CssRect], published: bool) -> bool {
    if !x.is_finite() || !y.is_finite() || y < 0.0 || y >= TITLEBAR_CSS_HEIGHT {
        return false;
    }
    if published && block.iter().any(|rect| contains(x, y, rect)) {
        return false;
    }
    true
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) struct PageKey(String);

pub(crate) fn page_key(url: &url::Url) -> Option<PageKey> {
    if url.scheme() != "http" && url.scheme() != "https" {
        return None;
    }
    if url.host_str() != Some("127.0.0.1") {
        return None;
    }
    let port = url.port_or_known_default()?;
    Some(PageKey(format!(
        "{}://127.0.0.1:{port}{}",
        url.scheme(),
        url.path()
    )))
}

fn contains(x: f64, y: f64, rect: &CssRect) -> bool {
    x >= rect.x && y >= rect.y && x < rect.x + rect.w && y < rect.y + rect.h
}

fn rects_from(values: Vec<[f64; 4]>) -> Vec<CssRect> {
    values
        .into_iter()
        .filter_map(|[x, y, w, h]| {
            if [x, y, w, h].iter().all(|value| value.is_finite()) && w > 0.0 && h > 0.0 {
                Some(CssRect { x, y, w, h })
            } else {
                None
            }
        })
        .take(512)
        .collect()
}

#[derive(Default)]
struct WindowDragRegions {
    page: Option<PageKey>,
    /// The page still publishes these. The strip claims the whole top band
    /// except `block`, so the drag rectangles are stored and not read.
    #[allow(dead_code)]
    drag: Vec<CssRect>,
    block: Vec<CssRect>,
}

static DRAG_REGIONS: std::sync::Mutex<WindowDragRegions> =
    std::sync::Mutex::new(WindowDragRegions {
        page: None,
        drag: Vec::new(),
        block: Vec::new(),
    });

fn store_regions(page: Option<PageKey>, drag: Vec<[f64; 4]>, block: Vec<[f64; 4]>) {
    let Some(page) = page else {
        return;
    };
    *DRAG_REGIONS
        .lock()
        .unwrap_or_else(|error| error.into_inner()) = WindowDragRegions {
        page: Some(page),
        drag: rects_from(drag),
        block: rects_from(block),
    };
}

/// Replace the rectangles the top chrome uses for window dragging.
#[tauri::command]
pub fn set_window_drag_regions(webview: tauri::Webview, drag: Vec<[f64; 4]>, block: Vec<[f64; 4]>) {
    let page = webview.url().ok().as_ref().and_then(page_key);
    store_regions(page, drag, block);
}

#[cfg(target_os = "macos")]
fn claim_current_titlebar(x: f64, y: f64) -> bool {
    let current = current_page_key();
    let regions = DRAG_REGIONS
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let published = current.is_some() && regions.page == current;
    claim_titlebar_drag(x, y, &regions.block, published)
}

#[cfg(target_os = "macos")]
pub(crate) fn install(window: &tauri::WebviewWindow) {
    use objc2_app_kit::NSWindow;

    let Ok(ptr) = window.ns_window() else {
        return;
    };
    DRAG_WINDOW.store(ptr.cast(), std::sync::atomic::Ordering::Release);
    let ns_window = unsafe { &*ptr.cast::<NSWindow>() };
    ns_window.setMovable(true);
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        install_drag_strip(ns_window);
        install_drag_dispatch(ns_window);
    }));
    register_drag_handler(window);
}

#[cfg(target_os = "macos")]
fn install_drag_strip(window: &objc2_app_kit::NSWindow) {
    use std::sync::atomic::{AtomicBool, Ordering};

    use objc2::MainThreadMarker;
    use objc2_app_kit::NSAutoresizingMaskOptions;

    use drag_strip::TitlebarDragView;

    static INSTALLED: AtomicBool = AtomicBool::new(false);
    if INSTALLED.load(Ordering::Acquire) {
        return;
    }
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let Some(content) = window.contentView() else {
        return;
    };
    let strip = TitlebarDragView::new(mtm);
    strip.setWantsLayer(true);
    content.addSubview(&strip);
    strip.setAutoresizingMask(
        NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewMinYMargin,
    );
    place_drag_strip(&strip);
    INSTALLED.store(true, Ordering::Release);
}

#[cfg(target_os = "macos")]
fn place_drag_strip(strip: &drag_strip::TitlebarDragView) {
    use objc2_foundation::{NSPoint, NSRect, NSSize};

    let Some(content) = (unsafe { strip.superview() }) else {
        return;
    };
    let bounds = content.bounds();
    if bounds.size.width < 1.0 || bounds.size.height < 1.0 {
        return;
    }
    let height = TITLEBAR_CSS_HEIGHT.min(bounds.size.height);
    let frame = NSRect::new(
        NSPoint::new(
            bounds.origin.x,
            bounds.origin.y + bounds.size.height - height,
        ),
        NSSize::new(bounds.size.width, height),
    );
    let current = strip.frame();
    if current.origin.x == frame.origin.x
        && current.origin.y == frame.origin.y
        && current.size.width == frame.size.width
        && current.size.height == frame.size.height
    {
        return;
    }
    strip.setFrame(frame);
}

#[cfg(target_os = "macos")]
fn track_window_drag(window: &objc2_app_kit::NSWindow) {
    use objc2_app_kit::{NSEvent, NSEventMask, NSEventTrackingRunLoopMode, NSEventType};
    use objc2_foundation::{NSDate, NSPoint};

    let start_mouse = NSEvent::mouseLocation();
    let start_origin = window.frame().origin;
    let mask = NSEventMask::LeftMouseDragged | NSEventMask::LeftMouseUp;
    let until = NSDate::distantFuture();
    let mode = unsafe { NSEventTrackingRunLoopMode };
    loop {
        let Some(next) =
            window.nextEventMatchingMask_untilDate_inMode_dequeue(mask, Some(&until), mode, true)
        else {
            break;
        };
        if next.r#type() == NSEventType::LeftMouseUp {
            break;
        }
        let mouse = NSEvent::mouseLocation();
        window.setFrameOrigin(NSPoint::new(
            start_origin.x + mouse.x - start_mouse.x,
            start_origin.y + mouse.y - start_mouse.y,
        ));
        if NSEvent::pressedMouseButtons() & 1 == 0 {
            break;
        }
    }
}

#[cfg(target_os = "macos")]
fn install_drag_dispatch(window: &objc2_app_kit::NSWindow) {
    use std::sync::atomic::{AtomicBool, Ordering};

    static INSTALLED: AtomicBool = AtomicBool::new(false);
    if INSTALLED.swap(true, Ordering::AcqRel) {
        return;
    }
    let class = window.class();
    let Some(method) = class.instance_method(objc2::sel!(sendEvent:)) else {
        return;
    };
    ORIGINAL_SEND_EVENT.store(method.implementation() as *mut (), Ordering::Release);
    unsafe {
        objc2::ffi::class_replaceMethod(
            class as *const _ as *mut _,
            objc2::sel!(sendEvent:),
            std::mem::transmute::<
                unsafe extern "C-unwind" fn(
                    *mut objc2::runtime::AnyObject,
                    objc2::runtime::Sel,
                    *mut objc2_app_kit::NSEvent,
                ),
                objc2::runtime::Imp,
            >(send_event),
            c"v@:@".as_ptr(),
        );
    }
}

#[cfg(target_os = "macos")]
static ORIGINAL_SEND_EVENT: std::sync::atomic::AtomicPtr<()> =
    std::sync::atomic::AtomicPtr::new(std::ptr::null_mut());

#[cfg(target_os = "macos")]
static DRAG_WINDOW: std::sync::atomic::AtomicPtr<objc2::runtime::AnyObject> =
    std::sync::atomic::AtomicPtr::new(std::ptr::null_mut());

#[cfg(target_os = "macos")]
unsafe extern "C-unwind" fn send_event(
    this: *mut objc2::runtime::AnyObject,
    cmd: objc2::runtime::Sel,
    event: *mut objc2_app_kit::NSEvent,
) {
    use std::sync::atomic::Ordering;

    use objc2_app_kit::{NSEventType, NSWindow};

    if owns_titlebar_drag(this as usize, DRAG_WINDOW.load(Ordering::Acquire) as usize)
        && !event.is_null()
    {
        let window = unsafe { &*this.cast::<NSWindow>() };
        let ns_event = unsafe { &*event };
        if ns_event.r#type() == NSEventType::LeftMouseDown
            && ns_event.clickCount() == 1
            && titlebar_drag_event(window, ns_event)
        {
            track_window_drag(window);
            return;
        }
    }
    let original = ORIGINAL_SEND_EVENT.load(Ordering::Acquire);
    if !original.is_null() {
        let original: unsafe extern "C-unwind" fn(
            *mut objc2::runtime::AnyObject,
            objc2::runtime::Sel,
            *mut objc2_app_kit::NSEvent,
        ) = unsafe { std::mem::transmute(original) };
        unsafe { original(this, cmd, event) };
    }
}

#[cfg(target_os = "macos")]
fn titlebar_drag_event(window: &objc2_app_kit::NSWindow, event: &objc2_app_kit::NSEvent) -> bool {
    if covers_traffic_light(window, event.locationInWindow()) {
        return false;
    }
    let Some(content) = window.contentView() else {
        return false;
    };
    let local = content.convertPoint_fromView(event.locationInWindow(), None);
    let bounds = content.bounds();
    let css_x = local.x - bounds.origin.x;
    let css_y = if content.isFlipped() {
        local.y - bounds.origin.y
    } else {
        bounds.origin.y + bounds.size.height - local.y
    };
    claim_current_titlebar(css_x, css_y)
}

#[cfg(target_os = "macos")]
fn covers_traffic_light(
    window: &objc2_app_kit::NSWindow,
    point: objc2_foundation::NSPoint,
) -> bool {
    use objc2_app_kit::NSWindowButton;

    for kind in [
        NSWindowButton::CloseButton,
        NSWindowButton::MiniaturizeButton,
        NSWindowButton::ZoomButton,
    ] {
        let Some(button) = window.standardWindowButton(kind) else {
            continue;
        };
        let rect = button.convertRect_toView(button.bounds(), None);
        if point.x >= rect.origin.x - 2.0
            && point.y >= rect.origin.y - 2.0
            && point.x < rect.origin.x + rect.size.width + 2.0
            && point.y < rect.origin.y + rect.size.height + 2.0
        {
            return true;
        }
    }
    false
}

#[cfg(target_os = "macos")]
static WEBVIEW: std::sync::atomic::AtomicPtr<std::ffi::c_void> =
    std::sync::atomic::AtomicPtr::new(std::ptr::null_mut());

#[cfg(target_os = "macos")]
fn current_page_key() -> Option<PageKey> {
    use objc2_web_kit::WKWebView;
    use std::sync::atomic::Ordering;

    let ptr = WEBVIEW.load(Ordering::Acquire);
    if ptr.is_null() {
        return None;
    }
    let webview = unsafe { &*ptr.cast::<WKWebView>() };
    let url = unsafe { webview.URL() }?;
    let absolute = url.absoluteString()?;
    let parsed = url::Url::parse(&absolute.to_string()).ok()?;
    page_key(&parsed)
}

#[cfg(target_os = "macos")]
fn register_drag_handler(window: &tauri::WebviewWindow) {
    use std::sync::atomic::{AtomicBool, Ordering};

    static REGISTERED: AtomicBool = AtomicBool::new(false);
    if REGISTERED.swap(true, Ordering::AcqRel) {
        return;
    }
    let _ = window.with_webview(|platform| {
        let view = platform.inner();
        if !view.is_null() {
            WEBVIEW.store(view, Ordering::Release);
        }
        let controller = platform.controller();
        if controller.is_null() {
            return;
        }
        let Some(mtm) = objc2::MainThreadMarker::new() else {
            return;
        };
        let controller = unsafe { &*controller.cast::<objc2_web_kit::WKUserContentController>() };
        attach_drag_handler(controller, mtm);
    });
}

#[cfg(target_os = "macos")]
fn attach_drag_handler(
    controller: &objc2_web_kit::WKUserContentController,
    mtm: objc2::MainThreadMarker,
) {
    use std::panic::AssertUnwindSafe;

    use objc2::msg_send;
    use objc2::runtime::ProtocolObject;
    use objc2_foundation::NSString;

    let allocated = mtm.alloc::<DragMessageHandler>().set_ivars(());
    let handler: objc2::rc::Retained<DragMessageHandler> =
        unsafe { msg_send![super(allocated), init] };
    let proto = ProtocolObject::from_ref(&*handler);
    let name = NSString::from_str("a3sDrag");
    let _ = objc2::exception::catch(AssertUnwindSafe(|| unsafe {
        controller.addScriptMessageHandler_name(proto, &name);
    }));
    std::mem::forget(handler);
}

#[cfg(target_os = "macos")]
mod drag_handler {
    use objc2::MainThreadOnly;
    use objc2::define_class;
    use objc2::runtime::NSObjectProtocol;
    use objc2_foundation::NSObject as FoundationNSObject;
    use objc2_web_kit::{WKScriptMessage, WKScriptMessageHandler, WKUserContentController};

    define_class!(
        #[unsafe(super(FoundationNSObject))]
        #[thread_kind = MainThreadOnly]
        #[ivars = ()]
        pub(super) struct DragMessageHandler;

        unsafe impl NSObjectProtocol for DragMessageHandler {}

        unsafe impl WKScriptMessageHandler for DragMessageHandler {
            #[unsafe(method(userContentController:didReceiveScriptMessage:))]
            fn did_receive(
                _this: &DragMessageHandler,
                _controller: &WKUserContentController,
                message: &WKScriptMessage,
            ) {
                super::receive_drag_message(message);
            }
        }
    );
}

#[cfg(target_os = "macos")]
use drag_handler::DragMessageHandler;

#[cfg(target_os = "macos")]
mod drag_strip {
    use objc2::MainThreadMarker;
    use objc2::MainThreadOnly;
    use objc2::Message;
    use objc2::define_class;
    use objc2::rc::Retained;
    use objc2_app_kit::{NSEvent, NSView};
    use objc2_foundation::{NSPoint, NSRect, NSSize};

    define_class!(
        #[unsafe(super(NSView))]
        #[thread_kind = MainThreadOnly]
        #[ivars = ()]
        pub(super) struct TitlebarDragView;

        impl TitlebarDragView {
            #[unsafe(method(mouseDown:))]
            fn mouse_down(&self, event: &NSEvent) {
                if event.clickCount() != 1 {
                    return;
                }
                let Some(window) = self.window() else {
                    return;
                };
                super::track_window_drag(&window);
            }

            #[unsafe(method_id(hitTest:))]
            fn hit_test(&self, point: NSPoint) -> Option<Retained<NSView>> {
                claimed_strip(self, point)
            }

            #[unsafe(method(isOpaque))]
            fn is_opaque(&self) -> bool {
                false
            }

            #[unsafe(method(acceptsFirstMouse:))]
            fn accepts_first_mouse(&self, _event: Option<&NSEvent>) -> bool {
                true
            }

            #[unsafe(method(mouseDownCanMoveWindow))]
            fn mouse_down_can_move_window(&self) -> bool {
                false
            }

            #[unsafe(method(viewDidMoveToWindow))]
            fn view_did_move_to_window(&self) {
                super::place_drag_strip(self);
            }

            #[unsafe(method(resizeWithOldSuperviewSize:))]
            fn resize_with_old_superview_size(&self, _old: NSSize) {
                super::place_drag_strip(self);
            }
        }
    );

    fn claimed_strip(view: &TitlebarDragView, point: NSPoint) -> Option<Retained<NSView>> {
        let frame = view.frame();
        let inside = point.x >= frame.origin.x
            && point.y >= frame.origin.y
            && point.x < frame.origin.x + frame.size.width
            && point.y < frame.origin.y + frame.size.height;
        if !inside {
            return None;
        }
        let superview = unsafe { view.superview() }?;
        let local = view.convertPoint_fromView(point, Some(&superview));
        let bounds = view.bounds();
        let css_x = local.x - bounds.origin.x;
        let css_y = if view.isFlipped() {
            local.y - bounds.origin.y
        } else {
            bounds.origin.y + bounds.size.height - local.y
        };
        if !super::claim_current_titlebar(css_x, css_y) {
            return None;
        }
        Some(view.retain().into_super())
    }

    impl TitlebarDragView {
        pub(super) fn new(mtm: MainThreadMarker) -> Retained<Self> {
            use objc2::msg_send;

            let allocated = mtm.alloc::<Self>().set_ivars(());
            unsafe { msg_send![super(allocated), initWithFrame: NSRect::ZERO] }
        }
    }
}

#[cfg(target_os = "macos")]
fn receive_drag_message(message: &objc2_web_kit::WKScriptMessage) {
    use objc2_foundation::NSString;

    #[derive(serde::Deserialize)]
    struct DragPayload {
        drag: Vec<[f64; 4]>,
        block: Vec<[f64; 4]>,
    }

    let frame = unsafe { message.frameInfo() };
    if !unsafe { frame.isMainFrame() } {
        return;
    }
    let request = unsafe { frame.request() };
    let Some(url) = request.URL() else {
        return;
    };
    let Some(absolute) = url.absoluteString() else {
        return;
    };
    let Ok(parsed) = url::Url::parse(&absolute.to_string()) else {
        return;
    };
    let Some(page) = page_key(&parsed) else {
        return;
    };
    let body = unsafe { message.body() };
    let Ok(text) = body.downcast::<NSString>() else {
        return;
    };
    let Ok(payload) = serde_json::from_str::<DragPayload>(&text.to_string()) else {
        return;
    };
    store_regions(Some(page), payload.drag, payload.block);
}

#[cfg(test)]
mod tests {
    use super::{
        CssRect, claim_titlebar_drag, hit_window_chrome, hit_window_drag, owns_titlebar_drag,
        page_key, rects_from,
    };

    #[test]
    fn applet_clicks_are_not_the_main_titlebar_drag() {
        assert!(!owns_titlebar_drag(2, 1));
        assert!(!owns_titlebar_drag(1, 0));
        assert!(owns_titlebar_drag(1, 1));
    }

    #[test]
    fn empty_chrome_does_not_drag() {
        assert!(!hit_window_drag(20.0, 10.0, &[], &[]));
    }

    #[test]
    fn blank_titlebar_drags_and_controls_do_not() {
        let drag = [CssRect {
            x: 0.0,
            y: 0.0,
            w: 800.0,
            h: 44.0,
        }];
        let block = [CssRect {
            x: 80.0,
            y: 8.0,
            w: 120.0,
            h: 28.0,
        }];
        assert!(hit_window_drag(400.0, 20.0, &drag, &block));
        assert!(!hit_window_drag(100.0, 20.0, &drag, &block));
        assert!(!hit_window_drag(400.0, 60.0, &drag, &block));
        assert!(hit_window_drag(200.0, 20.0, &drag, &block));
        assert!(!hit_window_drag(199.9, 20.0, &drag, &block));
    }

    #[test]
    fn unpublished_titlebar_ignores_stale_rectangles() {
        let stale = [CssRect {
            x: 60.0,
            y: 0.0,
            w: 400.0,
            h: 44.0,
        }];
        assert!(hit_window_chrome(10.0, 10.0, &stale, &[], false));
        assert!(!hit_window_chrome(10.0, 44.0, &stale, &[], false));
        assert!(!hit_window_chrome(10.0, 10.0, &stale, &[], true));
        assert!(hit_window_chrome(80.0, 10.0, &stale, &[], true));
    }

    #[test]
    fn top_band_drags_and_published_controls_do_not() {
        let block = [CssRect {
            x: 80.0,
            y: 8.0,
            w: 40.0,
            h: 20.0,
        }];
        assert!(claim_titlebar_drag(10.0, 10.0, &block, false));
        assert!(claim_titlebar_drag(90.0, 10.0, &block, false));
        assert!(!claim_titlebar_drag(90.0, 10.0, &block, true));
        assert!(claim_titlebar_drag(200.0, 10.0, &block, true));
        assert!(!claim_titlebar_drag(200.0, 44.0, &block, true));
        assert!(!claim_titlebar_drag(200.0, 80.0, &block, false));
    }

    #[test]
    fn page_key_keeps_loopback_document_identity() {
        let main = url::Url::parse("http://127.0.0.1:4318/windows/main/index.html?x=1").unwrap();
        let login = url::Url::parse("http://127.0.0.1:62544/").unwrap();
        assert_ne!(page_key(&main), page_key(&login));
        assert_eq!(
            page_key(&main).unwrap(),
            page_key(&url::Url::parse("http://127.0.0.1:4318/windows/main/index.html").unwrap())
                .unwrap()
        );
        assert!(page_key(&url::Url::parse("https://example.com/").unwrap()).is_none());
    }

    #[test]
    fn rects_drop_non_finite_and_empty_boxes() {
        let rects = rects_from(vec![
            [0.0, 0.0, 10.0, 10.0],
            [1.0, 1.0, 0.0, 10.0],
            [f64::NAN, 0.0, 10.0, 10.0],
            [0.0, 0.0, -4.0, 4.0],
        ]);
        assert_eq!(rects.len(), 1);
        assert_eq!(rects[0].w, 10.0);
    }
}
