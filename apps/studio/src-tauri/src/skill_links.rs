//! One publisher for agent skill links.
//!
//! A link lives at `<agents>/<name>` and records its owner in
//! `.<name>.a3s-owner`. The review package also writes `.a3s-review-managed`.
//! A second owner does not replace the first. A real directory stays in place.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub const OWNER_CATALOG: &str = "catalog";
pub const OWNER_REVIEW: &str = "use/a3s/review";
pub const REVIEW_MARKER: &str = ".a3s-review-managed";
const REVIEW_NAME: &str = "review";

#[derive(Debug)]
pub enum ClaimError {
    HeldByOther,
    UserOwned,
    Io(io::Error),
}

pub fn owner_file(agents: &Path, name: &str) -> PathBuf {
    agents.join(format!(".{name}.a3s-owner"))
}

pub fn recorded_owner(agents: &Path, name: &str) -> Option<String> {
    let text = fs::read_to_string(owner_file(agents, name)).ok()?;
    let owner = text.trim();
    if owner.is_empty() {
        None
    } else {
        Some(owner.to_string())
    }
}

pub fn held_by_review(agents: &Path, name: &str) -> bool {
    recorded_owner(agents, name).as_deref() == Some(OWNER_REVIEW)
        || (name == REVIEW_NAME && agents.join(REVIEW_MARKER).is_file())
}

pub fn write_owner(agents: &Path, owner: &str, name: &str) -> io::Result<()> {
    fs::write(owner_file(agents, name), owner)
}

/// Point `agents/name` at `source`. `source` must be a directory that contains
/// `SKILL.md`.
pub fn claim_skill_link(
    agents: &Path,
    owner: &str,
    name: &str,
    source: &Path,
) -> Result<(), ClaimError> {
    if agents.as_os_str().is_empty() || name.is_empty() {
        return Err(ClaimError::Io(io::Error::new(
            io::ErrorKind::InvalidInput,
            "skill link needs an agents directory and a name",
        )));
    }
    let source = fs::canonicalize(source).map_err(ClaimError::Io)?;
    if !source.join("SKILL.md").is_file() {
        return Err(ClaimError::Io(io::Error::new(
            io::ErrorKind::InvalidData,
            "missing SKILL.md",
        )));
    }
    fs::create_dir_all(agents).map_err(ClaimError::Io)?;
    let dest = agents.join(name);
    match fs::symlink_metadata(&dest) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Ok(metadata) if metadata.file_type().is_symlink() => {
            if same_owner(agents, owner, name) {
                if links_to(&dest, &source) {
                    record(agents, owner, name, &source)?;
                    return Ok(());
                }
                fs::remove_file(&dest).map_err(ClaimError::Io)?;
            } else if owner == OWNER_REVIEW && bundled_review_link(&dest) {
                fs::remove_file(&dest).map_err(ClaimError::Io)?;
            } else {
                return Err(ClaimError::HeldByOther);
            }
        }
        Ok(_) => return Err(ClaimError::UserOwned),
        Err(error) => return Err(ClaimError::Io(error)),
    }
    place_link(&source, &dest)?;
    record(agents, owner, name, &source)?;
    Ok(())
}

/// Remove a link this owner created. A directory or another owner's link stays.
pub fn release_skill_link(agents: &Path, owner: &str, name: &str) -> Result<(), ClaimError> {
    if agents.as_os_str().is_empty() || name.is_empty() {
        return Ok(());
    }
    let dest = agents.join(name);
    let marker = agents.join(REVIEW_MARKER);
    let review_marker = owner == OWNER_REVIEW && name == REVIEW_NAME && marker.is_file();
    if recorded_owner(agents, name).as_deref() != Some(owner) && !review_marker {
        return Ok(());
    }
    if fs::symlink_metadata(&dest).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
        fs::remove_file(&dest).map_err(ClaimError::Io)?;
    }
    match fs::remove_file(owner_file(agents, name)) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(ClaimError::Io(error)),
    }
    if review_marker {
        match fs::remove_file(&marker) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(ClaimError::Io(error)),
        }
    }
    Ok(())
}

/// Review keeps a name only while `.a3s-review-managed` is present.
/// Deleting that marker releases the name even if the owner file remains.
fn same_owner(agents: &Path, owner: &str, name: &str) -> bool {
    if owner == OWNER_REVIEW && name == REVIEW_NAME && !agents.join(REVIEW_MARKER).is_file() {
        return false;
    }
    match recorded_owner(agents, name).as_deref() {
        Some(recorded) => recorded == owner,
        None => owner == OWNER_REVIEW && name == REVIEW_NAME,
    }
}

