//! Tenant-local connectors, skills, and wiki pages the agent process can see.
//!
//! The catalog carries names and short descriptions. Bodies stay in the files.
//! Saved wiki pages are projected to `<tenant>/wiki/<id>.md` because the agent
//! cannot read renderer memory. The same file shape is what a3s-code rescans.

use std::collections::BTreeSet;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::{Value, json};

const DESCRIPTION_LIMIT: usize = 160;
const BODY_LIMIT: usize = 32 * 1024;
const FRONTMATTER_LIMIT: usize = 4 * 1024;
const MAX_PER_KIND: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogEntry {
    pub kind: &'static str,
    pub name: String,
    pub description: String,
}

#[derive(Debug, Deserialize)]
struct WikiPut {
    pages: Vec<WikiPutPage>,
}

#[derive(Debug, Deserialize)]
struct WikiPutPage {
    id: String,
    title: String,
    #[serde(default)]
    body: String,
}

/// `{workspace}/wiki` for the personal bot.
///
/// An absolute, non-root stored workspace wins. A relative path, a filesystem
/// root, or an empty preference uses `~/.a3s/wiki`. An empty home yields nothing.
pub fn app_wiki_directory(stored: &str, home: &Path) -> Option<PathBuf> {
    let home = normalize_workspace(home);
    if home.as_os_str().is_empty() || is_filesystem_root(&home) {
        return None;
    }
    let custom = normalize_workspace_text(stored);
    let workspace = if !custom.is_empty()
        && is_absolute_workspace(&custom)
        && !is_filesystem_root(Path::new(&custom))
    {
        PathBuf::from(custom)
    } else {
        home.join(".a3s")
    };
    let wiki = workspace.join("wiki");
    (wiki.is_absolute() && wiki.file_name().and_then(|name| name.to_str()) == Some("wiki"))
        .then_some(wiki)
}

/// Saved `app.workspace.path` for this tenant. Missing sign-in or file yields empty.
pub fn stored_workspace_preference(slug: &str) -> String {
    let Some(root) = crate::providers::tenant_data_dir(slug) else {
        return String::new();
    };
    let Some(member) = crate::member_session::MemberSessionStore::open(slug).member_id() else {
        return String::new();
    };
    let store = crate::preferences::PreferenceStore::new(root.join("preferences"));
    let Ok(snapshot) = store.read(&member) else {
        return String::new();
    };
    snapshot
        .values
        .get("app.workspace.path")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned()
}

fn normalize_workspace(path: &Path) -> PathBuf {
    PathBuf::from(normalize_workspace_text(&path.to_string_lossy()))
}

fn normalize_workspace_text(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if is_filesystem_root(Path::new(trimmed)) {
        return trimmed.trim_end_matches(['/', '\\']).to_owned();
    }
    trimmed.trim_end_matches(['/', '\\']).to_owned()
}

fn is_absolute_workspace(value: &str) -> bool {
    value.starts_with('/') || drive_prefix(value)
}

fn drive_prefix(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_ascii_alphabetic() {
        return false;
    }
    matches!(chars.next(), Some(':'))
}

fn is_filesystem_root(path: &Path) -> bool {
    let text = path.to_string_lossy();
    let trimmed = text.trim_end_matches(['/', '\\']);
    trimmed.is_empty()
        || trimmed == "/"
        || trimmed == "\\"
        || (drive_prefix(trimmed) && trimmed.len() == 2)
}

/// Absolute `…/wiki` directory from a turn's host context.
pub fn studio_wiki_root(body: &Value) -> Option<PathBuf> {
    let text = body.get("hostContext")?.get("wiki")?.as_str()?;
    if text.is_empty() || text.contains('\0') {
        return None;
    }
    let path = PathBuf::from(text);
    if !path.is_absolute() {
        return None;
    }
    (path.file_name().and_then(|name| name.to_str()) == Some("wiki")).then_some(path)
}

