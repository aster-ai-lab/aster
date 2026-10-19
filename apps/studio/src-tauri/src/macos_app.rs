//! The Dock names an unpackaged binary after its file name and uses the icon
//! tauri-build embeds. Launch Services reads the product name and tenant icon
//! only when the process starts inside an app bundle.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::Duration;

use crate::tenant;

#[repr(C)]
struct ProcessSerialNumber {
    high_long_of_psn: u32,
    low_long_of_psn: u32,
}

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn TransformProcessType(psn: *mut ProcessSerialNumber, transform: u32) -> i32;
}

/// Carbon process transforms. Foreground is the policy that publishes AX windows.
const PROCESS_TRANSFORM_FOREGROUND: u32 = 1;
const PROCESS_TRANSFORM_BACKGROUND: u32 = 2;

/// The bundle owns the CG windows. The debug launcher does not.
pub(crate) fn process_transform(bundle_executable: bool) -> u32 {
    if bundle_executable {
        PROCESS_TRANSFORM_FOREGROUND
    } else {
        PROCESS_TRANSFORM_BACKGROUND
    }
}

/// No process type is applied before `NSApplication` exists.
pub(crate) fn transform_before_application(bundle_executable: bool) -> Option<u32> {
    let _ = bundle_executable;
    None
}

/// Publish this bundle to the accessibility server without activating it.
///
/// `setActivationPolicy` does not call `activateIgnoringOtherApps`. The Carbon
/// transform runs only after the shared application object exists, which is
/// the call that otherwise returns paramErr. An already-foreground bundle
/// returns paramErr and keeps the regular policy.
pub fn register_bundle_with_window_server() {
    let Ok(current) = std::env::current_exe() else {
        return;
    };
    if !is_app_executable(&current) {
        return;
    }
    let Some(marker) = objc2::MainThreadMarker::new() else {
        eprintln!("just studio: foreground registration is off the main thread");
        return;
    };
    let application = objc2_app_kit::NSApplication::sharedApplication(marker);
    // The transform has to precede the policy change. Reversing the calls
    // leaves a fresh process as a background app whose windows never join
    // the accessibility tree.
    let status = transform_current_process(process_transform(true));
    let applied =
        application.setActivationPolicy(objc2_app_kit::NSApplicationActivationPolicy::Regular);
    if status != 0 {
        eprintln!("just studio: foreground registration policy={applied} transform={status}");
    }
}

/// Whether the process loaded its accessibility implementation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AccessibilityPublish {
    pub initialized: bool,
    pub enabled: bool,
}

/// Load AppKit accessibility for windows that are already on screen.
///
/// This does not activate the application and does not order a window front.
/// A background launch otherwise answers `AXWindows` with the application
/// element, so a client cannot address the page controls.
pub fn publish_accessibility_windows() -> AccessibilityPublish {
    let published = publish_accessibility_windows_on_main();
    static LOGGED_OK: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if published.initialized {
        if !LOGGED_OK.swap(true, std::sync::atomic::Ordering::Relaxed) {
            eprintln!(
                "just studio: accessibility publish initialized=1 enabled={}",
                u8::from(published.enabled)
            );
        }
    } else {
        eprintln!("just studio: accessibility publish initialized=0 enabled=0");
    }
    published
}

fn publish_accessibility_windows_on_main() -> AccessibilityPublish {
    let Some(marker) = objc2::MainThreadMarker::new() else {
        return AccessibilityPublish {
            initialized: false,
            enabled: false,
        };
    };
    use objc2::ClassType;
    use objc2_foundation::NSObjectProtocol;
    let class = objc2_app_kit::NSApplication::class();
    let initialize = objc2::sel!(_accessibilityInitialize);
    let initialized = unsafe {
        let responds: bool = objc2::msg_send![class, respondsToSelector: initialize];
        if responds {
            let _: () = objc2::msg_send![class, _accessibilityInitialize];
        }
        responds
    };
    let application = objc2_app_kit::NSApplication::sharedApplication(marker);
    let enabled_selector = objc2::sel!(setAccessibilityEnabled:);
    let enabled = application.respondsToSelector(enabled_selector);
    if enabled {
        let _: () = unsafe { objc2::msg_send![&*application, setAccessibilityEnabled: true] };
    }
    // Web content captured its window token before this initialization, so
    // page controls report the application. Rebind them to the real window.
    bind_webview_accessibility_window();
    refresh_webview_accessibility_tokens();
    AccessibilityPublish {
        initialized,
        enabled,
    }
}

