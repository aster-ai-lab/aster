//! Git worktree and GitHub pull request for the code work mode.
//! Commands use argv. They do not open a shell.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeWorktree {
    path: String,
    branch: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodePullRequest {
    url: String,
}

fn validate_cwd(cwd: &str) -> Result<&Path, String> {
    if cwd.is_empty() || cwd.contains('\0') || cwd.contains('\n') || cwd.contains('\r') {
        return Err("invalid-cwd".to_string());
    }
    let path = Path::new(cwd);
    if !path.is_absolute() {
        return Err("invalid-cwd".to_string());
    }
    if !path.is_dir() {
        return Err("missing-cwd".to_string());
    }
    Ok(path)
}

fn validate_branch(branch: &str) -> Result<(), String> {
    let first_ok = branch
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_alphanumeric());
    let body_ok = branch
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '.' || ch == '_' || ch == '/' || ch == '-');
    if first_ok && body_ok && branch.len() <= 100 && !branch.contains("..") {
        Ok(())
    } else {
        Err("invalid-branch".to_string())
    }
}

fn worktree_destination(cwd: &Path, branch: &str) -> PathBuf {
    let name = cwd
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("repo");
    let folder = format!("{name}-{}", branch.replace(['\\', '/'], "-"));
    match cwd.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.join(folder),
        _ => PathBuf::from(folder),
    }
}

fn run_argv(program: &str, cwd: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new(program)
        .args(args)
        .current_dir(cwd)
        .output()
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                format!("{program}-missing")
            } else {
                error.to_string()
            }
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if stderr.is_empty() {
            return Err(format!("{program} failed"));
        }
        return Err(stderr);
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn clamp(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

fn pull_request_url(stdout: &str) -> Result<String, String> {
    let url = stdout
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with("http://") || line.starts_with("https://"))
        .unwrap_or("")
        .to_string();
    if url.is_empty() {
        Err("pr-url-missing".to_string())
    } else {
        Ok(url)
    }
}

fn worktree_add_args<'a>(
    branch: &'a str,
    dest: &'a str,
    start: Option<&'a str>,
    existing: bool,
) -> Result<Vec<&'a str>, String> {
    validate_branch(branch)?;
    if existing {
        return Ok(vec!["worktree", "add", dest, branch]);
    }
    let mut args = vec!["worktree", "add", "-b", branch, dest];
    if let Some(start) = start.filter(|value| !value.is_empty()) {
        validate_branch(start)?;
        args.push(start);
    }
    Ok(args)
}

