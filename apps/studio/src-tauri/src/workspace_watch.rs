//! Disk invalidation for one workspace root.
//!
//! The file manager holds one `text/event-stream` on
//! `GET /api/local-documents/changes`. A platform watcher is shared by every
//! stream on the same canonical root and dropped once it has been idle.
//! Events are directory hints. The client re-lists those directories. Overflow,
//! a lost cursor, or a path that cannot be expressed inside the root becomes a
//! full scan instead of a partial tree.

use std::collections::{BTreeSet, HashMap, VecDeque};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock, Weak};
use std::time::{Duration, Instant};

use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde_json::{Value, json};

const DEBOUNCE: Duration = Duration::from_millis(180);
const IDLE_AFTER: Duration = Duration::from_secs(45);
const LOG_LIMIT: usize = 32;
/// Dependency and build directories. The directory itself and its direct
/// children still invalidate a listing. Anything deeper is noise.
const NOISE_DIRECTORIES: &[&str] = &["node_modules", "target", "dist", "coverage"];

/// One subscriber on a workspace watch. The subscription stays alive until drop.
pub(crate) struct ChangePump {
    state: Arc<WatchState>,
    subscription: Subscription,
}

impl ChangePump {
    pub(crate) fn open(raw_root: &str) -> Result<Self, u16> {
        let state = watch_for(raw_root)?;
        let subscription = Subscription::attach(&state);
        Ok(Self {
            state,
            subscription,
        })
    }

    /// Wait for a revision newer than `after`. `Ok(None)` is a quiet timeout.
    pub(crate) fn poll(&self, after: u64, timeout: Duration) -> Result<Option<Value>, u16> {
        let _held = &self.subscription;
        let deadline = Instant::now() + timeout;
        let mut guard = lock_inner(&self.state.inner);
        loop {
            commit_due(&mut guard);
            if let Some(body) = snapshot(&guard, after) {
                return Ok(Some(body));
            }
            if Instant::now() >= deadline {
                return Ok(None);
            }
            let slice = next_wait(&guard, deadline);
            let waited = self.state.wake.wait_timeout(guard, slice);
            guard = match waited {
                Ok((next, _)) => next,
                Err(poisoned) => poisoned.into_inner().0,
            };
        }
    }
}

#[cfg(test)]
fn wait_for_changes(raw_root: &str, after: u64, timeout: Duration) -> Result<Value, u16> {
    let pump = ChangePump::open(raw_root)?;
    match pump.poll(after, timeout)? {
        Some(body) => Ok(body),
        None => Ok(keep_alive(lock_inner(&pump.state.inner).revision)),
    }
}

fn watch_for(raw_root: &str) -> Result<Arc<WatchState>, u16> {
    let root = crate::local_documents::canonical_workspace(raw_root)?;
    let retired = {
        let mut hub = lock_hub();
        let retired = take_idle(&mut hub, IDLE_AFTER);
        if let Some(existing) = hub.get(&root) {
            let state = Arc::clone(&existing.state);
            drop(hub);
            drop(retired);
            return Ok(state);
        }
        drop(hub);
        retired
    };
    drop(retired);

    let created = open_watch(root.clone())?;
    let mut hub = lock_hub();
    if let Some(existing) = hub.get(&root) {
        let state = Arc::clone(&existing.state);
        drop(hub);
        drop(created);
        return Ok(state);
    }
    let state = Arc::clone(&created.state);
    hub.insert(root, created);
    drop(hub);
    Ok(state)
}

fn open_watch(root: PathBuf) -> Result<WatchHandle, u16> {
    let state = Arc::new(WatchState {
        inner: Mutex::new(Inner::default()),
        wake: Condvar::new(),
        subscribers: AtomicUsize::new(0),
        idle_at: Mutex::new(None),
    });
    let weak = Arc::downgrade(&state);
    let watched = root.clone();
    let mut watcher = RecommendedWatcher::new(
        move |event| remember(weak.clone(), &watched, event),
        Config::default().with_follow_symlinks(false),
    )
    .map_err(|_| 503u16)?;
    watcher
        .watch(&root, RecursiveMode::Recursive)
        .map_err(|_| 503u16)?;
    Ok(WatchHandle {
        state,
        watcher: Some(watcher),
    })
}

