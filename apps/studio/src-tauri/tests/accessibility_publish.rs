//! The library test harness does not run on the main thread. This binary does.
//! Publishing accessibility must load the system selectors and leave this
//! process inactive, with the same activation policy it started with.

use a3s_studio_lib::publish_accessibility_windows;
use objc2::MainThreadMarker;
use objc2_app_kit::NSApplication;

fn main() {
    let marker = MainThreadMarker::new().expect("the test process main thread");
    let application = NSApplication::sharedApplication(marker);
    let policy = application.activationPolicy();
    let active = application.isActive();
    let published = publish_accessibility_windows();
    assert!(
        published.initialized,
        "NSApplication is missing _accessibilityInitialize"
    );
    assert!(
        published.enabled,
        "NSApplication is missing setAccessibilityEnabled:"
    );
    assert_eq!(application.activationPolicy(), policy);
    assert_eq!(application.isActive(), active);
    assert!(
        !application.isActive(),
        "publishing accessibility activated the process"
    );
}