/// Web content reports the application as its window and parent.
/// Answer the view's window instead, without activating the application.
fn bind_webview_accessibility_window() {
    use std::sync::atomic::{AtomicBool, Ordering};

    static INSTALLED: AtomicBool = AtomicBool::new(false);
    if INSTALLED.swap(true, Ordering::AcqRel) {
        return;
    }
    use objc2::ClassType;
    use objc2::runtime::{AnyClass, Imp};
    use objc2_web_kit::WKWebView;

    let class: &AnyClass = WKWebView::class();
    let Some(method) = class.instance_method(objc2::sel!(accessibilityAttributeValue:)) else {
        INSTALLED.store(false, Ordering::Release);
        return;
    };
    let original = method.implementation();
    ORIGINAL_WEBVIEW_ATTRIBUTE.store(unsafe { std::mem::transmute(original) }, Ordering::Release);
    let replacement: Imp =
        unsafe { std::mem::transmute(webview_accessibility_attribute_value as AttributeValue) };
    unsafe { method.set_implementation(replacement) };
}

type AttributeValue = unsafe extern "C-unwind" fn(
    *mut objc2::runtime::AnyObject,
    objc2::runtime::Sel,
    *mut objc2::runtime::AnyObject,
) -> *mut objc2::runtime::AnyObject;

static ORIGINAL_WEBVIEW_ATTRIBUTE: std::sync::atomic::AtomicPtr<()> =
    std::sync::atomic::AtomicPtr::new(std::ptr::null_mut());

unsafe extern "C-unwind" fn webview_accessibility_attribute_value(
    this: *mut objc2::runtime::AnyObject,
    cmd: objc2::runtime::Sel,
    attribute: *mut objc2::runtime::AnyObject,
) -> *mut objc2::runtime::AnyObject {
    let original: AttributeValue = unsafe {
        std::mem::transmute(ORIGINAL_WEBVIEW_ATTRIBUTE.load(std::sync::atomic::Ordering::Relaxed))
    };
    let window: *mut objc2::runtime::AnyObject = unsafe { objc2::msg_send![this, window] };
    if window.is_null() {
        return unsafe { original(this, cmd, attribute) };
    }
    if unsafe {
        utf8_equals(attribute, b"AXWindow") || utf8_equals(attribute, b"AXTopLevelUIElement")
    } {
        return unsafe { retain_autoreleased(window) };
    }
    let current = unsafe { original(this, cmd, attribute) };
    if unsafe { utf8_equals(attribute, b"AXParent") && is_application_element(current) } {
        return unsafe { retain_autoreleased(window) };
    }
    current
}

unsafe fn retain_autoreleased(
    object: *mut objc2::runtime::AnyObject,
) -> *mut objc2::runtime::AnyObject {
    let _: *mut objc2::runtime::AnyObject = unsafe { objc2::msg_send![object, retain] };
    let _: *mut objc2::runtime::AnyObject = unsafe { objc2::msg_send![object, autorelease] };
    object
}

unsafe fn utf8_equals(object: *mut objc2::runtime::AnyObject, expected: &[u8]) -> bool {
    if object.is_null() {
        return false;
    }
    let bytes: *const i8 = unsafe { objc2::msg_send![object, UTF8String] };
    if bytes.is_null() {
        return false;
    }
    unsafe { std::ffi::CStr::from_ptr(bytes) }.to_bytes() == expected
}

