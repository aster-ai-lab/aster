//! Desktop settings that have to reach the operating system: login item, tray, and sleep.
//! The renderer keeps the preference. This module applies it.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};

use serde::{Deserialize, Serialize};
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

pub(crate) mod menu_labels;

pub(crate) fn menu_language() -> String {
    STATE
        .lock()
        .ok()
        .and_then(|state| {
            state
                .as_ref()
                .map(|runtime| runtime.settings.language.clone())
        })
        .unwrap_or_else(chinese)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopSettings {
    pub launch_on_boot: bool,
    pub tray: bool,
    pub tray_on_close: bool,
    pub tray_on_launch: bool,
    #[serde(default)]
    pub prevent_sleep_when_busy: bool,
    #[serde(default = "chinese")]
    pub language: String,
}

fn chinese() -> String {
    "zh-CN".to_owned()
}

impl Default for DesktopSettings {
    fn default() -> Self {
        Self {
            launch_on_boot: false,
            tray: false,
            tray_on_close: false,
            tray_on_launch: false,
            prevent_sleep_when_busy: false,
            language: chinese(),
        }
    }
}

struct Runtime {
    slug: String,
    product: String,
    logo: PathBuf,
    settings: DesktopSettings,
    launch_hidden: bool,
}

static STATE: Mutex<Option<Runtime>> = Mutex::new(None);
static APP: Mutex<Option<AppHandle>> = Mutex::new(None);
static TRAY: Mutex<Option<tauri::tray::TrayIcon>> = Mutex::new(None);
static STATUS_TITLE: Mutex<String> = Mutex::new(String::new());
static SLEEP_ASSERTION: AtomicU32 = AtomicU32::new(0);

pub fn should_hide_on_close(tray: bool, tray_on_close: bool) -> bool {
    tray && tray_on_close
}

pub fn should_launch_hidden(tray: bool, tray_on_launch: bool) -> bool {
    tray && tray_on_launch
}

pub fn settings_from_body(body: &[u8]) -> Result<DesktopSettings, u16> {
    let value: serde_json::Value = serde_json::from_slice(body).map_err(|_| 400u16)?;
    let flag = |key: &str| match value.get(key) {
        Some(item) if item.is_boolean() => Ok(item.as_bool().unwrap_or(false)),
        _ => Err(400u16),
    };
    let language = match value.get("language").and_then(|item| item.as_str()) {
        Some("en-US") => "en-US",
        _ => "zh-CN",
    };
    Ok(DesktopSettings {
        launch_on_boot: flag("launchOnBoot")?,
        tray: flag("tray")?,
        tray_on_close: flag("trayOnClose")?,
        tray_on_launch: flag("trayOnLaunch")?,
        prevent_sleep_when_busy: flag("preventSleepWhenBusy")?,
        language: language.to_owned(),
    })
}

pub fn launch_agent_label(slug: &str) -> Option<String> {
    if !slug_ok(slug) {
        return None;
    }
    Some(format!("dev.a3s.studio.{slug}"))
}

pub fn launch_agent_document(label: &str, executable: &str, slug: &str) -> String {
    let instance = crate::tenant::instance_name().ok().flatten();
    let (tenant, instance_name) = login_environment(slug, instance.as_deref());
    let instance_keys = match instance_name.as_deref() {
        Some(name) => format!(
            "\n    <key>A3S_STUDIO_INSTANCE</key>\n    <string>{}</string>",
            xml_escape(name)
        ),
        None => String::new(),
    };
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>{}</string>
  <key>ProgramArguments</key>
  <array>
    <string>{}</string>
  </array>
  <key>EnvironmentVariables</key>
  <dict>
    <key>A3S_TENANT_SLUG</key>
    <string>{}</string>{instance_keys}
  </dict>
  <key>RunAtLoad</key>
  <true/>
  <key>LimitLoadToSessionType</key>
  <string>Aqua</string>
</dict>
</plist>
"#,
        xml_escape(label),
        xml_escape(executable),
        xml_escape(&tenant)
    )
}