fn record(agents: &Path, owner: &str, name: &str, source: &Path) -> Result<(), ClaimError> {
    write_owner(agents, owner, name).map_err(ClaimError::Io)?;
    if owner == OWNER_REVIEW && name == REVIEW_NAME {
        fs::write(
            agents.join(REVIEW_MARKER),
            source.to_string_lossy().as_bytes(),
        )
        .map_err(ClaimError::Io)?;
    }
    Ok(())
}

fn links_to(link: &Path, source: &Path) -> bool {
    let Ok(metadata) = link.symlink_metadata() else {
        return false;
    };
    if !metadata.file_type().is_symlink() {
        return false;
    }
    let Ok(raw) = fs::read_link(link) else {
        return false;
    };
    let absolute = if raw.is_absolute() {
        raw
    } else {
        let Some(parent) = link.parent() else {
            return false;
        };
        parent.join(raw)
    };
    fs::canonicalize(&absolute)
        .ok()
        .is_some_and(|path| path.as_path() == source)
}

fn place_link(source: &Path, link: &Path) -> Result<(), ClaimError> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(source, link).map_err(ClaimError::Io)
    }
    #[cfg(not(unix))]
    {
        let _ = (source, link);
        Err(ClaimError::Io(io::Error::new(
            io::ErrorKind::Unsupported,
            "skill links are published on unix",
        )))
    }
}

pub(crate) fn bundled_review_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../resources/skills/review")
}

/// The app used to publish `review` as a symlink into its own skill bundle.
/// That link has no owner file. Review may replace it. Any other symlink stays.
fn bundled_review_link(dest: &Path) -> bool {
    let Ok(metadata) = fs::symlink_metadata(dest) else {
        return false;
    };
    if !metadata.file_type().is_symlink() {
        return false;
    }
    let Ok(raw) = fs::read_link(dest) else {
        return false;
    };
    let absolute = if raw.is_absolute() {
        raw
    } else {
        let Some(parent) = dest.parent() else {
            return false;
        };
        parent.join(raw)
    };
    let Some(bundled) = fs::canonicalize(bundled_review_dir()).ok() else {
        return false;
    };
    fs::canonicalize(&absolute).ok().as_ref() == Some(&bundled)
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use super::{ClaimError, OWNER_CATALOG, OWNER_REVIEW, claim_skill_link, release_skill_link};
    #[cfg(unix)]
    use std::fs;

    #[cfg(unix)]
    fn skill_dir(root: &std::path::Path, name: &str) -> std::path::PathBuf {
        let dir = root.join(name);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("SKILL.md"), format!("# {name}\n")).unwrap();
        fs::canonicalize(&dir).unwrap()
    }

    #[cfg(unix)]
    #[test]
    fn first_owner_keeps_the_name() {
        let root =
            std::env::temp_dir().join(format!("a3s-skill-link-owner-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let agents = root.join("agents");
        fs::create_dir_all(&agents).unwrap();
        let catalog = skill_dir(&root, "catalog-review");
        let review = skill_dir(&root, "review-pkg");

        claim_skill_link(&agents, OWNER_CATALOG, "review", &catalog).unwrap();
        let error = claim_skill_link(&agents, OWNER_REVIEW, "review", &review).unwrap_err();
        assert!(matches!(error, ClaimError::HeldByOther));
        assert_eq!(fs::read_link(agents.join("review")).unwrap(), catalog);
        assert_eq!(
            fs::read_to_string(agents.join("review/SKILL.md")).unwrap(),
            "# catalog-review\n"
        );

        release_skill_link(&agents, OWNER_CATALOG, "review").unwrap();
        claim_skill_link(&agents, OWNER_REVIEW, "review", &review).unwrap();
        let error = claim_skill_link(&agents, OWNER_CATALOG, "review", &catalog).unwrap_err();
        assert!(matches!(error, ClaimError::HeldByOther));
        assert_eq!(fs::read_link(agents.join("review")).unwrap(), review);
        assert!(agents.join(".a3s-review-managed").is_file());

        let user = root.join("user-review");
        fs::create_dir_all(&user).unwrap();
        fs::write(user.join("SKILL.md"), "# mine\n").unwrap();
        fs::remove_file(agents.join("review")).unwrap();
        fs::remove_file(agents.join(".a3s-review-managed")).unwrap();
        fs::remove_file(agents.join(".review.a3s-owner")).unwrap();
        std::os::unix::fs::symlink(&user, agents.join("review")).unwrap();
        let error = claim_skill_link(&agents, OWNER_REVIEW, "review", &review).unwrap_err();
        assert!(matches!(error, ClaimError::HeldByOther));
        assert_eq!(
            fs::read_to_string(agents.join("review/SKILL.md")).unwrap(),
            "# mine\n"
        );
        let _ = fs::remove_dir_all(&root);
    }
}