unsafe fn is_application_element(element: *mut objc2::runtime::AnyObject) -> bool {
    if element.is_null() {
        return true;
    }
    let name = c"AXRole".as_ptr();
    let role_name: *mut objc2::runtime::AnyObject =
        unsafe { objc2::msg_send![objc2::class!(NSString), stringWithUTF8String: name] };
    let role: *mut objc2::runtime::AnyObject =
        unsafe { objc2::msg_send![element, accessibilityAttributeValue: role_name] };
    unsafe { utf8_equals(role, b"AXApplication") }
}

fn refresh_webview_accessibility_tokens() {
    let Some(marker) = objc2::MainThreadMarker::new() else {
        return;
    };
    use objc2::ClassType;
    use objc2_web_kit::WKWebView;

    let application = objc2_app_kit::NSApplication::sharedApplication(marker);
    let webview_class = WKWebView::class();
    let mut refreshed = 0usize;
    let windows = application.windows();
    for index in 0..windows.count() {
        let window = windows.objectAtIndex(index);
        if let Some(content) = window.contentView() {
            refreshed += refresh_webview_tokens(&content, webview_class);
        }
    }
    if refreshed > 0 {
        static LOGGED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        if !LOGGED.swap(true, std::sync::atomic::Ordering::Relaxed) {
            eprintln!("just studio: accessibility webview tokens refreshed={refreshed}");
        }
    }
}

fn refresh_webview_tokens(
    view: &objc2_app_kit::NSView,
    webview_class: &objc2::runtime::AnyClass,
) -> usize {
    use objc2_foundation::NSObjectProtocol;

    let mut count = 0usize;
    if view.isKindOfClass(webview_class) {
        let _: () = unsafe { objc2::msg_send![view, viewDidMoveToWindow] };
        count += 1;
    }
    let subviews = view.subviews();
    for index in 0..subviews.count() {
        let subview = subviews.objectAtIndex(index);
        count += refresh_webview_tokens(&subview, webview_class);
    }
    count
}

#[link(name = "proc")]
unsafe extern "C" {
    fn proc_pidpath(pid: i32, buffer: *mut i8, buffersize: u32) -> i32;
}

unsafe extern "C" {
    fn kill(pid: i32, sig: i32) -> i32;
}

/// When this process is the debug binary, install `<product>.app` and wait for
/// that bundle. The bundle process returns here and continues startup.
pub fn relaunch_into_tenant_bundle() {
    let current = match std::env::current_exe() {
        Ok(path) => path,
        Err(error) => {
            eprintln!("just studio: {error}");
            std::process::exit(1);
        }
    };
    let bundle = is_app_executable(&current);
    // TransformProcessType before NSApplication exists returns paramErr (-50).
    // A background parent then leaves the child's CG windows with no AXWindow.
    // The bundle registers on the main thread, after the application object
    // exists and before it creates a window. The launcher is not transformed.
    if let Some(kind) = transform_before_application(bundle) {
        let status = transform_current_process(kind);
        if status != 0 {
            eprintln!("just studio: TransformProcessType returned {status}");
        }
    }
    if bundle {
        return;
    }
    let brand = tenant::load().unwrap_or_else(|error| {
        eprintln!("just studio: {error}");
        std::process::exit(1);
    });
    let instance = tenant::instance_name().unwrap_or_else(|error| {
        eprintln!("just studio: {error}");
        std::process::exit(1);
    });
    if tenant::is_fork_request(instance.as_deref()) {
        relaunch_next_fork(&brand, &current);
    } else {
        finish_relaunch(&brand, instance.as_deref(), &current);
    }
}

/// Each `just fork` takes the next free `forkN` lock. A lost race tries the next id.
fn relaunch_next_fork(brand: &tenant::TenantBrand, current: &Path) {
    for number in 1..=10_000 {
        let id = format!("fork{number}");
        let storage = tenant::storage_slug_for(&brand.slug, Some(&id)).unwrap_or_else(|error| {
            eprintln!("just fork: {error}");
            std::process::exit(1);
        });
        if crate::single_instance::is_held(&storage) {
            continue;
        }
        // This process has no other threads yet. The bundle process inherits these.
        unsafe {
            std::env::set_var("A3S_STUDIO_INSTANCE", &id);
            std::env::set_var("A3S_STUDIO_FORK_ALLOC", "1");
            std::env::set_var(
                "STUDIO_RUNTIME_FILE",
                std::env::temp_dir().join(format!("a3s-studio-runtime-{}-{id}.json", brand.slug)),
            );
        }
        let exit = spawn_bundle(brand, Some(&id), current);
        if exit.status.code() == Some(crate::single_instance::FORK_CLAIM_LOST) {
            continue;
        }
        report_bundle_exit("just fork", &exit);
    }
    eprintln!("just fork: no free window");
    std::process::exit(1);
}