fn remember(weak: Weak<WatchState>, root: &Path, event: Result<Event, notify::Error>) {
    let Some(state) = weak.upgrade() else {
        return;
    };
    let mut guard = lock_inner(&state.inner);
    absorb(root, event, &mut guard);
    state.wake.notify_all();
}

struct WatchHandle {
    state: Arc<WatchState>,
    watcher: Option<RecommendedWatcher>,
}

impl Drop for WatchHandle {
    fn drop(&mut self) {
        // Stop the platform watch before the shared state is dropped, and do
        // it on the caller thread. The callback only holds a weak reference.
        self.watcher.take();
    }
}

struct WatchState {
    inner: Mutex<Inner>,
    wake: Condvar,
    subscribers: AtomicUsize,
    idle_at: Mutex<Option<Instant>>,
}

struct Subscription {
    state: Arc<WatchState>,
}

impl Subscription {
    fn attach(state: &Arc<WatchState>) -> Self {
        state.subscribers.fetch_add(1, Ordering::AcqRel);
        let mut idle = lock_idle(&state.idle_at);
        *idle = None;
        Self {
            state: Arc::clone(state),
        }
    }
}

impl Drop for Subscription {
    fn drop(&mut self) {
        let previous = self.state.subscribers.fetch_sub(1, Ordering::AcqRel);
        if previous == 1 {
            let mut idle = lock_idle(&self.state.idle_at);
            *idle = Some(Instant::now());
        }
    }
}

#[derive(Default)]
struct Inner {
    revision: u64,
    records: VecDeque<Record>,
    pending_dirs: BTreeSet<String>,
    pending_full: bool,
    pending_started: Option<Instant>,
}

struct Record {
    revision: u64,
    paths: BTreeSet<String>,
    full_scan: bool,
}

fn absorb(root: &Path, event: Result<Event, notify::Error>, inner: &mut Inner) {
    let event = match event {
        Ok(event) => event,
        Err(_) => {
            inner.pending_full = true;
            arm(inner);
            return;
        }
    };
    if matches!(event.kind, EventKind::Access(_)) {
        return;
    }
    if event.need_rescan() || event.paths.is_empty() {
        inner.pending_full = true;
        arm(inner);
        if event.paths.is_empty() {
            return;
        }
    }
    let mut useful = inner.pending_full;
    for path in &event.paths {
        match refresh_directories(root, path) {
            Refresh::Ignore => {}
            Refresh::FullScan => {
                inner.pending_full = true;
                useful = true;
            }
            Refresh::Directories(dirs) => {
                inner.pending_dirs.extend(dirs);
                useful = true;
            }
        }
    }
    if useful {
        arm(inner);
    }
}

fn arm(inner: &mut Inner) {
    if inner.pending_started.is_none() {
        inner.pending_started = Some(Instant::now());
    }
}

fn commit_due(inner: &mut Inner) {
    let Some(started) = inner.pending_started else {
        return;
    };
    if started.elapsed() < DEBOUNCE {
        return;
    }
    commit(inner);
}

fn commit(inner: &mut Inner) {
    if inner.pending_dirs.is_empty() && !inner.pending_full {
        inner.pending_started = None;
        return;
    }
    inner.revision = inner.revision.saturating_add(1);
    inner.records.push_back(Record {
        revision: inner.revision,
        paths: std::mem::take(&mut inner.pending_dirs),
        full_scan: inner.pending_full,
    });
    inner.pending_full = false;
    inner.pending_started = None;
    while inner.records.len() > LOG_LIMIT {
        inner.records.pop_front();
    }
}

fn snapshot(inner: &Inner, after: u64) -> Option<Value> {
    if after > inner.revision || log_misses(inner, after) {
        return Some(json!({
            "revision": inner.revision,
            "paths": [],
            "fullScan": true,
        }));
    }
    if after == inner.revision {
        return None;
    }
    let mut paths = BTreeSet::new();
    let mut full_scan = false;
    for record in inner
        .records
        .iter()
        .filter(|record| record.revision > after)
    {
        paths.extend(record.paths.iter().cloned());
        full_scan |= record.full_scan;
    }
    Some(json!({
        "revision": inner.revision,
        "paths": paths.into_iter().collect::<Vec<_>>(),
        "fullScan": full_scan,
    }))
}