/// A side process logs in as the brand tenant. The instance stays in `A3S_STUDIO_INSTANCE`.
fn login_environment(slug: &str, instance: Option<&str>) -> (String, Option<String>) {
    let Some(instance) = instance else {
        return (slug.to_owned(), None);
    };
    let suffix = format!("-{instance}");
    match slug.strip_suffix(&suffix) {
        Some(tenant) if !tenant.is_empty() => (tenant.to_owned(), Some(instance.to_owned())),
        _ => (slug.to_owned(), Some(instance.to_owned())),
    }
}

pub fn sync_launch_agent(
    agents_dir: &Path,
    slug: &str,
    executable: &str,
    enabled: bool,
) -> io::Result<()> {
    let Some(label) = launch_agent_label(slug) else {
        return Ok(());
    };
    let path = agents_dir.join(format!("{label}.plist"));
    if !enabled {
        if path.exists() {
            fs::remove_file(path)?;
        }
        return Ok(());
    }
    fs::create_dir_all(agents_dir)?;
    fs::write(path, launch_agent_document(&label, executable, slug))
}

/// Read the saved file and remember it. The tray waits until [`attach`].
pub fn boot(slug: &str, product: &str, logo: &Path) {
    let settings = load(slug);
    let launch_hidden = should_launch_hidden(settings.tray, settings.tray_on_launch);
    {
        let mut guard = STATE.lock().expect("desktop settings");
        *guard = Some(Runtime {
            slug: slug.to_owned(),
            product: product.to_owned(),
            logo: logo.to_owned(),
            settings: settings.clone(),
            launch_hidden,
        });
    }
    sync_os(&settings, slug);
}

pub fn attach(app: AppHandle) {
    *APP.lock().expect("desktop app") = Some(app);
    refresh_tray();
}

pub fn update(settings: DesktopSettings) {
    let (slug, tray_changed) = {
        let mut guard = STATE.lock().expect("desktop settings");
        let runtime = guard.get_or_insert_with(|| Runtime {
            slug: String::new(),
            product: String::new(),
            logo: PathBuf::new(),
            settings: DesktopSettings::default(),
            launch_hidden: false,
        });
        let tray_changed = runtime.settings.tray != settings.tray
            || runtime.settings.language != settings.language;
        runtime.settings = settings.clone();
        (runtime.slug.clone(), tray_changed)
    };
    if !slug.is_empty() {
        save(&slug, &settings);
        sync_os(&settings, &slug);
    }
    // The renderer posts on every preference sync. Rebuilding an unchanged
    // status item makes it flicker.
    if tray_changed {
        refresh_tray();
    }
}

pub fn hide_on_close() -> bool {
    let enabled = STATE
        .lock()
        .expect("desktop settings")
        .as_ref()
        .is_some_and(|runtime| {
            should_hide_on_close(runtime.settings.tray, runtime.settings.tray_on_close)
        });
    enabled && tray_installed()
}

/// The first main-renderer load stays hidden when launch-to-tray was on at startup.
pub fn take_launch_hidden() -> bool {
    if !tray_installed() {
        return false;
    }
    let mut guard = STATE.lock().expect("desktop settings");
    let Some(runtime) = guard.as_mut() else {
        return false;
    };
    if !runtime.launch_hidden {
        return false;
    }
    runtime.launch_hidden = false;
    true
}

fn tray_installed() -> bool {
    TRAY.lock().expect("desktop tray").is_some()
}