fn finish_relaunch(brand: &tenant::TenantBrand, instance: Option<&str>, current: &Path) {
    report_bundle_exit("just studio", &spawn_bundle(brand, instance, current));
}

struct BundleExit {
    status: std::process::ExitStatus,
    /// Launcher pid that took over this instance while the bundle ran.
    replaced_by: Option<u32>,
}

fn report_bundle_exit(command: &str, exit: &BundleExit) -> ! {
    use std::os::unix::process::ExitStatusExt;

    let status = exit.status;
    if status.success() {
        std::process::exit(0);
    }
    if let Some(pid) = exit.replaced_by {
        eprintln!("{command}: a newer launch (pid {pid}) replaced this window");
        std::process::exit(0);
    }
    match status.signal() {
        Some(15) | Some(9) => eprintln!(
            "{command}: bundle process {status}. Another process stopped it; no newer launch replaced it"
        ),
        _ => eprintln!("{command}: bundle process {status}"),
    }
    std::process::exit(status.code().unwrap_or(1));
}

const LAUNCHER_FILE: &str = "launcher.pid";

/// The newest launcher of an instance records its pid before it stops the others,
/// so a stopped launcher can tell a replacement from an outside kill.
fn claim_launcher(support: &Path, pid: u32) {
    let written = fs::create_dir_all(support)
        .and_then(|()| fs::write(support.join(LAUNCHER_FILE), pid.to_string()));
    if let Err(error) = written {
        eprintln!(
            "just studio: cannot record this launch in {}: {error}",
            support.display()
        );
    }
}

fn replacing_launcher(support: &Path, pid: u32) -> Option<u32> {
    let recorded: u32 = fs::read_to_string(support.join(LAUNCHER_FILE))
        .ok()?
        .trim()
        .parse()
        .ok()?;
    (recorded != pid).then_some(recorded)
}

fn spawn_bundle(brand: &tenant::TenantBrand, instance: Option<&str>, current: &Path) -> BundleExit {
    let storage = tenant::storage_slug_for(&brand.slug, instance).unwrap_or_else(|error| {
        eprintln!("just studio: {error}");
        std::process::exit(1);
    });
    let product = tenant::window_product(&brand.product, instance);
    let support = instance_support(&storage, instance).unwrap_or_else(|error| {
        eprintln!("just studio: {error}");
        std::process::exit(1);
    });
    let me = std::process::id();
    claim_launcher(&support, me);
    // Only this instance's bundle. The shared debug binary is also the primary
    // launcher, and another tenant's app lives in a different directory.
    stop_previous_instances(&support);
    let executable =
        install_bundle(brand, &storage, instance, &product, current).unwrap_or_else(|error| {
            eprintln!("just studio: {error}");
            std::process::exit(1);
        });
    let mut child = match Command::new(&executable)
        .env("A3S_STUDIO_APP_BUNDLE", "1")
        .args(std::env::args_os().skip(1))
        .spawn()
    {
        Ok(child) => child,
        Err(error) => {
            eprintln!("just studio: {error}");
            std::process::exit(1);
        }
    };
    match child.wait() {
        Ok(status) => BundleExit {
            status,
            replaced_by: replacing_launcher(&support, me),
        },
        Err(error) => {
            eprintln!("just studio: {error}");
            std::process::exit(1);
        }
    }
}

fn transform_current_process(kind: u32) -> i32 {
    // {0, 2} is kCurrentProcess.
    let mut process = ProcessSerialNumber {
        high_long_of_psn: 0,
        low_long_of_psn: 2,
    };
    unsafe { TransformProcessType(&mut process, kind) }
}

