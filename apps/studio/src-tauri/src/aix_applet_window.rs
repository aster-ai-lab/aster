//! System window for an installed AIX applet, editor or ActivityBar container.
//!
//! The frame matches a workbench applet window. The page mounts only that
//! package's view; the view stays in its sandboxed frame inside the page.

use std::collections::{HashMap, HashSet};
use std::sync::{LazyLock, Mutex};

use serde::Serialize;
use sha2::{Digest, Sha256};
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

use crate::applet_window::AppletWindowHost;

const WINDOW_WIDTH: f64 = 1280.0;
const WINDOW_HEIGHT: f64 = 860.0;
const SEED_LIMIT: usize = 96 * 1024;
const SLOT_LIMIT: usize = 64;
const NOTICE_LIMIT: usize = 200;
/// The applet window pulls a staged seed. The script carries no seed bytes.
const HOME_SEED_PULL: &str = "window.dispatchEvent(new Event('a3s:aix-applet-home-seed'))";

/// Same closed forms as the AIX catalogue: package slug and view ID.
fn slug_ok(slug: &str) -> bool {
    identifier_ok(slug, 64)
}

fn view_ok(view: &str) -> bool {
    identifier_ok(view, 128)
}

fn identifier_ok(value: &str, limit: usize) -> bool {
    let mut bytes = value.bytes();
    bytes.next().is_some_and(|first| first.is_ascii_lowercase())
        && value.len() <= limit
        && bytes.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

pub fn is_aix_applet_label(label: &str) -> bool {
    let Some(rest) = label.strip_prefix("aix-applet-") else {
        return false;
    };
    rest.len() == 16 && rest.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn window_label(slug: &str, view: &str) -> String {
    let digest = Sha256::new()
        .chain_update(slug.as_bytes())
        .chain_update([0])
        .chain_update(view.as_bytes())
        .finalize();
    let hex: String = digest
        .iter()
        .take(8)
        .map(|byte| format!("{byte:02x}"))
        .collect();
    format!("aix-applet-{hex}")
}

fn page_url(slug: &str, view: &str, title: &str) -> Result<String, String> {
    if !slug_ok(slug) || !view_ok(view) {
        return Err("AIX mini app identity is invalid.".to_owned());
    }
    let mut url = crate::main_renderer_url(crate::packaged_shell())
        .parse::<url::Url>()
        .map_err(|_| "AIX mini app window URL is invalid.".to_owned())?;
    url.query_pairs_mut()
        .append_pair("aix-applet", "1")
        .append_pair("slug", slug)
        .append_pair("view", view)
        .append_pair("title", title);
    Ok(url.to_string())
}

fn normalize_title(title: &str, slug: &str) -> String {
    let cleaned: String = title
        .chars()
        .filter(|character| !character.is_control())
        .collect();
    let trimmed = cleaned.trim();
    if trimmed.is_empty() || trimmed.chars().count() > 120 {
        return slug.to_owned();
    }
    trimmed.to_owned()
}

/// One store per package: its windows share storage, other packages do not.
fn data_store_id(slug: &str) -> [u8; 16] {
    let digest = Sha256::digest(format!("aix-applet-{slug}").as_bytes());
    let mut id = [0u8; 16];
    id.copy_from_slice(&digest[..16]);
    id
}

#[derive(Default)]
struct HomeSlot {
    delivery: u64,
    pending: Option<String>,
    holds_work: bool,
    applied: Option<bool>,
    seed: Option<serde_json::Value>,
    notice: Option<String>,
    model_requests: u32,
}

struct HomeStore {
    next: u64,
    slots: HashMap<(String, String), HomeSlot>,
    recovery_windows: HashSet<String>,
}

impl Default for HomeStore {
    fn default() -> Self {
        Self {
            next: 0,
            slots: HashMap::new(),
            recovery_windows: HashSet::new(),
        }
    }
}

static HOME: LazyLock<Mutex<HomeStore>> = LazyLock::new(|| Mutex::new(HomeStore::default()));

struct HomeReport {
    slug: String,
    view: String,
    delivery: u64,
    holds_work: bool,
    applied: Option<bool>,
    seed: Option<serde_json::Value>,
    notice: Option<String>,
    model_requests: u32,
    has_applied: bool,
    has_seed: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HomeSeedTake {
    delivery: u64,
    seed: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HomeDeckFact {
    delivery: u64,
    holds_work: bool,
    applied: Option<bool>,
    seed: Option<serde_json::Value>,
    notice: Option<String>,
    model_requests: u32,
}

fn home_store() -> Result<std::sync::MutexGuard<'static, HomeStore>, String> {
    HOME.lock()
        .map_err(|_| "AIX mini app seed is unavailable.".to_owned())
}

#[cfg(test)]
pub(crate) static HOME_TEST_LOCK: Mutex<()> = Mutex::new(());

#[cfg(test)]
pub(crate) fn stage_home_seed_for_tests(slug: &str, view: &str, seed: &str) -> Result<u64, String> {
    stage_home_seed(slug, view, seed)
}

#[cfg(test)]
pub(crate) fn home_has_slug_for_tests(slug: &str) -> bool {
    home_store()
        .map(|store| store.slots.keys().any(|(name, _)| name == slug))
        .unwrap_or(false)
}

/// Drop the homepage seed and deck fact for one uninstalled plugin.
pub(crate) fn release_home_slug(slug: &str) {
    let Ok(mut store) = home_store() else {
        return;
    };
    let labels: Vec<String> = store
        .slots
        .keys()
        .filter(|(name, _)| name == slug)
        .map(|(name, view)| window_label(name, view))
        .collect();
    store.slots.retain(|(name, _), _| name != slug);
    for label in labels {
        store.recovery_windows.remove(&label);
    }
}

pub(crate) fn retire_home_window(label: &str) -> Result<(), String> {
    if !is_aix_applet_label(label) {
        return Ok(());
    }
    let mut store = home_store()?;
    store
        .slots
        .retain(|(slug, view), _| window_label(slug, view) != label);
    store.recovery_windows.remove(label);
    Ok(())
}

pub(crate) fn protects_recovery(label: &str) -> bool {
    HOME.lock()
        .map(|store| store.recovery_windows.contains(label))
        .unwrap_or(true)
}

#[tauri::command]
pub fn protect_aix_applet_window(window: tauri::WebviewWindow) -> Result<(), String> {
    if !is_aix_applet_label(window.label()) {
        return Err("AIX window is unavailable.".into());
    }
    home_store()?.recovery_windows.insert(window.label().into());
    Ok(())
}

#[tauri::command]
pub fn close_aix_applet_window(window: tauri::WebviewWindow) -> Result<(), String> {
    if !is_aix_applet_label(window.label()) {
        return Err("AIX window is unavailable.".into());
    }
    window
        .destroy()
        .map_err(|_| "AIX window could not close.".into())
}

fn identity_ok(slug: &str, view: &str) -> Result<(), String> {
    if slug_ok(slug) && view_ok(view) {
        Ok(())
    } else {
        Err("AIX mini app identity is invalid.".to_owned())
    }
}

fn validate_seed(seed: &str) -> Result<(), String> {
    if seed.len() > SEED_LIMIT {
        return Err("AIX mini app seed is invalid.".to_owned());
    }
    let value: serde_json::Value =
        serde_json::from_str(seed).map_err(|_| "AIX mini app seed is invalid.".to_owned())?;
    if !value.is_object() {
        return Err("AIX mini app seed is invalid.".to_owned());
    }
    Ok(())
}

fn stage_home_seed(slug: &str, view: &str, seed: &str) -> Result<u64, String> {
    validate_seed(seed)?;
    let mut store = home_store()?;
    let key = (slug.to_owned(), view.to_owned());
    if store.slots.len() >= SLOT_LIMIT && !store.slots.contains_key(&key) {
        return Err("AIX mini app seed is unavailable.".to_owned());
    }
    store.next = store.next.saturating_add(1);
    let delivery = store.next;
    let slot = store.slots.entry(key).or_default();
    slot.delivery = delivery;
    slot.pending = Some(seed.to_owned());
    slot.applied = None;
    slot.seed = None;
    slot.notice = None;
    slot.model_requests = 0;
    Ok(delivery)
}

fn take_home_seed(label: &str, slug: &str, view: &str) -> Result<Option<HomeSeedTake>, String> {
    identity_ok(slug, view)?;
    if label != window_label(slug, view) {
        return Err("This window cannot take the homepage seed.".to_owned());
    }
    let mut store = home_store()?;
    let Some(slot) = store.slots.get_mut(&(slug.to_owned(), view.to_owned())) else {
        return Ok(None);
    };
    let Some(seed) = slot.pending.take() else {
        return Ok(None);
    };
    Ok(Some(HomeSeedTake {
        delivery: slot.delivery,
        seed,
    }))
}

fn report_home_deck(label: &str, report: HomeReport) -> Result<(), String> {
    identity_ok(&report.slug, &report.view)?;
    if label != window_label(&report.slug, &report.view) {
        return Err("This window cannot report the homepage deck.".to_owned());
    }
    let mut store = home_store()?;
    let Some(slot) = store.slots.get_mut(&(report.slug, report.view)) else {
        return Ok(());
    };
    if report.delivery == 0 || report.delivery != slot.delivery {
        return Ok(());
    }
    slot.holds_work = report.holds_work;
    slot.model_requests = report.model_requests;
    slot.notice = report.notice.and_then(|notice| {
        let trimmed = notice.trim();
        if trimmed.is_empty() || trimmed.chars().count() > NOTICE_LIMIT {
            None
        } else {
            Some(trimmed.to_owned())
        }
    });
    if report.has_applied {
        if let Some(applied) = report.applied {
            slot.applied = Some(applied);
        }
    }
    if report.has_seed {
        if report
            .seed
            .as_ref()
            .is_some_and(serde_json::Value::is_object)
        {
            slot.seed = report.seed;
        }
    }
    Ok(())
}

fn read_home_deck(label: &str, slug: &str, view: &str) -> Result<Option<HomeDeckFact>, String> {
    identity_ok(slug, view)?;
    if label != "main" {
        return Err("This window cannot read the homepage deck.".to_owned());
    }
    let store = home_store()?;
    let Some(slot) = store.slots.get(&(slug.to_owned(), view.to_owned())) else {
        return Ok(None);
    };
    if slot.delivery == 0 {
        return Ok(None);
    }
    Ok(Some(HomeDeckFact {
        delivery: slot.delivery,
        holds_work: slot.holds_work,
        applied: slot.applied,
        seed: slot.seed.clone(),
        notice: slot.notice.clone(),
        model_requests: slot.model_requests,
    }))
}

#[cfg(test)]
fn reset_home_store() {
    *HOME.lock().expect("home store") = HomeStore::default();
}

#[tauri::command]
pub fn open_aix_applet_window(
    app: tauri::AppHandle,
    slug: String,
    view: String,
    title: String,
    seed: Option<String>,
) -> Result<u64, String> {
    let slug = slug.trim();
    let view = view.trim();
    identity_ok(slug, view)?;
    let delivery = match seed {
        Some(seed) => stage_home_seed(slug, view, &seed)?,
        None => 0,
    };
    let title = normalize_title(&title, slug);
    let label = window_label(slug, view);

    if let Some(existing) = app.get_webview_window(&label) {
        if delivery > 0 {
            let _ = existing.eval(HOME_SEED_PULL);
        }
        let _ = existing.unminimize();
        crate::applet_window::reveal(&existing);
        return Ok(delivery);
    }

    let url = page_url(slug, view, &title)?
        .parse::<url::Url>()
        .map_err(|_| "AIX mini app window URL is invalid.".to_owned())?;
    let host = app
        .try_state::<AppletWindowHost>()
        .ok_or_else(|| "AIX mini app window host is missing.".to_owned())?;
    let (tenant, product, runtime) = host.boot_context();
    drop(host);
    let mut script =
        crate::boot_initialization_script(true, false, &tenant, &product, &runtime, false);
    script.push_str("document.documentElement.dataset.a3sAixApplet='true';");

    let builder = crate::tenant::isolate_webview(
        WebviewWindowBuilder::new(&app, &label, WebviewUrl::External(url))
            .initialization_script(script)
            .data_store_identifier(data_store_id(slug)),
        data_store_id(slug),
    )
    .map_err(|error| error.to_string())?
    .title(title)
    .inner_size(WINDOW_WIDTH, WINDOW_HEIGHT)
    .min_inner_size(720.0, 520.0)
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
        if !is_aix_applet_label(window.label()) {
            return;
        }
        crate::applet_window::reveal(&window);
    });
    #[cfg(target_os = "macos")]
    let builder = builder
        .hidden_title(true)
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .accept_first_mouse(true);
    let window = builder
        .build()
        .map_err(|error| format!("Could not open the AIX mini app window: {error}"))?;
    crate::applet_window::reveal(&window);
    Ok(delivery)
}

#[tauri::command]
pub fn take_aix_applet_home_seed(
    window: tauri::WebviewWindow,
    slug: String,
    view: String,
) -> Result<Option<HomeSeedTake>, String> {
    take_home_seed(window.label(), slug.trim(), view.trim())
}

#[tauri::command]
pub fn report_aix_applet_home_deck(
    window: tauri::WebviewWindow,
    slug: String,
    view: String,
    delivery: u64,
    holds_work: bool,
    applied: Option<bool>,
    seed: Option<serde_json::Value>,
    notice: Option<String>,
    model_requests: u32,
    has_applied: bool,
    has_seed: bool,
) -> Result<(), String> {
    report_home_deck(
        window.label(),
        HomeReport {
            slug: slug.trim().to_owned(),
            view: view.trim().to_owned(),
            delivery,
            holds_work,
            applied,
            seed,
            notice,
            model_requests,
            has_applied,
            has_seed,
        },
    )
}

#[tauri::command]
pub fn read_aix_applet_home_deck(
    window: tauri::WebviewWindow,
    slug: String,
    view: String,
) -> Result<Option<HomeDeckFact>, String> {
    read_home_deck(window.label(), slug.trim(), view.trim())
}

#[cfg(test)]
mod tests {
    use super::{
        HOME_SEED_PULL, HomeReport, SEED_LIMIT, data_store_id, is_aix_applet_label,
        normalize_title, page_url, read_home_deck, report_home_deck, reset_home_store,
        stage_home_seed, take_home_seed, window_label,
    };

    #[test]
    fn identities_stay_in_the_catalogue_forms() {
        let url = page_url("a3s-g1-arm", "workbench", "宇树 G1").expect("url");
        assert!(url.contains("aix-applet=1"));
        assert!(url.contains("slug=a3s-g1-arm"));
        assert!(url.contains("view=workbench"));
        assert!(page_url("../g1", "workbench", "x").is_err());
        assert!(page_url("A3s", "workbench", "x").is_err());
        assert!(page_url("a3s-g1-arm", "work/bench", "x").is_err());
        assert!(page_url("a3s-g1-arm", "", "x").is_err());
        assert!(page_url(&format!("a{}", "b".repeat(64)), "workbench", "x").is_err());
    }

    #[test]
    fn one_window_per_view_and_one_store_per_package() {
        let label = window_label("a3s-g1-arm", "workbench");
        assert!(is_aix_applet_label(&label));
        assert_eq!(label, window_label("a3s-g1-arm", "workbench"));
        assert_ne!(label, window_label("a3s-cobot-arm", "workbench"));
        assert_ne!(window_label("ab", "c"), window_label("a", "bc"));
        assert!(!is_aix_applet_label("aix-applet-short"));
        assert!(!is_aix_applet_label("applet-leave"));
        assert_ne!(data_store_id("a3s-g1-arm"), data_store_id("a3s-cobot-arm"));
    }

    #[test]
    fn title_drops_control_characters() {
        assert_eq!(normalize_title(" 宇树 G1\n", "a3s-g1-arm"), "宇树 G1");
        assert_eq!(normalize_title("  ", "a3s-g1-arm"), "a3s-g1-arm");
    }

    fn seed_body() -> String {
        r#"{"content":"topic","n_slides":null}"#.to_owned()
    }

    fn with_store<T>(body: impl FnOnce() -> T) -> T {
        let _guard = super::HOME_TEST_LOCK.lock().expect("home test lock");
        reset_home_store();
        body()
    }

    #[test]
    fn release_home_slug_drops_that_package_and_leaves_another() {
        with_store(|| {
            stage_home_seed("a3s-presenton", "workbench", &seed_body()).expect("stage");
            stage_home_seed("a3s-other", "workbench", &seed_body()).expect("other");
            super::release_home_slug("a3s-presenton");
            let store = super::home_store().expect("store");
            assert!(store.slots.keys().all(|(slug, _)| slug != "a3s-presenton"));
            assert!(store.slots.keys().any(|(slug, _)| slug == "a3s-other"));
        });
    }

    fn report(
        label: &str,
        delivery: u64,
        holds_work: bool,
        applied: Option<bool>,
        has_applied: bool,
        seed: Option<serde_json::Value>,
        has_seed: bool,
    ) -> Result<(), String> {
        report_home_deck(
            label,
            HomeReport {
                slug: "a3s-presenton".to_owned(),
                view: "workbench".to_owned(),
                delivery,
                holds_work,
                applied,
                seed,
                notice: None,
                model_requests: 0,
                has_applied,
                has_seed,
            },
        )
    }

    #[test]
    fn home_seed_pull_script_carries_no_payload() {
        assert_eq!(
            HOME_SEED_PULL,
            "window.dispatchEvent(new Event('a3s:aix-applet-home-seed'))"
        );
        assert!(!HOME_SEED_PULL.contains('{'));
    }

    #[test]
    fn home_seed_rejects_a_non_object_and_is_taken_once() {
        with_store(|| {
            assert!(stage_home_seed("a3s-presenton", "workbench", "[]").is_err());
            assert!(stage_home_seed("a3s-presenton", "workbench", "\"topic\"").is_err());
            assert!(
                stage_home_seed(
                    "a3s-presenton",
                    "workbench",
                    &format!("{{\"a\":\"{}\"}}", "x".repeat(SEED_LIMIT))
                )
                .is_err()
            );
            let label = window_label("a3s-presenton", "workbench");
            let first = stage_home_seed("a3s-presenton", "workbench", &seed_body()).expect("stage");
            assert_eq!(first, 1);
            let taken = take_home_seed(&label, "a3s-presenton", "workbench")
                .expect("take")
                .expect("pending");
            assert_eq!(taken.delivery, 1);
            assert_eq!(taken.seed, seed_body());
            assert!(
                take_home_seed(&label, "a3s-presenton", "workbench")
                    .expect("second")
                    .is_none()
            );
            assert!(take_home_seed("main", "a3s-presenton", "workbench").is_err());
            assert!(
                take_home_seed("aix-applet-0123456789abcdef", "a3s-presenton", "workbench")
                    .is_err()
            );
            assert!(read_home_deck(&label, "a3s-presenton", "workbench").is_err());
        });
    }

    #[test]
    fn home_seed_report_keeps_applied_seed_and_ignores_a_stale_delivery() {
        with_store(|| {
            let label = window_label("a3s-presenton", "workbench");
            let first = stage_home_seed("a3s-presenton", "workbench", &seed_body()).expect("stage");
            let _ = take_home_seed(&label, "a3s-presenton", "workbench").expect("take");
            report(&label, first, false, None, false, None, false).expect("holds");
            let kept = serde_json::json!({"content": "topic", "n_slides": null});
            report(
                &label,
                first,
                false,
                Some(true),
                true,
                Some(kept.clone()),
                true,
            )
            .expect("applied");
            report(&label, first, true, None, false, None, false).expect("deck fact");
            let fact = read_home_deck("main", "a3s-presenton", "workbench")
                .expect("read")
                .expect("fact");
            assert!(fact.holds_work);
            assert_eq!(fact.applied, Some(true));
            assert_eq!(fact.seed, Some(kept));
            assert_eq!(fact.delivery, first);
            let second =
                stage_home_seed("a3s-presenton", "workbench", &seed_body()).expect("restage");
            assert_ne!(second, first);
            report(
                &label,
                first,
                false,
                Some(true),
                true,
                Some(serde_json::json!({"content":"stale"})),
                true,
            )
            .expect("stale");
            let cleared = read_home_deck("main", "a3s-presenton", "workbench")
                .expect("read")
                .expect("fact");
            assert_eq!(cleared.delivery, second);
            assert_eq!(cleared.applied, None);
            assert!(cleared.seed.is_none());
        });
    }
}

#[cfg(test)]
#[path = "aix_applet_window_tests.rs"]
mod lifecycle_tests;