fn sync_os(settings: &DesktopSettings, slug: &str) {
    if let (Some(dir), Ok(executable)) = (launch_agents_dir(), std::env::current_exe()) {
        let _ = sync_launch_agent(
            dir.as_path(),
            slug,
            &executable.to_string_lossy(),
            settings.launch_on_boot,
        );
    }
    let product = STATE
        .lock()
        .expect("desktop settings")
        .as_ref()
        .map(|runtime| runtime.product.clone())
        .unwrap_or_else(|| "ToWork".to_owned());
    apply_sleep(settings.prevent_sleep_when_busy, &product);
}

fn refresh_tray() {
    let app = APP.lock().expect("desktop app").clone();
    let Some(app) = app else {
        return;
    };
    // The menu and the status item have to be created on the main thread.
    // Queue the work and return. Waiting here deadlocks: the page-load
    // callback also runs on the main thread and takes the tray lock.
    let _ = app.run_on_main_thread(refresh_tray_on_main);
}

fn refresh_tray_on_main() {
    let runtime = STATE.lock().expect("desktop settings").clone();
    let app = APP.lock().expect("desktop app").clone();
    let (Some(runtime), Some(app)) = (runtime, app) else {
        return;
    };
    let title = status_title();
    // The app keeps every tray it built. Dropping the handle leaves the old
    // status item in the menu bar, so remove it by id before adding the new one.
    let mut slot = TRAY.lock().expect("desktop tray");
    slot.take();
    let _ = app.remove_tray_by_id("desktop");
    // A count still needs a status item when the tray preference is off.
    // Close-to-tray stays on that preference.
    if status_item_wanted(runtime.settings.tray, &title) {
        *slot = build_tray(&app, &runtime, &title);
    }
}

fn status_item_wanted(tray: bool, title: &str) -> bool {
    tray || !title.is_empty()
}

fn status_title() -> String {
    STATUS_TITLE.lock().expect("status title").clone()
}

/// Menu-bar title beside the status icon. An empty title clears the number.
pub(crate) fn set_status_title(title: &str) {
    let refresh = {
        let mut guard = STATUS_TITLE.lock().expect("status title");
        let crossed_zero = guard.is_empty() != title.is_empty();
        *guard = title.to_owned();
        crossed_zero
    };
    if refresh {
        refresh_tray();
        return;
    }
    let app = APP.lock().expect("desktop app").clone();
    let Some(app) = app else {
        return;
    };
    let _ = app.run_on_main_thread(apply_status_title_on_main);
}

fn apply_status_title_on_main() {
    let title = status_title();
    let slot = TRAY.lock().expect("desktop tray");
    let Some(tray) = slot.as_ref() else {
        return;
    };
    let _ = tray.set_title(if title.is_empty() { None } else { Some(title) });
}

fn build_tray(app: &AppHandle, runtime: &Runtime, title: &str) -> Option<tauri::tray::TrayIcon> {
    let (show, quit) = if runtime.settings.language == "en-US" {
        ("Show", "Quit")
    } else {
        ("显示主窗口", "退出")
    };
    let show_item = MenuItem::with_id(app, "desktop-tray-show", show, true, None::<&str>).ok()?;
    let quit_item = MenuItem::with_id(app, "desktop-tray-quit", quit, true, None::<&str>).ok()?;
    let menu = Menu::with_items(app, &[&show_item, &quit_item]).ok()?;
    #[cfg(target_os = "macos")]
    let (icon, template) = match tray_template_image(&runtime.logo) {
        Some(icon) => (icon, true),
        None => (tray_image(&runtime.logo), false),
    };
    #[cfg(not(target_os = "macos"))]
    let (icon, template) = (tray_image(&runtime.logo), false);
    let handle = app.clone();
    let mut builder = TrayIconBuilder::with_id("desktop")
        .tooltip(&runtime.product)
        .icon(icon)
        .icon_as_template(template)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "desktop-tray-show" => show_main(app),
            "desktop-tray-quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(move |_tray, event| {
            let TrayIconEvent::Click {
                button,
                button_state,
                ..
            } = event
            else {
                return;
            };
            if button != MouseButton::Left || button_state != MouseButtonState::Up {
                return;
            }
            show_main(&handle);
            crate::message_attention::emit_open(&handle, None);
        });
    if !title.is_empty() {
        builder = builder.title(title);
    }
    builder.build(app).ok()
}