fn support_root() -> io::Result<PathBuf> {
    let home = std::env::var_os("HOME")
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "HOME is not set"))?;
    Ok(PathBuf::from(home).join("Library/Application Support/dev.a3s.studio"))
}

fn instance_support(storage: &str, instance: Option<&str>) -> io::Result<PathBuf> {
    if storage.is_empty()
        || storage.len() > 64
        || storage.split('-').any(|part| part.is_empty())
        || !storage
            .bytes()
            .all(|byte| matches!(byte, b'a'..=b'z' | b'0'..=b'9' | b'-'))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid studio storage id: {storage}"),
        ));
    }
    Ok(tenant::profile_dir_under(
        &support_root()?,
        storage,
        instance,
    ))
}

fn install_bundle(
    brand: &tenant::TenantBrand,
    storage: &str,
    instance: Option<&str>,
    product_name: &str,
    debug_exe: &Path,
) -> io::Result<PathBuf> {
    let product = bundle_product_name(product_name)?;
    let root = support_root()?;
    let legacy_exists = root.join(storage).is_dir();
    let support = instance_support(storage, instance)?;
    fs::create_dir_all(&support)?;
    let bundle = support.join(format!("{product}.app"));
    if let Ok(entries) = fs::read_dir(&support) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path != bundle && path.extension().and_then(|ext| ext.to_str()) == Some("app") {
                let _ = fs::remove_dir_all(path);
            }
        }
    }
    let contents = bundle.join("Contents");
    let macos = contents.join("MacOS");
    let resources = contents.join("Resources");
    fs::create_dir_all(&macos)?;
    fs::create_dir_all(&resources)?;
    let icon = brand.icon_path.with_file_name("icon.icns");
    if !icon.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("tenant icon {} was not found", icon.display()),
        ));
    }
    let version = icon_version(&icon);
    let identifier = tenant::bundle_identifier(storage, instance, legacy_exists);
    // Icon Services caches by bundle and icon file name. A new name per icon
    // version makes the Dock pick up a replaced tenant icon.
    let icon_name = format!("AppIcon-{version}");
    if let Ok(entries) = fs::read_dir(&resources) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with("AppIcon") && name.ends_with(".icns") {
                let _ = fs::remove_file(entry.path());
            }
        }
    }
    fs::write(
        contents.join("Info.plist"),
        info_plist(product, &identifier, &version, &icon_name),
    )?;
    fs::copy(&icon, resources.join(format!("{icon_name}.icns")))?;
    let executable = macos.join(product);
    install_executable(debug_exe, &executable)?;
    let _ = Command::new("/usr/bin/xattr")
        .args(["-cr"])
        .arg(&bundle)
        .status();
    codesign(&bundle, &identifier)?;
    let _ = Command::new(
        "/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister",
    )
    .args(["-f"])
    .arg(&bundle)
    .status();
    Ok(executable)
}

fn install_executable(source: &Path, dest: &Path) -> io::Result<()> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    let temporary = dest.with_extension("new");
    fs::copy(source, &temporary)?;
    let mut permissions = fs::metadata(&temporary)?.permissions();
    use std::os::unix::fs::PermissionsExt;
    permissions.set_mode(0o755);
    fs::set_permissions(&temporary, permissions)?;
    fs::rename(&temporary, dest)?;
    Ok(())
}

fn codesign(bundle: &Path, identifier: &str) -> io::Result<()> {
    let output = Command::new("/usr/bin/codesign")
        .args(["--force", "--sign", "-", "--identifier", identifier])
        .arg(bundle)
        .output()?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(io::Error::other(format!(
        "codesign exited with {}: {stderr}",
        output.status
    )))
}

fn icon_version(path: &Path) -> String {
    fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(|modified| modified.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs().to_string())
        .unwrap_or_else(|| "1".to_owned())
}

fn bundle_product_name(product: &str) -> io::Result<&str> {
    if product.is_empty() || product.contains(['/', '\0']) || product == "." || product == ".." {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "tenant product cannot name the app",
        ));
    }
    Ok(product)
}