/// Point the handoff at the app-workspace wiki and rescan that directory.
pub fn with_studio_wiki(assets: Option<Value>, wiki: &Path) -> Value {
    let mut value = assets.unwrap_or_else(|| json!({}));
    let connectors = value
        .get("connectors")
        .and_then(Value::as_str)
        .map(PathBuf::from);
    let skills = value
        .get("skills")
        .and_then(Value::as_str)
        .map(PathBuf::from);
    let catalog = scan(
        connectors.as_deref().unwrap_or(Path::new("")),
        skills.as_deref().unwrap_or(Path::new("")),
        wiki,
    );
    let Some(obj) = value.as_object_mut() else {
        return json!({
            "wiki": wiki.display().to_string(),
            "catalog": catalog_entries(&catalog),
        });
    };
    obj.insert("wiki".into(), json!(wiki.display().to_string()));
    obj.insert("catalog".into(), json!(catalog_entries(&catalog)));
    value
}

fn catalog_entries(catalog: &[CatalogEntry]) -> Vec<Value> {
    catalog
        .iter()
        .map(|entry| {
            json!({
                "kind": entry.kind,
                "name": entry.name,
                "description": entry.description,
            })
        })
        .collect()
}

/// `_meta.a3sHost.assets` for `session/new`. `None` when nothing is installed.
pub fn handoff_value(tenant: &Path) -> Option<Value> {
    let connectors = tenant.join("capabilities");
    let skills = connectors.join("skills");
    let wiki = tenant.join("wiki");
    let catalog = scan(&connectors, &skills, &wiki);
    if catalog.is_empty() {
        return None;
    }
    let entries = catalog_entries(&catalog);
    Some(json!({
        "connectors": connectors.display().to_string(),
        "skills": skills.display().to_string(),
        "wiki": wiki.display().to_string(),
        "catalog": entries,
    }))
}

pub fn scan(connectors: &Path, skills: &Path, wiki: &Path) -> Vec<CatalogEntry> {
    let mut items = Vec::new();
    items.extend(connector_entries(connectors));
    items.extend(skill_entries(skills));
    items.extend(wiki_entries(wiki));
    items.sort_by(|left, right| {
        left.kind
            .cmp(right.kind)
            .then_with(|| left.name.cmp(&right.name))
    });
    items
}

/// Replace `<tenant>/wiki` with the member's saved pages.
pub fn project_wiki(dir: &Path, body: &[u8]) -> io::Result<()> {
    let put: WikiPut = serde_json::from_slice(body)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid"))?;
    fs::create_dir_all(dir)?;
    let mut keep = BTreeSet::new();
    for page in put.pages.into_iter().take(MAX_PER_KIND) {
        if !page_id_ok(&page.id) || !title_ok(&page.title) {
            continue;
        }
        let file_name = format!("{}.md", page.id);
        let path = dir.join(&file_name);
        if path.parent() != Some(dir) {
            continue;
        }
        fs::write(&path, render_page(&page))?;
        keep.insert(file_name);
    }
    let entries = fs::read_dir(dir)?;
    for entry in entries.filter_map(Result::ok) {
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if !name.ends_with(".md") || keep.contains(name) {
            continue;
        }
        let path = entry.path();
        if fs::symlink_metadata(&path)
            .is_ok_and(|meta| meta.file_type().is_file() || meta.file_type().is_symlink())
        {
            let _ = fs::remove_file(path);
        }
    }
    Ok(())
}

fn render_page(page: &WikiPutPage) -> String {
    let description = page
        .body
        .lines()
        .find(|line| !line.trim().is_empty())
        .map(one_line)
        .unwrap_or_default();
    let mut body = String::new();
    for ch in page.body.chars().take(BODY_LIMIT) {
        body.push(ch);
    }
    format!(
        "---\ntitle: {}\ndescription: {}\n---\n{}",
        yaml_quote(page.title.trim()),
        yaml_quote(&description),
        body
    )
}

fn connector_entries(root: &Path) -> Vec<CatalogEntry> {
    let Ok(bytes) = fs::read(root.join("mcp.json")) else {
        return Vec::new();
    };
    let Ok(doc) = serde_json::from_slice::<Value>(&bytes) else {
        return Vec::new();
    };
    let Some(servers) = doc.get("mcpServers").and_then(Value::as_object) else {
        return Vec::new();
    };
    let mut names: Vec<&str> = servers
        .keys()
        .map(String::as_str)
        .filter(|name| crate::org_login::capability_slug_ok(name))
        .collect();
    names.sort_unstable();
    names.truncate(MAX_PER_KIND);
    names
        .into_iter()
        .map(|name| {
            let kind = servers
                .get(name)
                .and_then(Value::as_object)
                .and_then(|server| server.get("type"))
                .and_then(Value::as_str);
            let path = root.join(format!("{name}.md"));
            let description = read_prefix(&path, FRONTMATTER_LIMIT)
                .and_then(|text| crate::connector_notes::note_summary(&text))
                .unwrap_or_else(|| crate::connector_notes::transport_label(kind));
            CatalogEntry {
                kind: "connector",
                name: name.to_string(),
                description,
            }
        })
        .collect()
}