pub(crate) fn show_main(app: &AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let _ = window.unminimize();
    let _ = window.show();
    let _ = window.set_focus();
}

fn tray_image(logo: &Path) -> Image<'static> {
    Image::from_path(logo)
        .map(|image| image.to_owned())
        .unwrap_or_else(|_| Image::new_owned(vec![0x17, 0x17, 0x17, 0xff].repeat(16 * 16), 16, 16))
}

/// The menu bar tints template images itself, so only the glyph's alpha is kept.
/// A tenant whose wordmark is too wide for the menu bar ships a square `tray.png`.
#[cfg(target_os = "macos")]
fn tray_template_image(logo: &Path) -> Option<Image<'static>> {
    let mark = Image::from_path(logo.with_file_name("tray.png"))
        .or_else(|_| Image::from_path(logo.with_file_name("mark-light.png")))
        .ok()?;
    let (rgba, width, height) = template_glyph(mark.rgba(), mark.width(), mark.height(), 32, 112)?;
    Some(Image::new_owned(rgba, width, height))
}

/// Crops `rgba` to its opaque pixels and area-averages the alpha into a black
/// glyph at most `max_height` x `max_width` pixels (16pt x 56pt at 2x).
fn template_glyph(
    rgba: &[u8],
    width: u32,
    height: u32,
    max_height: u32,
    max_width: u32,
) -> Option<(Vec<u8>, u32, u32)> {
    let (width, height) = (width as usize, height as usize);
    if width == 0 || height == 0 || rgba.len() < width * height * 4 {
        return None;
    }
    let alpha = |x: usize, y: usize| rgba[(y * width + x) * 4 + 3];
    let (mut left, mut top, mut right, mut bottom) = (width, height, 0, 0);
    for y in 0..height {
        for x in 0..width {
            if alpha(x, y) > 8 {
                left = left.min(x);
                top = top.min(y);
                right = right.max(x + 1);
                bottom = bottom.max(y + 1);
            }
        }
    }
    if left >= right || top >= bottom {
        return None;
    }
    let (crop_w, crop_h) = (right - left, bottom - top);
    let scale = (max_height as f64 / crop_h as f64).min(max_width as f64 / crop_w as f64);
    let out_w = ((crop_w as f64 * scale).round() as usize).max(1);
    let out_h = ((crop_h as f64 * scale).round() as usize).max(1);
    let mut out = vec![0u8; out_w * out_h * 4];
    for oy in 0..out_h {
        let y0 = top + oy * crop_h / out_h;
        let y1 = (top + (oy + 1) * crop_h / out_h).max(y0 + 1);
        for ox in 0..out_w {
            let x0 = left + ox * crop_w / out_w;
            let x1 = (left + (ox + 1) * crop_w / out_w).max(x0 + 1);
            let mut sum = 0u64;
            for y in y0..y1 {
                for x in x0..x1 {
                    sum += alpha(x, y) as u64;
                }
            }
            let count = ((y1 - y0) * (x1 - x0)) as u64;
            out[(oy * out_w + ox) * 4 + 3] = (sum / count) as u8;
        }
    }
    Some((out, out_w as u32, out_h as u32))
}

fn load(slug: &str) -> DesktopSettings {
    let Some(path) = settings_path(slug) else {
        return DesktopSettings::default();
    };
    let Ok(bytes) = fs::read(path) else {
        return DesktopSettings::default();
    };
    serde_json::from_slice(&bytes).unwrap_or_default()
}

fn save(slug: &str, settings: &DesktopSettings) {
    let Some(path) = settings_path(slug) else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(bytes) = serde_json::to_vec_pretty(settings) {
        let _ = fs::write(path, bytes);
    }
}