fn log_misses(inner: &Inner, after: u64) -> bool {
    if after >= inner.revision {
        return false;
    }
    match inner.records.front() {
        Some(record) => record.revision > after.saturating_add(1),
        None => true,
    }
}

#[cfg(test)]
fn keep_alive(revision: u64) -> Value {
    json!({
        "revision": revision,
        "paths": [],
        "fullScan": false,
    })
}

fn next_wait(inner: &Inner, deadline: Instant) -> Duration {
    let until_deadline = deadline.saturating_duration_since(Instant::now());
    let Some(started) = inner.pending_started else {
        return until_deadline;
    };
    DEBOUNCE
        .saturating_sub(started.elapsed())
        .min(until_deadline)
}

enum Refresh {
    Ignore,
    FullScan,
    Directories(Vec<String>),
}

/// Directories whose listing can change because `path` changed.
/// An empty string is the workspace root. Hidden subtrees are ignored.
fn refresh_directories(root: &Path, path: &Path) -> Refresh {
    let Ok(relative) = path.strip_prefix(root) else {
        return Refresh::FullScan;
    };
    if relative.as_os_str().is_empty() {
        return Refresh::FullScan;
    }
    let mut parts = Vec::new();
    for component in relative.components() {
        let Component::Normal(value) = component else {
            return Refresh::FullScan;
        };
        let Some(text) = value.to_str() else {
            return Refresh::FullScan;
        };
        if text.is_empty() || text.contains('/') || text.contains('\\') {
            return Refresh::FullScan;
        }
        if text.starts_with('.') {
            // Explicitly opened hidden files still invalidate their parent without publishing a name.
            if relative.components().count() == 1 {
                return Refresh::Directories(vec![String::new()]);
            }
            return Refresh::Ignore;
        }
        parts.push(text);
    }
    if parts.is_empty() {
        return Refresh::FullScan;
    }
    if let Some(index) = parts
        .iter()
        .position(|part| NOISE_DIRECTORIES.contains(part))
    {
        // `node_modules/pkg` still refreshes `node_modules`. A file under that
        // package does not. The same shape applies to target, dist, and coverage.
        if parts.len() - index > 2 {
            return Refresh::Ignore;
        }
    }
    let mut dirs = Vec::new();
    if parts.len() == 1 {
        dirs.push(String::new());
    } else {
        dirs.push(parts[..parts.len() - 1].join("/"));
    }
    if path.is_dir() {
        dirs.push(parts.join("/"));
    }
    Refresh::Directories(dirs)
}

fn hub() -> &'static Mutex<HashMap<PathBuf, WatchHandle>> {
    static HUB: OnceLock<Mutex<HashMap<PathBuf, WatchHandle>>> = OnceLock::new();
    HUB.get_or_init(|| Mutex::new(HashMap::new()))
}

fn lock_hub() -> MutexGuard<'static, HashMap<PathBuf, WatchHandle>> {
    hub().lock().unwrap_or_else(|err| err.into_inner())
}

fn lock_inner(mutex: &Mutex<Inner>) -> MutexGuard<'_, Inner> {
    mutex.lock().unwrap_or_else(|err| err.into_inner())
}

fn lock_idle(mutex: &Mutex<Option<Instant>>) -> MutexGuard<'_, Option<Instant>> {
    mutex.lock().unwrap_or_else(|err| err.into_inner())
}

fn take_idle(map: &mut HashMap<PathBuf, WatchHandle>, max_idle: Duration) -> Vec<WatchHandle> {
    let idle_keys: Vec<PathBuf> = map
        .iter()
        .filter(|(_, handle)| {
            if handle.state.subscribers.load(Ordering::Acquire) > 0 {
                return false;
            }
            lock_idle(&handle.state.idle_at).is_some_and(|at| at.elapsed() >= max_idle)
        })
        .map(|(key, _)| key.clone())
        .collect();
    idle_keys
        .into_iter()
        .filter_map(|key| map.remove(&key))
        .collect()
}

