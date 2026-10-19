//! Bounded file and folder search inside one workspace folder, for the composer's `@` picker.
//! Honors `.gitignore` / `.ignore`, skips hidden entries and common build output, never follows
//! symlinks, and caps both the walk and the result list.

use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ignore::{DirEntry, WalkBuilder, WalkState};
use serde_json::{Value, json};

const MAX_DEPTH: usize = 12;
const SCAN_CAP: usize = 300_000;
const SCAN_BUDGET: Duration = Duration::from_millis(1500);
const DEFAULT_LIMIT: usize = 40;
const MAX_LIMIT: usize = 200;
const MAX_QUERY_LEN: usize = 256;

const SKIPPED_DIRS: &[&str] = &[
    "node_modules",
    "target",
    "dist",
    "build",
    "out",
    "coverage",
    "__pycache__",
    "venv",
    "vendor",
];

pub fn search_json(raw_root: &str, query: &str, limit: Option<usize>) -> Result<String, u16> {
    if query.len() > MAX_QUERY_LEN || query.contains('\0') {
        return Err(400);
    }
    let root = crate::local_documents::canonical_workspace(raw_root)?;
    let limit = limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);
    let query = query.trim().to_lowercase();
    let entries = if query.is_empty() {
        top_level(&root, limit)
    } else {
        search(&root, &query, limit)
    };
    Ok(json!({ "entries": entries }).to_string())
}

fn walker(root: &Path, depth: usize) -> WalkBuilder {
    let mut builder = WalkBuilder::new(root);
    builder
        .hidden(true)
        .follow_links(false)
        .require_git(false)
        .git_global(false)
        .parents(false)
        .max_depth(Some(depth))
        .filter_entry(|entry| {
            !(entry.depth() > 0
                && entry.file_type().is_some_and(|kind| kind.is_dir())
                && entry
                    .file_name()
                    .to_str()
                    .is_some_and(|name| SKIPPED_DIRS.contains(&name)))
        });
    builder
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct Hit {
    path: String,
    relative: String,
    is_dir: bool,
}

fn hit(root: &Path, entry: &DirEntry) -> Option<Hit> {
    if entry.depth() == 0 {
        return None;
    }
    let kind = entry.file_type()?;
    if kind.is_symlink() {
        return None;
    }
    let relative = entry
        .path()
        .strip_prefix(root)
        .ok()?
        .to_str()?
        .replace('\\', "/");
    Some(Hit {
        path: entry.path().to_string_lossy().into_owned(),
        relative,
        is_dir: kind.is_dir(),
    })
}

fn top_level(root: &Path, limit: usize) -> Vec<Value> {
    let mut found: Vec<Hit> = walker(root, 1)
        .build()
        .flatten()
        .filter_map(|entry| hit(root, &entry))
        .collect();
    found.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.relative.to_lowercase().cmp(&b.relative.to_lowercase()))
    });
    found.into_iter().take(limit).map(entry_json).collect()
}

/// Min-heap key: the worst kept hit sits on top so it is the one evicted.
type Ranked = Reverse<(i64, Reverse<usize>, Reverse<String>, Hit)>;

fn ranked(score: i64, hit: Hit) -> Ranked {
    Reverse((
        score,
        Reverse(hit.relative.len()),
        Reverse(hit.relative.clone()),
        hit,
    ))
}

fn keep(heap: &mut BinaryHeap<Ranked>, item: Ranked, limit: usize) {
    if heap.len() < limit {
        heap.push(item);
    } else if heap.peek().is_some_and(|worst| item < *worst) {
        heap.pop();
        heap.push(item);
    }
}

/// Flushes one walker thread's best hits into the shared heap when the thread finishes.
struct LocalTop {
    heap: BinaryHeap<Ranked>,
    shared: Arc<Mutex<BinaryHeap<Ranked>>>,
    limit: usize,
}

impl Drop for LocalTop {
    fn drop(&mut self) {
        let Ok(mut shared) = self.shared.lock() else {
            return;
        };
        for item in self.heap.drain() {
            keep(&mut shared, item, self.limit);
        }
    }
}

fn search(root: &Path, query: &str, limit: usize) -> Vec<Value> {
    let shared = Arc::new(Mutex::new(BinaryHeap::with_capacity(limit + 1)));
    let scanned = Arc::new(AtomicUsize::new(0));
    let deadline = Instant::now() + SCAN_BUDGET;
    walker(root, MAX_DEPTH).build_parallel().run(|| {
        let mut local = LocalTop {
            heap: BinaryHeap::with_capacity(limit + 1),
            shared: Arc::clone(&shared),
            limit,
        };
        let scanned = Arc::clone(&scanned);
        Box::new(move |entry| {
            if scanned.fetch_add(1, Ordering::Relaxed) >= SCAN_CAP || Instant::now() >= deadline {
                return WalkState::Quit;
            }
            let Ok(entry) = entry else {
                return WalkState::Continue;
            };
            if let Some(found) = hit(root, &entry)
                && let Some(value) = score(&found.relative, query)
            {
                keep(&mut local.heap, ranked(value, found), local.limit);
            }
            WalkState::Continue
        })
    });
    let heap = Arc::try_unwrap(shared)
        .ok()
        .and_then(|mutex| mutex.into_inner().ok())
        .unwrap_or_default();
    heap.into_sorted_vec()
        .into_iter()
        .map(|Reverse((_, _, _, hit))| entry_json(hit))
        .collect()
}