fn settings_path(slug: &str) -> Option<PathBuf> {
    Some(support_dir(slug)?.join("desktop-settings.json"))
}

fn support_dir(slug: &str) -> Option<PathBuf> {
    if !slug_ok(slug) {
        return None;
    }
    #[cfg(test)]
    {
        let instance = crate::tenant::instance_name().ok().flatten();
        Some(crate::tenant::profile_dir_under(
            &support_root(),
            slug,
            instance.as_deref(),
        ))
    }
    #[cfg(not(test))]
    crate::tenant::profile_dir(slug)
}

#[cfg(test)]
fn support_root() -> PathBuf {
    std::env::temp_dir().join("a3s-studio-desktop-settings-test")
}

fn launch_agents_dir() -> Option<PathBuf> {
    #[cfg(test)]
    {
        Some(std::env::temp_dir().join("a3s-studio-launch-agents-test"))
    }
    #[cfg(not(test))]
    {
        let home = std::env::var_os("HOME")?;
        Some(PathBuf::from(home).join("Library/LaunchAgents"))
    }
}

fn slug_ok(slug: &str) -> bool {
    (1..=64).contains(&slug.len())
        && slug
            .bytes()
            .all(|byte| matches!(byte, b'a'..=b'z' | b'0'..=b'9' | b'-'))
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(all(target_os = "macos", not(test)))]
fn apply_sleep(enabled: bool, product: &str) {
    use objc2_foundation::NSString;

    #[link(name = "IOKit", kind = "framework")]
    unsafe extern "C" {
        fn IOPMAssertionCreateWithName(
            assertion_type: *const std::ffi::c_void,
            assertion_level: u32,
            assertion_name: *const std::ffi::c_void,
            assertion_id: *mut u32,
        ) -> i32;
        fn IOPMAssertionRelease(assertion_id: u32) -> i32;
    }

    let current = SLEEP_ASSERTION.load(Ordering::Acquire);
    if enabled && current == 0 {
        let kind = NSString::from_str("PreventUserIdleSystemSleep");
        let reason = NSString::from_str(&format!("{product} is running a task"));
        let mut id = 0u32;
        let status = unsafe {
            IOPMAssertionCreateWithName(
                objc2::rc::Retained::as_ptr(&kind).cast(),
                255,
                objc2::rc::Retained::as_ptr(&reason).cast(),
                &mut id,
            )
        };
        if status == 0 && id != 0 {
            SLEEP_ASSERTION.store(id, Ordering::Release);
        }
        return;
    }
    if !enabled && current != 0 && SLEEP_ASSERTION.swap(0, Ordering::AcqRel) == current {
        unsafe { IOPMAssertionRelease(current) };
    }
}

#[cfg(not(all(target_os = "macos", not(test))))]
fn apply_sleep(enabled: bool, product: &str) {
    let _ = (enabled, product);
}