fn info_plist(product: &str, identifier: &str, version: &str, icon_name: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDisplayName</key>
  <string>{product}</string>
  <key>CFBundleExecutable</key>
  <string>{product}</string>
  <key>CFBundleIconFile</key>
  <string>{icon_name}</string>
  <key>CFBundleIdentifier</key>
  <string>{identifier}</string>
  <key>CFBundleInfoDictionaryVersion</key>
  <string>6.0</string>
  <key>CFBundleName</key>
  <string>{product}</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleShortVersionString</key>
  <string>{short_version}</string>
  <key>CFBundleVersion</key>
  <string>{version}</string>
  <key>NSHighResolutionCapable</key>
  <true/>
  <key>NSPrincipalClass</key>
  <string>NSApplication</string>
</dict>
</plist>
"#,
        product = xml_text(product),
        identifier = xml_text(identifier),
        short_version = xml_text(env!("CARGO_PKG_VERSION")),
        version = xml_text(version),
        icon_name = xml_text(icon_name),
    )
}

fn xml_text(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

pub(crate) fn is_app_executable(path: &Path) -> bool {
    let Some(macos) = path.parent() else {
        return false;
    };
    let Some(contents) = macos.parent() else {
        return false;
    };
    let Some(bundle) = contents.parent() else {
        return false;
    };
    macos.file_name().and_then(|name| name.to_str()) == Some("MacOS")
        && contents.file_name().and_then(|name| name.to_str()) == Some("Contents")
        && bundle.extension().and_then(|ext| ext.to_str()) == Some("app")
}

fn is_instance_process(path: &Path, support: &Path) -> bool {
    // The .app directory's parent is this instance. A nested side window is not the primary.
    is_app_executable(path)
        && path
            .ancestors()
            .nth(4)
            .is_some_and(|parent| parent == support)
}

fn stop_previous_instances(support: &Path) {
    let me = std::process::id() as i32;
    let victims: Vec<i32> = process_ids()
        .into_iter()
        .filter(|pid| *pid != me)
        .filter_map(|pid| pid_path(pid).map(|path| (pid, path)))
        .filter(|(_, path)| is_instance_process(path, support))
        .map(|(pid, _)| pid)
        .collect();
    for pid in &victims {
        unsafe {
            kill(*pid, 15);
        }
    }
    for _ in 0..40 {
        if victims.iter().all(|pid| unsafe { kill(*pid, 0) } != 0) {
            return;
        }
        thread::sleep(Duration::from_millis(50));
    }
    for pid in &victims {
        unsafe {
            kill(*pid, 9);
        }
    }
    thread::sleep(Duration::from_millis(100));
}

fn process_ids() -> Vec<i32> {
    let Ok(output) = Command::new("/bin/ps").args(["-ax", "-o", "pid="]).output() else {
        return Vec::new();
    };
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.trim().parse().ok())
        .collect()
}