/// Higher is better: name match beats path match beats an in-order fuzzy match; shallow beats deep.
fn score(relative: &str, query: &str) -> Option<i64> {
    let path = relative.to_lowercase();
    let name = path.rsplit('/').next().unwrap_or(&path);
    let stem = name.split('.').next().unwrap_or(name);
    let base = if name == query || stem == query {
        1000
    } else if name.starts_with(query) {
        800
    } else if name.contains(query) {
        600
    } else if path.contains(query) {
        400
    } else {
        fuzzy(&path, query)?
    };
    Some(base - (path.matches('/').count() as i64) * 8 - (path.len() as i64) / 8)
}

/// Every query character in order, preferring consecutive and then word-start positions;
/// consecutive and word-start characters score higher.
fn fuzzy(path: &str, query: &str) -> Option<i64> {
    let chars: Vec<char> = path.chars().collect();
    let word_start =
        |index: usize| index == 0 || matches!(chars[index - 1], '/' | '-' | '_' | '.' | ' ');
    let mut position = 0;
    let mut previous: Option<usize> = None;
    let mut total = 100;
    for wanted in query.chars() {
        let next = previous.map_or(0, |index| index + 1);
        let found = if chars.get(next) == Some(&wanted) && previous.is_some() {
            next
        } else {
            (position..chars.len())
                .find(|&index| chars[index] == wanted && word_start(index))
                .or_else(|| (position..chars.len()).find(|&index| chars[index] == wanted))?
        };
        if previous == Some(found.wrapping_sub(1)) {
            total += 15;
        }
        if word_start(found) {
            total += 10;
        }
        previous = Some(found);
        position = found + 1;
    }
    Some(total)
}

fn entry_json(hit: Hit) -> Value {
    json!({ "path": hit.path, "relativePath": hit.relative, "isDirectory": hit.is_dir })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn workspace(tag: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "a3s-workspace-search-{tag}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or(0)
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn relatives(json: &str) -> Vec<String> {
        let value: Value = serde_json::from_str(json).unwrap();
        value["entries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| entry["relativePath"].as_str().unwrap().to_owned())
            .collect()
    }

    #[test]
    fn lists_folders_first_and_hides_ignored_hidden_and_build_entries() {
        let root = workspace("list");
        fs::create_dir_all(root.join("src")).unwrap();
        fs::create_dir_all(root.join("node_modules/pkg")).unwrap();
        fs::create_dir_all(root.join(".git")).unwrap();
        fs::write(root.join("README.md"), b"r").unwrap();
        fs::write(root.join(".env"), b"secret").unwrap();
        fs::write(root.join("debug.log"), b"l").unwrap();
        fs::write(root.join(".gitignore"), b"*.log\n").unwrap();

        let listing = search_json(root.to_str().unwrap(), "", None).unwrap();
        assert_eq!(relatives(&listing), vec!["src", "README.md"]);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn ranks_name_matches_above_path_and_fuzzy_matches() {
        let root = workspace("rank");
        fs::create_dir_all(root.join("src/composer")).unwrap();
        fs::create_dir_all(root.join("docs")).unwrap();
        fs::write(root.join("src/composer/index.ts"), b"i").unwrap();
        fs::write(root.join("src/composer.ts"), b"c").unwrap();
        fs::write(root.join("docs/compose-guide.md"), b"g").unwrap();
        fs::write(root.join("src/main.ts"), b"m").unwrap();

        let found = relatives(&search_json(root.to_str().unwrap(), "composer", None).unwrap());
        assert_eq!(found[0], "src/composer");
        assert_eq!(found[1], "src/composer.ts");
        assert!(found.contains(&"src/composer/index.ts".to_owned()));
        assert!(!found.contains(&"src/main.ts".to_owned()));

        let fuzzy = relatives(&search_json(root.to_str().unwrap(), "scoi", None).unwrap());
        assert_eq!(fuzzy[0], "src/composer/index.ts");
        assert!(relatives(&search_json(root.to_str().unwrap(), "zzz", None).unwrap()).is_empty());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_roots_outside_a_real_folder() {
        assert_eq!(search_json("/", "", None), Err(400));
        assert_eq!(search_json("relative/path", "", None), Err(400));
        assert_eq!(search_json("/tmp/../etc", "", None), Err(400));
    }
}