#[tauri::command]
pub fn code_worktree_add(
    cwd: String,
    branch: String,
    start: Option<String>,
    existing: Option<bool>,
) -> Result<CodeWorktree, String> {
    let cwd_path = validate_cwd(&cwd)?;
    let dest = worktree_destination(cwd_path, &branch);
    let dest_text = dest.to_string_lossy().to_string();
    let args = worktree_add_args(
        &branch,
        &dest_text,
        start.as_deref(),
        existing.unwrap_or(false),
    )?;
    run_argv("git", cwd_path, &args)?;
    Ok(CodeWorktree {
        path: dest_text,
        branch,
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeBranches {
    head: String,
    branches: Vec<String>,
}

#[tauri::command]
pub fn code_branches(cwd: String) -> Result<CodeBranches, String> {
    let cwd_path = validate_cwd(&cwd)?;
    let head = run_argv("git", cwd_path, &["rev-parse", "--abbrev-ref", "HEAD"])?;
    let listed = run_argv(
        "git",
        cwd_path,
        &["for-each-ref", "--format=%(refname:short)", "refs/heads"],
    )?;
    let branches = listed
        .lines()
        .map(str::trim)
        .filter(|name| !name.is_empty() && validate_branch(name).is_ok())
        .take(200)
        .map(str::to_string)
        .collect();
    Ok(CodeBranches {
        head: head.trim().to_string(),
        branches,
    })
}

#[tauri::command]
pub fn code_pull_request(
    cwd: String,
    branch: Option<String>,
    title: String,
    body: String,
) -> Result<CodePullRequest, String> {
    let cwd_path = validate_cwd(&cwd)?;
    let head = match branch {
        Some(value) if !value.trim().is_empty() => {
            validate_branch(value.trim())?;
            value.trim().to_string()
        }
        _ => {
            let name = run_argv("git", cwd_path, &["rev-parse", "--abbrev-ref", "HEAD"])?;
            let name = name.trim().to_string();
            if name.is_empty() || name == "HEAD" {
                return Err("detached".to_string());
            }
            validate_branch(&name)?;
            name
        }
    };
    run_argv("git", cwd_path, &["push", "-u", "origin", &head])?;
    let stdout = run_argv(
        "gh",
        cwd_path,
        &[
            "pr",
            "create",
            "--head",
            &head,
            "--title",
            &clamp(&title, 120),
            "--body",
            &clamp(&body, 4000),
        ],
    )?;
    Ok(CodePullRequest {
        url: pull_request_url(&stdout)?,
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitWorktreeCommand {
    pub cwd: String,
    pub action: String,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub paths: Option<Vec<String>>,
    #[serde(default)]
    pub branch: Option<String>,
    #[serde(default)]
    pub since_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GitWorktreeRow {
    pub id: String,
    pub path: String,
    pub dirty: bool,
    pub primary: bool,
    pub mtime_ms: u64,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct GitWorktreeOutput {
    pub items: Vec<GitWorktreeRow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refused: Option<String>,
}

fn output(
    items: Vec<GitWorktreeRow>,
    error: Option<String>,
    refused: Option<String>,
) -> GitWorktreeOutput {
    GitWorktreeOutput {
        items,
        error,
        refused,
    }
}

fn parse_worktree_paths(stdout: &str) -> Vec<String> {
    stdout
        .lines()
        .filter_map(|line| line.strip_prefix("worktree "))
        .map(|path| path.trim().trim_matches('"').to_string())
        .filter(|path| !path.is_empty())
        .collect()
}

fn mtime_ms(path: &Path) -> u64 {
    std::fs::metadata(path)
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| u64::try_from(duration.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

fn is_dirty(path: &Path) -> bool {
    match run_argv("git", path, &["status", "--porcelain"]) {
        Ok(stdout) => !stdout.trim().is_empty(),
        Err(_) => true,
    }
}

fn list_worktrees(cwd: &Path) -> Result<Vec<GitWorktreeRow>, String> {
    let stdout = run_argv("git", cwd, &["worktree", "list", "--porcelain"])?;
    let mut items = Vec::new();
    for (index, path) in parse_worktree_paths(&stdout).into_iter().enumerate() {
        let worktree = PathBuf::from(&path);
        items.push(GitWorktreeRow {
            id: path.clone(),
            dirty: is_dirty(&worktree),
            primary: index == 0,
            mtime_ms: mtime_ms(&worktree),
            path,
        });
    }
    Ok(items)
}

fn restore_path_allowed(file: &str) -> bool {
    if file.is_empty() || file.contains(['\0', '\n', '\r']) {
        return false;
    }
    let path = Path::new(file);
    !path.is_absolute()
        && path
            .components()
            .all(|component| !matches!(component, std::path::Component::ParentDir))
}

/// Lists, restores, removes, creates, or clears worktrees.
/// A refusal keeps the current list. The empty-task picker does not send create.
#[tauri::command]
pub fn git_worktrees(input: GitWorktreeCommand) -> GitWorktreeOutput {
    let cwd = match validate_cwd(&input.cwd) {
        Ok(path) => path,
        Err(error) => return output(Vec::new(), Some(error), None),
    };
    match input.action.as_str() {
        "list" => match list_worktrees(cwd) {
            Ok(items) => output(items, None, None),
            Err(error) => output(Vec::new(), Some(error), None),
        },
        "restore" => {
            let files = input.paths.unwrap_or_default();
            if files.iter().any(|file| !restore_path_allowed(file)) {
                let items = list_worktrees(cwd).unwrap_or_default();
                return output(items, Some("invalid-path".to_string()), None);
            }
            if !files.is_empty() {
                let mut args = vec!["restore", "--source=HEAD", "--worktree", "--"];
                args.extend(files.iter().map(String::as_str));
                if let Err(error) = run_argv("git", cwd, &args) {
                    return output(Vec::new(), Some(error), None);
                }
            }
            match list_worktrees(cwd) {
                Ok(items) => output(items, None, None),
                Err(error) => output(Vec::new(), Some(error), None),
            }
        }
        "remove" => {
            let items = match list_worktrees(cwd) {
                Ok(items) => items,
                Err(error) => return output(Vec::new(), Some(error), None),
            };
            let wanted = input.path.unwrap_or_default();
            let found = items
                .iter()
                .find(|item| item.path == wanted || item.id == wanted)
                .map(|item| (item.primary, item.dirty, item.path.clone()));
            let Some((primary, dirty, target_path)) = found else {
                return output(items, None, Some("missing".to_string()));
            };
            if primary {
                return output(items, None, Some("primary".to_string()));
            }
            if dirty {
                return output(items, None, Some("dirty".to_string()));
            }
            if run_argv("git", cwd, &["worktree", "remove", &target_path]).is_err() {
                return output(items, None, Some("dirty".to_string()));
            }
            match list_worktrees(cwd) {
                Ok(items) => output(items, None, None),
                Err(error) => output(Vec::new(), Some(error), None),
            }
        }
        "clear" => {
            let items = match list_worktrees(cwd) {
                Ok(items) => items,
                Err(error) => return output(Vec::new(), Some(error), None),
            };
            let since = input.since_ms.unwrap_or(0);
            for item in &items {
                if item.primary || item.dirty || item.mtime_ms < since {
                    continue;
                }
                let _ = run_argv("git", cwd, &["worktree", "remove", &item.path]);
            }
            match list_worktrees(cwd) {
                Ok(next) => output(next, None, None),
                Err(error) => output(items, Some(error), None),
            }
        }
        "create" => {
            let items = match list_worktrees(cwd) {
                Ok(items) => items,
                Err(error) => return output(Vec::new(), Some(error), None),
            };
            let branch = input.branch.unwrap_or_default();
            if validate_branch(&branch).is_err() {
                return output(items, Some("invalid-branch".to_string()), None);
            }
            let dest = worktree_destination(cwd, &branch);
            let dest_text = dest.to_string_lossy().to_string();
            let args = match worktree_add_args(&branch, &dest_text, None, false) {
                Ok(args) => args,
                Err(error) => return output(items, Some(error), None),
            };
            if let Err(error) = run_argv("git", cwd, &args) {
                return output(items, Some(error), None);
            }
            match list_worktrees(cwd) {
                Ok(next) => output(next, None, None),
                Err(error) => output(items, Some(error), None),
            }
        }
        _ => match list_worktrees(cwd) {
            Ok(items) => output(items, Some("unsupported".to_string()), None),
            Err(error) => output(Vec::new(), Some(error), None),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{
        GitWorktreeCommand, git_worktrees, validate_branch, validate_cwd, worktree_add_args,
        worktree_destination,
    };
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    struct TempDir(PathBuf);
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn branch_rejects_traversal_and_empty() {
        assert!(validate_branch("code/task-ab12").is_ok());
        assert_eq!(validate_branch("").unwrap_err(), "invalid-branch");
        assert_eq!(
            validate_branch("code/../main").unwrap_err(),
            "invalid-branch"
        );
        assert_eq!(validate_branch("-bad").unwrap_err(), "invalid-branch");
    }

    #[test]
    fn destination_is_a_sibling_of_the_checkout() {
        let cwd = Path::new("/tmp/demo");
        let dest = worktree_destination(cwd, "code/task-ab12");
        assert_eq!(
            dest.file_name().and_then(|value| value.to_str()),
            Some("demo-code-task-ab12")
        );
        assert_eq!(dest.parent(), cwd.parent());
    }

    #[test]
    fn worktree_starts_at_head_unless_a_base_is_named() {
        let bare =
            worktree_add_args("code/task-ab12", "/tmp/demo-code-task-ab12", None, false).unwrap();
        assert_eq!(
            bare,
            [
                "worktree",
                "add",
                "-b",
                "code/task-ab12",
                "/tmp/demo-code-task-ab12"
            ]
        );
        let based = worktree_add_args(
            "feature/billing",
            "/tmp/demo-feature-billing",
            Some("main"),
            false,
        )
        .unwrap();
        assert_eq!(based.last().copied(), Some("main"));
        assert_eq!(
            worktree_add_args("feature/billing", "/tmp/out", Some(".."), false).unwrap_err(),
            "invalid-branch"
        );
        assert_eq!(
            worktree_add_args("release", "/tmp/demo-release", Some("main"), true).unwrap(),
            ["worktree", "add", "/tmp/demo-release", "release"]
        );
    }

    #[test]
    fn cwd_must_be_an_existing_absolute_directory() {
        assert_eq!(validate_cwd("relative").unwrap_err(), "invalid-cwd");
        assert_eq!(
            validate_cwd("/no/such/studio-code-cwd").unwrap_err(),
            "missing-cwd"
        );
        let temp = std::env::temp_dir();
        let text = temp.to_string_lossy().to_string();
        assert!(validate_cwd(&text).is_ok());
    }

    fn git(cwd: &Path, args: &[&str]) {
        let output = Command::new("git")
            .args([
                "-c",
                "user.email=t@example.com",
                "-c",
                "user.name=Test",
                "-c",
                "commit.gpgsign=false",
                "-c",
                "init.defaultBranch=main",
            ])
            .args(args)
            .current_dir(cwd)
            .env("GIT_TERMINAL_PROMPT", "0")
            .output()
            .expect("git");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn command(
        cwd: &Path,
        action: &str,
        path: Option<String>,
        paths: Option<Vec<String>>,
    ) -> GitWorktreeCommand {
        GitWorktreeCommand {
            cwd: cwd.to_string_lossy().to_string(),
            action: action.to_string(),
            path,
            paths,
            branch: None,
            since_ms: None,
        }
    }

    #[test]
    fn git_worktrees_lists_restores_and_refuses_dirty_remove() {
        let stamp = std::process::id();
        let root = TempDir(std::env::temp_dir().join(format!("studio-git-worktrees-{stamp}")));
        let feature =
            TempDir(std::env::temp_dir().join(format!("studio-git-worktrees-feature-{stamp}")));
        let _ = fs::remove_dir_all(&root.0);
        let _ = fs::remove_dir_all(&feature.0);
        fs::create_dir_all(root.0.join("src")).unwrap();
        git(&root.0, &["init", "-b", "main"]);
        fs::write(root.0.join("src/a.ts"), "one\n").unwrap();
        git(&root.0, &["add", "src/a.ts"]);
        git(&root.0, &["commit", "-m", "init"]);
        git(
            &root.0,
            &[
                "worktree",
                "add",
                "-b",
                "feature",
                feature.0.to_str().unwrap(),
            ],
        );

        let listed = git_worktrees(command(&root.0, "list", None, None));
        assert!(listed.error.is_none());
        assert_eq!(listed.items.len(), 2);
        assert!(listed.items[0].primary);
        assert!(!listed.items[0].dirty);
        let feature_path = listed
            .items
            .iter()
            .find(|item| !item.primary)
            .expect("feature worktree")
            .path
            .clone();
        assert!(listed.items.iter().any(|item| !item.primary && !item.dirty));

        fs::write(Path::new(&feature_path).join("src/a.ts"), "dirty\n").unwrap();
        let dirty = git_worktrees(command(&root.0, "list", None, None));
        assert!(
            dirty
                .items
                .iter()
                .find(|item| item.path == feature_path)
                .unwrap()
                .dirty
        );
        let refused = git_worktrees(command(&root.0, "remove", Some(feature_path.clone()), None));
        assert_eq!(refused.refused.as_deref(), Some("dirty"));
        assert!(Path::new(&feature_path).join("src/a.ts").is_file());

        let blocked = git_worktrees(command(
            Path::new(&feature_path),
            "restore",
            None,
            Some(vec!["../outside.ts".to_string()]),
        ));
        assert_eq!(blocked.error.as_deref(), Some("invalid-path"));
        assert_eq!(
            fs::read_to_string(Path::new(&feature_path).join("src/a.ts")).unwrap(),
            "dirty\n"
        );

        let restored = git_worktrees(command(
            Path::new(&feature_path),
            "restore",
            None,
            Some(vec!["src/a.ts".to_string()]),
        ));
        assert!(restored.error.is_none());
        assert_eq!(
            fs::read_to_string(Path::new(&feature_path).join("src/a.ts")).unwrap(),
            "one\n"
        );
        assert!(
            restored
                .items
                .iter()
                .find(|item| item.path == feature_path)
                .unwrap()
                .dirty
                == false
        );

        let primary = git_worktrees(command(
            &root.0,
            "remove",
            Some(listed.items[0].path.clone()),
            None,
        ));
        assert_eq!(primary.refused.as_deref(), Some("primary"));
        let removed = git_worktrees(command(&root.0, "remove", Some(feature_path.clone()), None));
        assert!(removed.error.is_none());
        assert!(removed.refused.is_none());
        assert!(removed.items.iter().all(|item| item.path != feature_path));

        let kept = git_worktrees(command(&root.0, "nope", None, None));
        assert_eq!(kept.error.as_deref(), Some("unsupported"));
        assert_eq!(kept.items.len(), 1);
        let refused = git_worktrees(command(&root.0, "create", None, None));
        assert_eq!(refused.error.as_deref(), Some("invalid-branch"));
        assert_eq!(refused.items.len(), 1);
        let mut bad = command(&root.0, "create", None, None);
        bad.branch = Some("../x".to_string());
        let bad = git_worktrees(bad);
        assert_eq!(bad.error.as_deref(), Some("invalid-branch"));
        assert_eq!(bad.items.len(), 1);

        let dest = worktree_destination(&root.0, "task");
        let _ = fs::remove_dir_all(&dest);
        let mut create = command(&root.0, "create", None, None);
        create.branch = Some("task".to_string());
        let created = git_worktrees(create);
        assert!(created.error.is_none(), "{:?}", created.error);
        assert_eq!(created.items.len(), 2);
        let created_path = created
            .items
            .iter()
            .find(|item| !item.primary)
            .expect("created worktree")
            .path
            .clone();
        assert_eq!(
            fs::canonicalize(&created_path).unwrap(),
            fs::canonicalize(&dest).unwrap()
        );
        assert_eq!(
            fs::read_to_string(Path::new(&created_path).join("src/a.ts")).unwrap(),
            "one\n"
        );
        let mut again = command(&root.0, "create", None, None);
        again.branch = Some("task".to_string());
        let again = git_worktrees(again);
        assert!(again.error.is_some());
        assert_eq!(again.items.len(), 2);

        fs::write(Path::new(&created_path).join("src/a.ts"), "dirty\n").unwrap();
        let mut clear = command(&root.0, "clear", None, None);
        clear.since_ms = Some(0);
        let dirty = git_worktrees(clear);
        assert!(dirty.items.iter().any(|item| item.path == created_path));
        assert_eq!(
            fs::read_to_string(Path::new(&created_path).join("src/a.ts")).unwrap(),
            "dirty\n"
        );
        let restored = git_worktrees(command(
            Path::new(&created_path),
            "restore",
            None,
            Some(vec!["src/a.ts".to_string()]),
        ));
        assert!(restored.error.is_none());
        let mut future = command(&root.0, "clear", None, None);
        future.since_ms = Some(u64::MAX);
        let future = git_worktrees(future);
        assert!(future.items.iter().any(|item| item.path == created_path));
        let mut clear = command(&root.0, "clear", None, None);
        clear.since_ms = Some(0);
        let cleared = git_worktrees(clear);
        assert!(cleared.error.is_none(), "{:?}", cleared.error);
        assert!(cleared.items.iter().all(|item| item.path != created_path));
        assert!(cleared.items.iter().any(|item| item.primary));
        assert!(!Path::new(&created_path).exists());
        assert!(!dest.exists());
    }
}