fn pid_path(pid: i32) -> Option<PathBuf> {
    let mut buffer = vec![0u8; 4096];
    let length = unsafe { proc_pidpath(pid, buffer.as_mut_ptr() as *mut i8, buffer.len() as u32) };
    if length <= 0 {
        return None;
    }
    let end = buffer
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(buffer.len());
    let text = String::from_utf8_lossy(&buffer[..end]);
    if text.is_empty() {
        None
    } else {
        Some(PathBuf::from(text.as_ref() as &str))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plist_uses_the_product_name_and_tenant_icon() {
        let plist = info_plist("ToWork", "dev.a3s.studio.toyou", "7", "AppIcon-7");
        assert!(plist.contains("<key>CFBundleDisplayName</key>\n  <string>ToWork</string>"));
        assert!(plist.contains("<key>CFBundleName</key>\n  <string>ToWork</string>"));
        assert!(plist.contains("<key>CFBundleExecutable</key>\n  <string>ToWork</string>"));
        assert!(plist.contains("<key>CFBundleIconFile</key>\n  <string>AppIcon-7</string>"));
        assert!(plist.contains("<string>dev.a3s.studio.toyou</string>"));
        assert!(plist.contains(&format!(
            "<key>CFBundleShortVersionString</key>\n  <string>{}</string>",
            env!("CARGO_PKG_VERSION")
        )));
        assert!(!plist.contains("Cherry"));
        assert!(!plist.contains(">Studio<"));
        let escaped = info_plist("A&B <C>", "dev.a3s.studio.a3s", "1", "AppIcon-1");
        assert!(escaped.contains("<string>A&amp;B &lt;C&gt;</string>"));
        assert!(bundle_product_name("A3S Desktop").is_ok());
        assert!(bundle_product_name("Kense Work").is_ok());
        assert!(bundle_product_name("../Studio").is_err());
    }

    #[test]
    fn bundle_paths_are_app_executables_and_cherry_studio_is_left_alone() {
        assert!(is_app_executable(Path::new(
            "/Users/x/Library/Application Support/dev.a3s.studio/toyou/ToWork.app/Contents/MacOS/ToWork"
        )));
        assert!(!is_app_executable(Path::new(
            "/Users/roylin/code/a3s/apps/studio/src-tauri/target/debug/Studio"
        )));
        let primary = Path::new("/Users/x/Library/Application Support/dev.a3s.studio/toyou");
        let side = Path::new("/Users/x/Library/Application Support/dev.a3s.studio/toyou-fork1");
        let primary_app = Path::new(
            "/Users/x/Library/Application Support/dev.a3s.studio/toyou/ToWork.app/Contents/MacOS/ToWork",
        );
        let side_app = Path::new(
            "/Users/x/Library/Application Support/dev.a3s.studio/toyou-fork1/ToWork.app/Contents/MacOS/ToWork",
        );
        let debug = Path::new("/Users/roylin/code/a3s/apps/studio/src-tauri/target/debug/Studio");
        assert!(is_instance_process(primary_app, primary));
        assert!(!is_instance_process(primary_app, side));
        assert!(is_instance_process(side_app, side));
        assert!(!is_instance_process(side_app, primary));
        assert!(!is_instance_process(debug, primary));
        assert!(!is_instance_process(debug, side));
        let nested =
            Path::new("/Users/x/Library/Application Support/dev.a3s.studio/acme/instances/side1");
        let nested_app = Path::new(
            "/Users/x/Library/Application Support/dev.a3s.studio/acme/instances/side1/ToWork.app/Contents/MacOS/ToWork",
        );
        assert!(is_instance_process(nested_app, nested));
        assert!(!is_instance_process(nested_app, primary));
        assert!(!is_instance_process(primary_app, nested));
        assert!(!is_instance_process(
            Path::new("/Applications/Cherry Studio.app/Contents/MacOS/Cherry Studio"),
            primary
        ));
        assert!(bundle_product_name("ToWork 2").is_ok());
    }

    #[test]
    fn the_bundle_registers_foreground_after_the_application_exists() {
        assert_eq!(process_transform(true), 1);
        assert_eq!(process_transform(false), 2);
        assert_eq!(transform_before_application(true), None);
        assert_eq!(transform_before_application(false), None);
    }

    #[test]
    fn publishing_accessibility_off_the_main_thread_does_nothing() {
        if objc2::MainThreadMarker::new().is_some() {
            return;
        }
        let published = publish_accessibility_windows();
        assert!(!published.initialized);
        assert!(!published.enabled);
    }

    #[test]
    fn a_newer_launch_is_told_apart_from_an_outside_kill() {
        let dir = tempfile::tempdir().expect("tempdir");
        let support = dir.path().join("toyou");
        assert_eq!(replacing_launcher(&support, 100), None);
        claim_launcher(&support, 100);
        assert_eq!(replacing_launcher(&support, 100), None);
        claim_launcher(&support, 200);
        assert_eq!(replacing_launcher(&support, 100), Some(200));
        assert_eq!(replacing_launcher(&support, 200), None);
        fs::write(support.join(LAUNCHER_FILE), "not a pid").expect("write");
        assert_eq!(replacing_launcher(&support, 100), None);
    }
}
