//! Installed plugin OKF pages and the local a3s-vec index of those pages.
//!
//! The unpacked package stays unchanged. Edits land under
//! `<tenant>/wiki/plugins/<slug>/`, which a handbook projection does not
//! delete, and the same markdown is copied into the workspace wiki when the
//! caller names one. The index is a durable a3s-vec collection beside that
//! wiki, not inside it.

use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};

use a3s_vec::{
    Collection, CollectionOptions, CollectionSchema, DataType, Doc, FieldSchema, Fts, IndexParams,
    SearchQuery,
};
use serde::Serialize;

const BODY_LIMIT: usize = 1024 * 1024;
const MAX_PAGES: usize = 256;
const TITLE_CHARS: usize = 80;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ListedPage {
    pub path: String,
    pub title: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ReadPage {
    pub path: String,
    pub title: String,
    pub markdown: String,
}

struct PageBytes {
    path: String,
    markdown: String,
}

pub fn list_pages(tenant: &Path, slug: &str) -> io::Result<Vec<ListedPage>> {
    let pages = effective_pages(tenant, slug)?;
    Ok(pages
        .into_iter()
        .map(|page| ListedPage {
            title: page_title(&page.markdown, &page.path),
            path: page.path,
        })
        .collect())
}

pub fn read_page(tenant: &Path, slug: &str, relative: &str) -> io::Result<ReadPage> {
    let pages = effective_pages(tenant, slug)?;
    if !page_path_ok(relative) {
        return Err(invalid("invalid"));
    }
    let page = pages
        .into_iter()
        .find(|page| page.path == relative)
        .ok_or_else(|| invalid("invalid"))?;
    Ok(ReadPage {
        title: page_title(&page.markdown, &page.path),
        path: page.path,
        markdown: page.markdown,
    })
}

/// Remove the member overlay and the local a3s-vec index for one plugin.
pub fn remove_local_state(tenant: &Path, slug: &str) -> io::Result<()> {
    if !crate::org_login::capability_slug_ok(slug) {
        return Err(invalid("invalid"));
    }
    remove_child(&tenant.join("wiki").join("plugins"), slug)?;
    remove_child(&tenant.join("vec").join("plugin-okf"), slug)
}

/// Copy packaged OKF pages into an agent wiki when that page is absent.
///
/// An existing local file stays as the user's copy. The package file is only read.
pub fn seed_missing(tenant: &Path, slug: &str, agent_wiki: &Path) -> io::Result<usize> {
    if !crate::org_login::capability_slug_ok(slug) {
        return Err(invalid("invalid"));
    }
    let plugin = tenant.join("capabilities").join("plugins").join(slug);
    if !plugin.is_dir() {
        return Err(missing_plugin());
    }
    let agent_wiki = agent_wiki_dir(agent_wiki, slug)?;
    let mut bundled = BTreeMap::new();
    collect_markdown(&plugin, &plugin, &mut bundled)?;
    let mut seeded = 0usize;
    for (relative, markdown) in bundled {
        let dest = rooted(&agent_wiki, &relative)?;
        if fs::symlink_metadata(&dest).is_ok() {
            continue;
        }
        write_inside(&agent_wiki, &relative, markdown.as_bytes())?;
        seeded += 1;
    }
    Ok(seeded)
}

/// Write one existing OKF page and rebuild the plugin's a3s-vec collection.
pub fn save_page(
    tenant: &Path,
    slug: &str,
    relative: &str,
    markdown: &str,
    workspace_wiki: Option<&Path>,
) -> io::Result<usize> {
    if !page_path_ok(relative) || markdown.contains('\0') || markdown.len() > BODY_LIMIT {
        return Err(invalid("invalid"));
    }
    let known = effective_pages(tenant, slug)?;
    if !known.iter().any(|page| page.path == relative) {
        return Err(invalid("invalid"));
    }
    let overlay_root = wiki_plugin_root(tenant, slug);
    write_inside(&overlay_root, relative, markdown.as_bytes())?;
    if let Some(wiki) = workspace_wiki {
        let wiki = workspace_wiki_dir(wiki)?;
        let mirror_root = wiki.join("plugins").join(slug);
        let mirror = rooted(&mirror_root, relative)?;
        let overlay = rooted(&overlay_root, relative)?;
        if mirror != overlay {
            write_inside(&mirror_root, relative, markdown.as_bytes())?;
        }
    }
    let pages = effective_pages(tenant, slug)?;
    rebuild_index(&index_dir(tenant, slug), &pages)
}

fn effective_pages(tenant: &Path, slug: &str) -> io::Result<Vec<PageBytes>> {
    if !crate::org_login::capability_slug_ok(slug) {
        return Err(invalid("invalid"));
    }
    let plugin = tenant.join("capabilities").join("plugins").join(slug);
    if !plugin.is_dir() {
        return Err(missing_plugin());
    }
    let mut bundled = BTreeMap::new();
    collect_markdown(&plugin, &plugin, &mut bundled)?;
    let overlay_root = wiki_plugin_root(tenant, slug);
    let mut overlay = BTreeMap::new();
    if overlay_root.is_dir() {
        collect_markdown(&overlay_root, &overlay_root, &mut overlay)?;
    }
    let mut paths: Vec<String> = bundled.keys().cloned().collect();
    for path in overlay.keys() {
        if !paths.iter().any(|existing| existing == path) {
            paths.push(path.clone());
        }
    }
    paths.sort();
    paths.truncate(MAX_PAGES);
    let mut pages = Vec::with_capacity(paths.len());
    for path in paths {
        let Some(markdown) = overlay
            .get(&path)
            .cloned()
            .or_else(|| bundled.get(&path).cloned())
        else {
            continue;
        };
        pages.push(PageBytes { path, markdown });
    }
    Ok(pages)
}

fn collect_markdown(root: &Path, dir: &Path, out: &mut BTreeMap<String, String>) -> io::Result<()> {
    if out.len() >= MAX_PAGES {
        return Ok(());
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return Ok(());
    };
    let mut children = Vec::new();
    for entry in entries.filter_map(Result::ok) {
        children.push(entry.path());
    }
    children.sort();
    for path in children {
        if out.len() >= MAX_PAGES {
            break;
        }
        let Ok(metadata) = fs::symlink_metadata(&path) else {
            continue;
        };
        if metadata.file_type().is_symlink() {
            continue;
        }
        if metadata.is_dir() {
            collect_markdown(root, &path, out)?;
            continue;
        }
        if !metadata.is_file() || metadata.len() > BODY_LIMIT as u64 {
            continue;
        }
        let Some(relative) = path.strip_prefix(root).ok().and_then(|path| path.to_str()) else {
            continue;
        };
        let relative = relative.replace('\\', "/");
        if !page_path_ok(&relative) {
            continue;
        }
        let Ok(markdown) = fs::read_to_string(&path) else {
            continue;
        };
        if markdown.contains('\0') || markdown.len() > BODY_LIMIT {
            continue;
        }
        out.insert(relative, markdown);
    }
    Ok(())
}

fn write_inside(root: &Path, relative: &str, bytes: &[u8]) -> io::Result<()> {
    let path = rooted(root, relative)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = fs::File::create(&path)?;
    file.write_all(bytes)?;
    Ok(())
}

fn rooted(root: &Path, relative: &str) -> io::Result<PathBuf> {
    if !page_path_ok(relative) {
        return Err(invalid("invalid"));
    }
    let mut current = root.to_path_buf();
    for part in relative.split('/') {
        current = current.join(part);
        if fs::symlink_metadata(&current).is_ok_and(|meta| meta.file_type().is_symlink()) {
            return Err(invalid("invalid"));
        }
    }
    Ok(current)
}

fn workspace_wiki_dir(path: &Path) -> io::Result<PathBuf> {
    if !path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
        || path.file_name().and_then(|name| name.to_str()) != Some("wiki")
    {
        return Err(invalid("invalid"));
    }
    let text = path.to_str().ok_or_else(|| invalid("invalid"))?;
    if text.is_empty() || text.contains('\0') {
        return Err(invalid("invalid"));
    }
    if fs::symlink_metadata(path).is_ok_and(|meta| meta.file_type().is_symlink()) {
        return Err(invalid("invalid"));
    }
    Ok(path.to_path_buf())
}

fn remove_child(parent: &Path, slug: &str) -> io::Result<()> {
    if fs::symlink_metadata(parent).is_ok_and(|meta| meta.file_type().is_symlink()) {
        return Err(invalid("invalid"));
    }
    let target = parent.join(slug);
    match fs::symlink_metadata(&target) {
        Ok(meta) if meta.is_dir() => fs::remove_dir_all(&target),
        Ok(_) => fs::remove_file(&target),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn agent_wiki_dir(path: &Path, slug: &str) -> io::Result<PathBuf> {
    if !path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
        || path.file_name().and_then(|name| name.to_str()) != Some(slug)
    {
        return Err(invalid("invalid"));
    }
    let agents = path.parent().ok_or_else(|| invalid("invalid"))?;
    let wiki = agents.parent().ok_or_else(|| invalid("invalid"))?;
    if agents.file_name().and_then(|name| name.to_str()) != Some("agents")
        || wiki.file_name().and_then(|name| name.to_str()) != Some("wiki")
    {
        return Err(invalid("invalid"));
    }
    let text = path.to_str().ok_or_else(|| invalid("invalid"))?;
    if text.is_empty() || text.contains('\0') {
        return Err(invalid("invalid"));
    }
    if fs::symlink_metadata(path).is_ok_and(|meta| meta.file_type().is_symlink()) {
        return Err(invalid("invalid"));
    }
    Ok(path.to_path_buf())
}

fn wiki_plugin_root(tenant: &Path, slug: &str) -> PathBuf {
    tenant.join("wiki").join("plugins").join(slug)
}

fn index_dir(tenant: &Path, slug: &str) -> PathBuf {
    tenant.join("vec").join("plugin-okf").join(slug)
}

fn page_path_ok(path: &str) -> bool {
    if path.len() > 240
        || !path.starts_with("okf/")
        || !path.ends_with(".md")
        || path.contains('\0')
        || path.contains('\\')
        || path.contains("//")
    {
        return false;
    }
    path.split('/').all(|part| {
        !part.is_empty()
            && part != "."
            && part != ".."
            && !part.starts_with('.')
            && part.chars().all(|ch| !ch.is_control())
    })
}

fn page_title(markdown: &str, relative: &str) -> String {
    if let Some(title) = frontmatter_title(markdown) {
        return title;
    }
    Path::new(relative)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .unwrap_or(relative)
        .chars()
        .take(TITLE_CHARS)
        .collect()
}

fn frontmatter_title(markdown: &str) -> Option<String> {
    let mut lines = markdown.lines();
    if lines.next().map(str::trim) != Some("---") {
        return None;
    }
    for line in lines {
        if line.trim() == "---" {
            break;
        }
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        if key.trim() != "title" {
            continue;
        }
        let value = value.trim().trim_matches(['"', '\'']).trim();
        if value.is_empty() || value.chars().count() > TITLE_CHARS {
            return None;
        }
        return Some(value.to_owned());
    }
    None
}

fn rebuild_index(dir: &Path, pages: &[PageBytes]) -> io::Result<usize> {
    let parent = dir
        .parent()
        .ok_or_else(|| invalid("invalid"))?
        .to_path_buf();
    let name = dir
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| invalid("invalid"))?;
    let staging = parent.join(format!(".{name}.staging"));
    let _ = fs::remove_dir_all(&staging);
    fs::create_dir_all(&parent)?;
    let count = match write_collection(&staging.join("collection"), pages) {
        Ok(count) => count,
        Err(error) => {
            let _ = fs::remove_dir_all(&staging);
            return Err(error);
        }
    };
    let _ = fs::remove_dir_all(dir);
    fs::rename(&staging, dir).map_err(|error| {
        let _ = fs::remove_dir_all(&staging);
        error
    })?;
    Ok(count)
}

fn write_collection(path: &Path, pages: &[PageBytes]) -> io::Result<usize> {
    ensure_engine()?;
    let mut body = FieldSchema::new("body", DataType::String, false, 0).map_err(engine_error)?;
    let fts = IndexParams::fts(Some("whitespace"), None, None).map_err(engine_error)?;
    body.set_index_params(&fts).map_err(engine_error)?;
    let path_field = FieldSchema::new("path", DataType::String, false, 0).map_err(engine_error)?;
    let schema = CollectionSchema::builder("plugin_okf")
        .add_field(body)
        .add_field(path_field)
        .build()
        .map_err(engine_error)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let path_text = path.to_str().ok_or_else(|| invalid("invalid"))?;
    let collection = Collection::create_and_open(path_text, &schema, None).map_err(engine_error)?;
    let mut docs = Vec::new();
    let mut ordinal = 0usize;
    for page in pages {
        let tokens = tokenize(&page.markdown);
        if tokens.is_empty() {
            continue;
        }
        let mut doc = Doc::new().map_err(engine_error)?;
        doc.set_pk(&format!("p{ordinal}"));
        ordinal += 1;
        doc.add_string("body", &tokens.join(" "))
            .map_err(engine_error)?;
        doc.add_string("path", &page.path).map_err(engine_error)?;
        docs.push(doc);
    }
    if !docs.is_empty() {
        let references = docs.iter().collect::<Vec<_>>();
        let result = collection.insert(&references).map_err(engine_error)?;
        if result.error_count != 0 {
            let _ = collection.close();
            return Err(io::Error::other("index_failed"));
        }
    }
    collection.flush().map_err(engine_error)?;
    collection.close().map_err(engine_error)?;
    Ok(ordinal)
}

fn search_paths(dir: &Path, query: &str) -> io::Result<Vec<String>> {
    let tokens = tokenize(query);
    if tokens.is_empty() {
        return Ok(Vec::new());
    }
    ensure_engine()?;
    let mut fts = Fts::new().map_err(engine_error)?;
    fts.set_match_string(&tokens.join(" "))
        .map_err(engine_error)?;
    let mut search = SearchQuery::fts("body", &fts, 8).map_err(engine_error)?;
    search.set_output_fields(&["path"]).map_err(engine_error)?;
    let mut options = CollectionOptions::new().map_err(engine_error)?;
    options.set_read_only(true).map_err(engine_error)?;
    let path = dir.join("collection");
    let path_text = path.to_str().ok_or_else(|| invalid("invalid"))?;
    let collection = Collection::open(path_text, Some(&options)).map_err(engine_error)?;
    let documents = collection.query(&search).map_err(engine_error)?;
    let mut paths = Vec::new();
    for document in &documents {
        if let Ok(Some(path)) = document.get_string("path") {
            paths.push(path);
        }
    }
    let _ = collection.close();
    Ok(paths)
}

fn ensure_engine() -> io::Result<()> {
    a3s_vec::initialize(None).map_err(engine_error)
}

fn engine_error(error: impl std::fmt::Display) -> io::Error {
    io::Error::other(error.to_string())
}

fn tokenize(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut word = String::new();
    let mut previous_cjk = None;
    for ch in text.chars() {
        if is_cjk(ch) {
            flush_word(&mut word, &mut tokens);
            tokens.push(ch.to_string());
            if let Some(previous) = previous_cjk {
                tokens.push(format!("{previous}{ch}"));
            }
            previous_cjk = Some(ch);
        } else {
            previous_cjk = None;
            if ch.is_alphanumeric() || ch == '_' {
                word.push(ch.to_ascii_lowercase());
            } else {
                flush_word(&mut word, &mut tokens);
            }
        }
    }
    flush_word(&mut word, &mut tokens);
    tokens
}

fn flush_word(word: &mut String, tokens: &mut Vec<String>) {
    if word.is_empty() {
        return;
    }
    tokens.push(std::mem::take(word));
}

fn is_cjk(ch: char) -> bool {
    matches!(
        ch as u32,
        0x3400..=0x4dbf
            | 0x4e00..=0x9fff
            | 0xf900..=0xfaff
            | 0x20000..=0x2fa1f
            | 0x3040..=0x30ff
            | 0xac00..=0xd7af
    )
}

fn missing_plugin() -> io::Error {
    io::Error::new(io::ErrorKind::NotFound, "missing_plugin")
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tenant(label: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("a3s-plugin-okf-{label}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("capabilities/plugins/knowledge/okf/domain")).unwrap();
        fs::create_dir_all(root.join("capabilities/plugins/knowledge/skills/research")).unwrap();
        fs::write(
            root.join("capabilities/plugins/knowledge/okf/domain/index.md"),
            "---\ntitle: 领域\n---\n甲方付款条款\n",
        )
        .unwrap();
        fs::write(
            root.join("capabilities/plugins/knowledge/okf/domain/note.py"),
            "print(1)\n",
        )
        .unwrap();
        fs::write(
            root.join("capabilities/plugins/knowledge/skills/research/SKILL.md"),
            "# skill\n",
        )
        .unwrap();
        root
    }

    #[test]
    fn list_reads_okf_markdown_and_skips_other_files() {
        let root = tenant("list");
        let pages = list_pages(&root, "knowledge").unwrap();
        assert_eq!(
            pages,
            vec![ListedPage {
                path: "okf/domain/index.md".to_owned(),
                title: "领域".to_owned(),
            }]
        );
        let page = read_page(&root, "knowledge", "okf/domain/index.md").unwrap();
        assert!(page.markdown.contains("甲方付款条款"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn save_keeps_the_package_and_builds_a_searchable_index() {
        let root = tenant("save");
        let package = root.join("capabilities/plugins/knowledge/okf/domain/index.md");
        let before = fs::read(&package).unwrap();
        let workspace = root.join("workspace/wiki");
        fs::create_dir_all(&workspace).unwrap();
        let indexed = save_page(
            &root,
            "knowledge",
            "okf/domain/index.md",
            "---\ntitle: 领域\n---\n甲方付款已修订\n",
            Some(workspace.as_path()),
        )
        .unwrap();
        assert_eq!(indexed, 1);
        assert_eq!(fs::read(&package).unwrap(), before);
        let overlay = root.join("wiki/plugins/knowledge/okf/domain/index.md");
        assert!(fs::read_to_string(&overlay).unwrap().contains("已修订"));
        let mirrored = workspace.join("plugins/knowledge/okf/domain/index.md");
        assert!(fs::read_to_string(&mirrored).unwrap().contains("已修订"));
        crate::local_assets::project_wiki(
            &root.join("wiki"),
            r#"{"pages":[{"id":"handbook","title":"手册","body":"个人百科"}]}"#.as_bytes(),
        )
        .unwrap();
        assert!(overlay.is_file());
        assert!(root.join("wiki/handbook.md").is_file());
        let hits = search_paths(&index_dir(&root, "knowledge"), "付款").unwrap();
        assert_eq!(hits, vec!["okf/domain/index.md".to_owned()]);
        let saved = read_page(&root, "knowledge", "okf/domain/index.md").unwrap();
        assert!(saved.markdown.contains("已修订"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn seed_copies_a_missing_page_and_keeps_a_local_edit() {
        let root = tenant("seed");
        let package = root.join("capabilities/plugins/knowledge/okf/domain/index.md");
        let before = fs::read(&package).unwrap();
        let wiki = root.join("workspace/wiki/agents/knowledge");
        fs::create_dir_all(wiki.join("okf/domain")).unwrap();
        fs::write(wiki.join("okf/domain/index.md"), "我改过的条款\n").unwrap();
        fs::write(
            root.join("capabilities/plugins/knowledge/okf/domain/extra.md"),
            "---\ntitle: 补充\n---\n默认补充\n",
        )
        .unwrap();
        let seeded = seed_missing(&root, "knowledge", &wiki).unwrap();
        assert_eq!(seeded, 1);
        assert_eq!(fs::read(&package).unwrap(), before);
        assert_eq!(
            fs::read_to_string(wiki.join("okf/domain/index.md")).unwrap(),
            "我改过的条款\n"
        );
        assert!(
            fs::read_to_string(wiki.join("okf/domain/extra.md"))
                .unwrap()
                .contains("默认补充")
        );
        let again = seed_missing(&root, "knowledge", &wiki).unwrap();
        assert_eq!(again, 0);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn save_rejects_a_path_outside_the_bundled_wiki() {
        let root = tenant("reject");
        let error = save_page(&root, "knowledge", "okf/../index.md", "x", None).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        let error =
            save_page(&root, "knowledge", "skills/research/SKILL.md", "x", None).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        let error = save_page(&root, "knowledge", "okf/domain/new.md", "新页", None).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(
            !root
                .join("wiki/plugins/knowledge/okf/domain/new.md")
                .exists()
        );
        let _ = fs::remove_dir_all(&root);
    }
}