fn skill_entries(root: &Path) -> Vec<CatalogEntry> {
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };
    let mut dirs: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .filter(|entry| {
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                return false;
            };
            if name.starts_with('.') || name.contains('/') || name.contains('\\') {
                return false;
            }
            fs::metadata(entry.path()).is_ok_and(|meta| meta.is_dir())
        })
        .map(|entry| entry.path())
        .collect();
    dirs.sort();
    let mut items = Vec::new();
    for dir in dirs {
        if items.len() >= MAX_PER_KIND {
            break;
        }
        let path = dir.join("SKILL.md");
        if !is_real_file(&path) {
            continue;
        }
        let Some(text) = read_prefix(&path, FRONTMATTER_LIMIT) else {
            continue;
        };
        let (name, description) = skill_frontmatter(&text);
        let Some(name) = name.filter(|name| crate::org_login::capability_slug_ok(name)) else {
            continue;
        };
        items.push(CatalogEntry {
            kind: "skill",
            name,
            description: description.map(|text| one_line(&text)).unwrap_or_default(),
        });
    }
    items
}

fn wiki_entries(root: &Path) -> Vec<CatalogEntry> {
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension().and_then(|ext| ext.to_str()) == Some("md") && is_real_file(path)
        })
        .collect();
    files.sort();
    let mut items = Vec::new();
    for path in files {
        if items.len() >= MAX_PER_KIND {
            break;
        }
        let Some(text) = read_prefix(&path, FRONTMATTER_LIMIT) else {
            continue;
        };
        let (title, description) = wiki_frontmatter(&text);
        let Some(title) = title.filter(|title| title_ok(title)) else {
            continue;
        };
        if items.iter().any(|item: &CatalogEntry| item.name == title) {
            continue;
        }
        items.push(CatalogEntry {
            kind: "wiki",
            name: title,
            description: description.map(|text| one_line(&text)).unwrap_or_default(),
        });
    }
    items
}

fn is_real_file(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|meta| meta.file_type().is_file())
}

fn read_prefix(path: &Path, limit: usize) -> Option<String> {
    let mut file = fs::File::open(path).ok()?;
    let mut bytes = vec![0_u8; limit];
    let read = file.read(&mut bytes).ok()?;
    bytes.truncate(read);
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

fn skill_frontmatter(text: &str) -> (Option<String>, Option<String>) {
    scalar_pair(text, "name", "description")
}

fn wiki_frontmatter(text: &str) -> (Option<String>, Option<String>) {
    scalar_pair(text, "title", "description")
}

fn scalar_pair(text: &str, primary: &str, secondary: &str) -> (Option<String>, Option<String>) {
    let mut lines = text.lines();
    if lines.next().map(str::trim) != Some("---") {
        return (None, None);
    }
    let mut first = None;
    let mut second = None;
    for line in lines {
        if line.trim() == "---" {
            break;
        }
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let value = unquote(value.trim());
        if value.is_empty() {
            continue;
        }
        if key.trim() == primary {
            first = Some(value);
        } else if key.trim() == secondary {
            second = Some(value);
        }
    }
    (first, second)
}

fn unquote(value: &str) -> String {
    let quoted = value.len() >= 2
        && ((value.starts_with('"') && value.ends_with('"'))
            || (value.starts_with('\'') && value.ends_with('\'')));
    if quoted {
        value[1..value.len() - 1].to_owned()
    } else {
        value.to_owned()
    }
}

fn one_line(raw: &str) -> String {
    let collapsed: String = raw
        .chars()
        .map(|ch| if ch.is_whitespace() { ' ' } else { ch })
        .filter(|ch| *ch != '\0')
        .collect();
    collapsed.trim().chars().take(DESCRIPTION_LIMIT).collect()
}

fn yaml_quote(value: &str) -> String {
    let mut out = String::from("\"");
    for ch in value
        .chars()
        .filter(|ch| *ch != '\n' && *ch != '\r' && *ch != '"' && *ch != '\\')
    {
        out.push(ch);
    }
    out.push('"');
    out
}

fn page_id_ok(id: &str) -> bool {
    let bytes = id.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= 64
        && bytes[0].is_ascii_alphanumeric()
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || *byte == b'_' || *byte == b'-')
}