impl Clone for Runtime {
    fn clone(&self) -> Self {
        Self {
            slug: self.slug.clone(),
            product: self.product.clone(),
            logo: self.logo.clone(),
            settings: self.settings.clone(),
            launch_hidden: self.launch_hidden,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_glyph_crops_padding_fits_the_menu_bar_and_drops_color() {
        // 400x100 red bar inside a 1000x300 transparent canvas.
        let (w, h) = (1000usize, 300usize);
        let mut rgba = vec![0u8; w * h * 4];
        for y in 100..200 {
            for x in 300..700 {
                rgba[(y * w + x) * 4..][..4].copy_from_slice(&[0xd8, 0x0c, 0x18, 0xff]);
            }
        }
        let (out, out_w, out_h) = template_glyph(&rgba, w as u32, h as u32, 32, 112).unwrap();
        assert_eq!((out_w, out_h), (112, 28));
        assert!(
            out.chunks(4)
                .all(|px| px[..3] == [0, 0, 0] && px[3] == 0xff)
        );

        let (_, tall_w, tall_h) = template_glyph(&rgba[..], 1000, 300, 32, 1000).unwrap();
        assert_eq!((tall_w, tall_h), (128, 32));

        assert!(template_glyph(&vec![0u8; 16 * 16 * 4], 16, 16, 32, 112).is_none());
    }

    #[test]
    fn reads_a_complete_body_and_rejects_a_bad_one() {
        let body = br#"{"launchOnBoot":true,"tray":true,"trayOnClose":false,"trayOnLaunch":true,"preventSleepWhenBusy":true,"language":"en-US"}"#;
        let settings = settings_from_body(body).expect("settings");
        assert!(settings.launch_on_boot);
        assert!(settings.tray);
        assert!(!settings.tray_on_close);
        assert!(settings.tray_on_launch);
        assert!(settings.prevent_sleep_when_busy);
        assert_eq!(settings.language, "en-US");
        assert!(settings_from_body(b"{").is_err());
        assert!(settings_from_body(br#"{"launchOnBoot":"yes"}"#).is_err());
    }

    #[test]
    fn keeps_chinese_when_the_language_is_outside_the_pair() {
        let body = br#"{"launchOnBoot":false,"tray":false,"trayOnClose":false,"trayOnLaunch":false,"preventSleepWhenBusy":false,"language":"ja-JP"}"#;
        let settings = settings_from_body(body).expect("settings");
        assert_eq!(settings.language, "zh-CN");
    }

    #[test]
    fn writes_a_login_plist_and_removes_it() {
        let dir = std::env::temp_dir().join(format!("a3s-launch-agent-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        sync_launch_agent(
            &dir,
            "toyou",
            "/Apps/ToWork & Co.app/Contents/MacOS/ToWork",
            true,
        )
        .expect("write");
        let path = dir.join("dev.a3s.studio.toyou.plist");
        let text = fs::read_to_string(&path).expect("plist");
        assert!(text.contains("<key>RunAtLoad</key>"));
        assert!(text.contains("dev.a3s.studio.toyou"));
        assert!(text.contains("/Apps/ToWork &amp; Co.app/Contents/MacOS/ToWork"));
        assert!(text.contains("<key>A3S_TENANT_SLUG</key>"));
        assert!(text.contains("<string>toyou</string>"));
        assert!(!text.contains("A3S_STUDIO_INSTANCE"));
        let side = login_environment("toyou-fork1", Some("fork1"));
        assert_eq!(side.0, "toyou");
        assert_eq!(side.1.as_deref(), Some("fork1"));
        sync_launch_agent(&dir, "toyou", "/Apps/ToWork.app", false).expect("remove");
        assert!(!path.exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn hides_on_close_only_when_the_tray_is_on() {
        assert!(!should_hide_on_close(false, true));
        assert!(!should_hide_on_close(true, false));
        assert!(should_hide_on_close(true, true));
        assert!(!should_launch_hidden(true, false));
        assert!(should_launch_hidden(true, true));
    }

    #[test]
    fn shows_the_status_item_for_a_count_without_turning_on_the_tray() {
        assert!(!status_item_wanted(false, ""));
        assert!(status_item_wanted(false, "4"));
        assert!(status_item_wanted(true, ""));
        assert!(status_item_wanted(true, "99+"));
    }

    #[test]
    fn roundtrips_the_settings_file() {
        let slug = format!("t{}", std::process::id());
        let settings = DesktopSettings {
            launch_on_boot: true,
            tray: true,
            tray_on_close: true,
            tray_on_launch: false,
            prevent_sleep_when_busy: true,
            language: "en-US".to_owned(),
        };
        save(&slug, &settings);
        assert_eq!(load(&slug), settings);
        if let Some(path) = settings_path(&slug) {
            let _ = fs::remove_file(path);
        }
    }
}