#[cfg(test)]
pub(crate) fn release_idle_watches(max_idle: Duration) {
    let retired = {
        let mut hub = lock_hub();
        take_idle(&mut hub, max_idle)
    };
    drop(retired);
}

#[cfg(test)]
mod tests {
    use super::*;
    use notify::event::{CreateKind, ModifyKind, RemoveKind};
    use std::fs;
    use std::thread;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn workspace() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "a3s-workspace-watch-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or(0)
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }

    struct Cleanup(PathBuf);

    impl Drop for Cleanup {
        fn drop(&mut self) {
            release_idle_watches(Duration::ZERO);
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn file_and_directory_events_name_the_listing_to_refresh() {
        let root = workspace();
        let _cleanup = Cleanup(root.clone());
        fs::create_dir(root.join("Notes")).unwrap();
        fs::write(root.join("root.txt"), b"r").unwrap();
        fs::write(root.join("Notes/note.txt"), b"n").unwrap();
        fs::create_dir_all(root.join(".git/objects")).unwrap();

        match refresh_directories(&root, &root.join("root.txt")) {
            Refresh::Directories(dirs) => assert_eq!(dirs, vec![String::new()]),
            other => panic!("root file should refresh the root, got {other:?}"),
        }
        match refresh_directories(&root, &root.join("Notes/note.txt")) {
            Refresh::Directories(dirs) => assert_eq!(dirs, vec!["Notes".to_owned()]),
            other => panic!("nested file should refresh its parent, got {other:?}"),
        }
        match refresh_directories(&root, &root.join("Notes")) {
            Refresh::Directories(dirs) => {
                assert_eq!(dirs, vec![String::new(), "Notes".to_owned()])
            }
            other => panic!("directory should refresh itself and its parent, got {other:?}"),
        }
        assert!(matches!(
            refresh_directories(&root, &root.join(".git/objects")),
            Refresh::Ignore
        ));
        assert!(matches!(
            refresh_directories(&root, Path::new("/outside/file.txt")),
            Refresh::FullScan
        ));
        assert!(matches!(
            refresh_directories(&root, &root),
            Refresh::FullScan
        ));
    }

    #[test]
    fn dependency_and_build_trees_stop_at_the_direct_child() {
        let root = workspace();
        let _cleanup = Cleanup(root.clone());
        fs::create_dir_all(root.join("node_modules/pkg")).unwrap();
        fs::write(root.join("node_modules/pkg/index.js"), b"x").unwrap();
        fs::create_dir_all(root.join("target/debug/deps")).unwrap();
        fs::write(root.join("target/debug/deps/foo"), b"x").unwrap();
        fs::create_dir_all(root.join("dist")).unwrap();
        fs::write(root.join("dist/app.js"), b"x").unwrap();

        assert!(matches!(
            refresh_directories(&root, &root.join("node_modules/pkg/index.js")),
            Refresh::Ignore
        ));
        match refresh_directories(&root, &root.join("node_modules/pkg")) {
            Refresh::Directories(dirs) => {
                assert_eq!(
                    dirs,
                    vec!["node_modules".to_owned(), "node_modules/pkg".to_owned()]
                )
            }
            other => panic!("a direct dependency should refresh its parent, got {other:?}"),
        }
        assert!(matches!(
            refresh_directories(&root, &root.join("target/debug/deps/foo")),
            Refresh::Ignore
        ));
        match refresh_directories(&root, &root.join("target/debug")) {
            Refresh::Directories(dirs) => {
                assert_eq!(dirs, vec!["target".to_owned(), "target/debug".to_owned()])
            }
            other => panic!("a direct build child should refresh its parent, got {other:?}"),
        }
        match refresh_directories(&root, &root.join("dist/app.js")) {
            Refresh::Directories(dirs) => assert_eq!(dirs, vec!["dist".to_owned()]),
            other => panic!("a file beside dist should refresh dist, got {other:?}"),
        }
    }

    #[test]
    fn a_cursor_ahead_of_the_log_asks_for_a_full_scan() {
        let mut inner = Inner::default();
        inner.revision = 2;
        inner.records.push_back(Record {
            revision: 2,
            paths: BTreeSet::from(["Notes".to_owned()]),
            full_scan: false,
        });
        let ahead = snapshot(&inner, 9).unwrap();
        assert_eq!(ahead["fullScan"], json!(true));
        assert_eq!(ahead["revision"], json!(2));
        assert!(snapshot(&inner, 2).is_none());
        let caught_up = snapshot(&inner, 1).unwrap();
        assert_eq!(caught_up["paths"], json!(["Notes"]));
        assert_eq!(caught_up["fullScan"], json!(false));
    }

    #[test]
    fn hidden_subtree_and_access_events_do_not_publish() {
        let root = workspace();
        let _cleanup = Cleanup(root.clone());
        fs::create_dir_all(root.join(".git")).unwrap();
        let mut inner = Inner::default();
        absorb(
            &root,
            Ok(
                Event::new(EventKind::Access(notify::event::AccessKind::Any))
                    .add_path(root.join("visible.txt")),
            ),
            &mut inner,
        );
        absorb(
            &root,
            Ok(Event::new(EventKind::Create(CreateKind::File)).add_path(root.join(".git/config"))),
            &mut inner,
        );
        assert!(inner.pending_started.is_none());
        assert!(!inner.pending_full);
        absorb(
            &root,
            Ok(Event::new(EventKind::Modify(ModifyKind::Any)).add_path(root.join("visible.txt"))),
            &mut inner,
        );
        assert!(inner.pending_started.is_some());
        inner.pending_started = Some(Instant::now() - DEBOUNCE);
        commit_due(&mut inner);
        assert_eq!(inner.revision, 1);
        assert!(inner.records[0].paths.contains(""));
    }

    #[test]
    fn direct_hidden_entries_publish_only_the_parent_even_after_removal() {
        let root = workspace();
        let _cleanup = Cleanup(root.clone());
        for kind in [
            EventKind::Create(CreateKind::File),
            EventKind::Modify(ModifyKind::Any),
            EventKind::Remove(RemoveKind::File),
        ] {
            let mut inner = Inner::default();
            absorb(
                &root,
                Ok(Event::new(kind).add_path(root.join(".source.docx"))),
                &mut inner,
            );
            inner.pending_started = Some(Instant::now() - DEBOUNCE);
            commit_due(&mut inner);
            let frame = snapshot(&inner, 0).unwrap();
            assert_eq!(frame["paths"], json!([""]));
            assert_eq!(frame["fullScan"], false);
            assert!(!frame.to_string().contains(".source.docx"));
        }
    }

    #[test]
    fn native_create_wakes_a_waiting_poll() {
        let root = workspace();
        let _cleanup = Cleanup(root.clone());
        let root_text = root.to_str().unwrap().to_owned();
        let primed = wait_for_changes(&root_text, 0, Duration::from_millis(500)).unwrap();
        let after = primed["revision"].as_u64().unwrap_or(0);
        let waited_root = root_text.clone();
        let handle = thread::spawn(move || {
            wait_for_changes(&waited_root, after, Duration::from_secs(8)).unwrap()
        });
        thread::sleep(Duration::from_millis(150));
        fs::write(root.join("fresh.txt"), b"x").unwrap();
        let body = handle.join().unwrap();
        let paths = body["paths"].as_array().cloned().unwrap_or_default();
        let full_scan = body["fullScan"].as_bool().unwrap_or(false);
        assert!(
            full_scan || paths.iter().any(|path| path.as_str() == Some("")),
            "expected the workspace root to be invalidated, got {body}"
        );
        assert!(body["revision"].as_u64().unwrap_or(0) > after);
    }
}

impl std::fmt::Debug for Refresh {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ignore => write!(formatter, "Ignore"),
            Self::FullScan => write!(formatter, "FullScan"),
            Self::Directories(dirs) => write!(formatter, "Directories({dirs:?})"),
        }
    }
}