fn title_ok(title: &str) -> bool {
    let trimmed = title.trim();
    !trimmed.is_empty()
        && trimmed != "."
        && trimmed != ".."
        && !trimmed
            .chars()
            .any(|ch| ch == '/' || ch == '\\' || ch == '\0')
        && trimmed.chars().count() <= 120
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projects_wiki_pages_onto_the_tenant_directory() {
        let dir = std::env::temp_dir().join(format!(
            "a3s-wiki-project-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let _ = fs::remove_dir_all(&dir);
        let body = serde_json::json!({
            "pages": [{
                "id": "hub_manual1",
                "title": "产品手册甲",
                "body": "Saved handbook page\nBODY-wiki-9f3a-unique"
            }]
        })
        .to_string();
        project_wiki(&dir, body.as_bytes()).unwrap();
        let catalog = scan(&dir.join("missing"), &dir.join("missing"), &dir);
        assert_eq!(catalog.len(), 1);
        assert_eq!(catalog[0].kind, "wiki");
        assert_eq!(catalog[0].name, "产品手册甲");
        assert_eq!(catalog[0].description, "Saved handbook page");
        assert!(!catalog[0].description.contains("BODY-wiki-9f3a-unique"));
        let written = fs::read_to_string(dir.join("hub_manual1.md")).unwrap();
        assert!(written.contains("BODY-wiki-9f3a-unique"));
        project_wiki(&dir, br#"{"pages":[]}"#).unwrap();
        assert!(scan(&dir.join("missing"), &dir.join("missing"), &dir).is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn studio_wiki_root_replaces_the_tenant_projection() {
        let root = std::env::temp_dir().join(format!(
            "a3s-wiki-root-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let _ = fs::remove_dir_all(&root);
        let wiki = root.join("wiki");
        fs::create_dir_all(wiki.join("local")).unwrap();
        fs::write(wiki.join("local").join("食谱.md"), "红烧只写在个人笔记\n").unwrap();
        let body = json!({ "hostContext": { "wiki": wiki.display().to_string() } });
        assert_eq!(studio_wiki_root(&body).as_deref(), Some(wiki.as_path()));
        assert!(studio_wiki_root(&json!({ "hostContext": { "wiki": "wiki" } })).is_none());
        assert!(
            studio_wiki_root(&json!({ "hostContext": { "wiki": root.display().to_string() } }))
                .is_none()
        );
        let assets = with_studio_wiki(
            Some(json!({
                "connectors": root.join("capabilities").display().to_string(),
                "wiki": "/tenant/wiki",
                "catalog": [{ "kind": "wiki", "name": "旧页", "description": "旧说明" }]
            })),
            &wiki,
        );
        assert_eq!(assets["wiki"], wiki.display().to_string());
        assert_eq!(
            assets["connectors"],
            root.join("capabilities").display().to_string()
        );
        let catalog = assets["catalog"].to_string();
        assert!(!catalog.contains("旧页"));
        assert!(!catalog.contains("红烧只写在个人笔记"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn app_wiki_directory_uses_the_saved_workspace_or_the_default() {
        let home = Path::new("/Users/roylin");
        assert_eq!(
            app_wiki_directory("", home).as_deref(),
            Some(Path::new("/Users/roylin/.a3s/wiki"))
        );
        assert_eq!(
            app_wiki_directory("/tmp/work/", home).as_deref(),
            Some(Path::new("/tmp/work/wiki"))
        );
        assert_eq!(
            app_wiki_directory("relative", home).as_deref(),
            Some(Path::new("/Users/roylin/.a3s/wiki"))
        );
        assert_eq!(
            app_wiki_directory("/", home).as_deref(),
            Some(Path::new("/Users/roylin/.a3s/wiki"))
        );
        assert!(app_wiki_directory("/tmp/work", Path::new("")).is_none());
        assert!(app_wiki_directory("/tmp/work", Path::new("/")).is_none());
        assert_eq!(stored_workspace_preference("missing"), "");
    }
}
