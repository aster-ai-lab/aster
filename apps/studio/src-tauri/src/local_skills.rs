//! Skills present on this machine, for the composer's `/` list. Installed
//! packages live under `capabilities/skills/` and are linked into
//! `~/.agents/skills/`. Skills the user wrote under `~/.a3s/skills/` are listed
//! after those. Each skill is a directory holding a `SKILL.md`.

use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

const MAX_SKILLS: usize = 256;
const MAX_SKILL_MD: u64 = 64 * 1024;
const MAX_DESCRIPTION_CHARS: usize = 280;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Installed,
    User,
}

impl Source {
    fn as_str(self) -> &'static str {
        match self {
            Self::Installed => "installed",
            Self::User => "user",
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct LocalSkill {
    pub name: String,
    pub description: String,
    pub source: Source,
}

pub fn user_root() -> Option<PathBuf> {
    if cfg!(test) {
        return None;
    }
    std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".a3s").join("skills"))
}

/// User-level Agent Skills directory, `~/.agents/skills`.
pub fn agents_root() -> Option<PathBuf> {
    #[cfg(test)]
    {
        let slot = match TEST_AGENTS.lock() {
            Ok(slot) => slot,
            Err(poisoned) => poisoned.into_inner(),
        };
        return slot.clone();
    }
    #[cfg(not(test))]
    {
        std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".agents").join("skills"))
    }
}

#[cfg(test)]
static TEST_AGENTS: std::sync::Mutex<Option<PathBuf>> = std::sync::Mutex::new(None);

#[cfg(test)]
pub fn set_test_agents_root(path: Option<PathBuf>) {
    let mut slot = match TEST_AGENTS.lock() {
        Ok(slot) => slot,
        Err(poisoned) => poisoned.into_inner(),
    };
    *slot = path;
}

/// Earlier roots win when two skills share a name.
pub fn scan(roots: &[(PathBuf, Source)]) -> Vec<LocalSkill> {
    let mut found: BTreeMap<String, LocalSkill> = BTreeMap::new();
    for (root, source) in roots {
        let Ok(entries) = fs::read_dir(root) else {
            continue;
        };
        let mut dirs: Vec<PathBuf> = entries
            .filter_map(Result::ok)
            .filter(|entry| {
                entry.file_type().is_ok_and(|kind| {
                    kind.is_dir()
                        || (kind.is_symlink()
                            && fs::metadata(entry.path()).is_ok_and(|meta| meta.is_dir()))
                })
            })
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| !name.starts_with('.'))
            })
            .collect();
        dirs.sort();
        for dir in dirs {
            if found.len() >= MAX_SKILLS {
                break;
            }
            let Some(skill) = read_skill(&dir, *source) else {
                continue;
            };
            found.entry(skill.name.clone()).or_insert(skill);
        }
    }
    found.into_values().collect()
}

pub fn list_json(roots: &[(PathBuf, Source)]) -> String {
    let skills: Vec<Value> = scan(roots)
        .into_iter()
        .map(|skill| {
            json!({
                "name": skill.name,
                "description": skill.description,
                "source": skill.source.as_str(),
            })
        })
        .collect();
    let listed_roots: Vec<Value> = roots
        .iter()
        .filter_map(|(path, source)| {
            let path = path.to_str()?;
            Some(json!({ "path": path, "source": source.as_str() }))
        })
        .collect();
    json!({ "roots": listed_roots, "skills": skills }).to_string()
}

fn read_skill(dir: &Path, source: Source) -> Option<LocalSkill> {
    let path = dir.join("SKILL.md");
    let meta = fs::symlink_metadata(&path).ok()?;
    if !meta.is_file() {
        return None;
    }
    let mut text = String::new();
    fs::File::open(&path)
        .ok()?
        .take(MAX_SKILL_MD)
        .read_to_string(&mut text)
        .ok()?;
    let (name, description) = frontmatter(&text);
    let name = name
        .or_else(|| dir.file_name()?.to_str().map(str::to_owned))
        .filter(|name| !name.is_empty())?;
    Some(LocalSkill {
        name,
        description: description
            .unwrap_or_default()
            .chars()
            .take(MAX_DESCRIPTION_CHARS)
            .collect(),
        source,
    })
}

/// `name` and `description` from a leading `---` YAML block of plain scalars.
fn frontmatter(text: &str) -> (Option<String>, Option<String>) {
    let mut lines = text.lines();
    if lines.next().map(str::trim) != Some("---") {
        return (None, None);
    }
    let mut name = None;
    let mut description = None;
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
        match key.trim() {
            "name" => name = Some(value),
            "description" => description = Some(value),
            _ => {}
        }
    }
    (name, description)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(tag: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("a3s-local-skills-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn skill(root: &Path, dir: &str, body: &str) {
        fs::create_dir_all(root.join(dir)).unwrap();
        fs::write(root.join(dir).join("SKILL.md"), body).unwrap();
    }

    #[test]
    fn reads_frontmatter_and_prefers_earlier_roots() {
        let installed = temp_root("installed");
        let user = temp_root("user");
        skill(
            &installed,
            "pdf",
            "---\nname: pdf\ndescription: \"Fill and merge PDFs\"\n---\n# PDF\n",
        );
        skill(
            &user,
            "pdf-copy",
            "---\nname: pdf\ndescription: shadowed\n---\n",
        );
        skill(&user, "notes", "# no frontmatter\n");
        skill(&user, ".hidden", "---\nname: hidden\n---\n");
        fs::create_dir_all(user.join("empty")).unwrap();

        let skills = scan(&[
            (installed.clone(), Source::Installed),
            (user.clone(), Source::User),
        ]);

        assert_eq!(
            skills,
            vec![
                LocalSkill {
                    name: "notes".into(),
                    description: String::new(),
                    source: Source::User,
                },
                LocalSkill {
                    name: "pdf".into(),
                    description: "Fill and merge PDFs".into(),
                    source: Source::Installed,
                },
            ]
        );
        let _ = fs::remove_dir_all(installed);
        let _ = fs::remove_dir_all(user);
    }

    #[cfg(unix)]
    #[test]
    fn follows_a_directory_symlink() {
        let root = temp_root("links");
        let real = temp_root("real");
        skill(
            &real,
            "find-skills",
            "---\nname: find-skills\ndescription: Find skills\n---\n",
        );
        std::os::unix::fs::symlink(real.join("find-skills"), root.join("find-skills")).unwrap();
        std::os::unix::fs::symlink(
            real.join("find-skills").join("SKILL.md"),
            root.join("not-a-dir"),
        )
        .unwrap();
        std::os::unix::fs::symlink(root.join("missing"), root.join("dangling")).unwrap();

        let skills = scan(&[(root.clone(), Source::User)]);
        assert_eq!(
            skills,
            vec![LocalSkill {
                name: "find-skills".into(),
                description: "Find skills".into(),
                source: Source::User,
            }]
        );
        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_dir_all(&real);
    }

    #[test]
    fn missing_roots_yield_an_empty_list() {
        let json = list_json(&[(PathBuf::from("/nonexistent/a3s/skills"), Source::User)]);
        assert_eq!(
            json,
            r#"{"roots":[{"path":"/nonexistent/a3s/skills","source":"user"}],"skills":[]}"#
        );
    }
}
