use super::*;
use serde_json::json;

struct Job {
    directory: tempfile::TempDir,
    scope: CompileScope,
    workspace: PathBuf,
    input: Option<aster_okf::WorkspaceKnowledgeInput>,
}

impl Job {
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("wiki/local");
        std::fs::create_dir_all(&root).unwrap();
        let root = root.canonicalize().unwrap();
        std::fs::write(root.join("source.md"), "Frozen source evidence.").unwrap();
        let job =
            aster_okf::begin_knowledge_maintenance(&root, &[], aster_okf::MaintenanceOperation::Ingest)
                .unwrap();
        Self {
            directory,
            scope: CompileScope {
                root,
                job_id: job.id,
                workspace_revision: None,
            },
            workspace: job.staging.into(),
            input: None,
        }
    }
    fn new_workspace() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let input = aster_okf::WorkspaceKnowledgeInput::new(
            &directory.path().canonicalize().unwrap(),
            "tenant:permission:binding",
            &format!("sha256:{}", "a".repeat(64)),
            vec![aster_okf::WorkspaceKnowledgeSource::text(
                "src/lib.rs",
                b"pub fn answer() -> u8 { 42 }\n",
            )
            .unwrap()],
        )
        .unwrap();
        let job = aster_okf::begin_workspace_knowledge_maintenance(&input)
            .unwrap()
            .unwrap();
        Self {
            directory,
            scope: CompileScope {
                root: input.output_root().into(),
                job_id: job.id,
                workspace_revision: Some(input.binding_revision().into()),
            },
            workspace: job.staging.into(),
            input: Some(input),
        }
    }
    fn guard(&self) -> CompileGuard {
        CompileGuard::new(&self.scope, &self.workspace).unwrap()
    }
}

impl Drop for Job {
    fn drop(&mut self) {
        if let Some(input) = &self.input {
            let _ = aster_okf::cancel_workspace_knowledge_maintenance(input, &self.scope.job_id);
        } else {
            let _ = aster_okf::cancel_knowledge_maintenance(&self.scope.root, &self.scope.job_id);
        }
    }
}

#[test]
fn workspace_wiki_compile_prompt_excludes_the_ambient_asset_catalog() {
    let job = Job::new_workspace();
    let skills = job.directory.path().join("ambient-skills/secret-skill");
    std::fs::create_dir_all(&skills).unwrap();
    std::fs::write(skills.join("SKILL.md"), "---\nname: personal-only-sentinel\ndescription: Personal catalog sentinel\n---\nPrivate skill").unwrap();
    let mut host = crate::host_context::StudioHost {
        assets: Some(crate::local_assets::AssetRoots {
            skills: skills.parent().map(PathBuf::from),
            ..Default::default()
        }),
        ..Default::default()
    };
    let render = |host: &crate::host_context::StudioHost| {
        crate::host_context::merge_studio_host(aster_core::SystemPromptSlots::default(), host)
            .build()
    };
    assert!(render(&host).contains("personal-only-sentinel"));
    host.knowledge_compile = Some(job.scope.clone());
    assert!(!render(&host).contains("personal-only-sentinel"));
}

#[test]
fn workspace_wiki_compiler_uses_the_same_candidate_only_guard() {
    let job = Job::new_workspace();
    let guard = job.guard();
    let source = std::fs::read_dir(job.workspace.join("_sources"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    for (tool, args) in [
        ("read", json!({"file_path":source})),
        ("ls", json!({})),
        ("search", json!({"mode":"grep","pattern":"answer"})),
        ("write", json!({"file_path":"concepts/overview.md"})),
        (
            "functions.write",
            json!({"file_path":"concepts/overview.md"}),
        ),
        (
            "Functions.Write",
            json!({"file_path":"concepts/overview.md"}),
        ),
        ("write", json!({"file_path":".wiki-changes.json"})),
    ] {
        assert_eq!(
            guard.check(tool, &args),
            PermissionDecision::Allow,
            "{tool}: {args}"
        );
    }
    for (tool, args) in [
        ("write", json!({"file_path":source})),
        ("functions.write", json!({"file_path":source})),
        (
            "functions.functions.write",
            json!({"file_path":"concepts/overview.md"}),
        ),
        ("函数工具名称", json!({"file_path":"concepts/overview.md"})),
        (
            "batch",
            json!({"invocations":[{"tool":"functions.batch","args":{"invocations":[{"tool":"write","args":{"file_path":"concepts/a.md"}}]}}]}),
        ),
        (
            "read",
            json!({"file_path":job.directory.path().join("src/lib.rs")}),
        ),
        (
            "write",
            json!({"file_path":job.scope.root.join("concepts/overview.md")}),
        ),
        ("read", json!({"file_path":"../outside"})),
        ("bash", json!({"command":"true"})),
        ("web_fetch", json!({"url":"https://example.invalid"})),
        (
            "batch",
            json!({"invocations":[{"tool":"write","args":{"file_path":"concepts/a.md"}},{"tool":"write","args":{"file_path":source}}]}),
        ),
    ] {
        assert_eq!(
            guard.check(tool, &args),
            PermissionDecision::Deny,
            "{tool}: {args}"
        );
    }
    assert!(CompileGuard::new(&job.scope, job.directory.path()).is_err());
    let mut forged = job.scope.clone();
    forged.workspace_revision = Some("different-binding".into());
    assert!(CompileGuard::new(&forged, &job.workspace).is_err());
    forged.workspace_revision = None;
    assert!(CompileGuard::new(&forged, &job.workspace).is_err());
    let personal = Job::new();
    forged = personal.scope.clone();
    forged.workspace_revision = job.scope.workspace_revision.clone();
    assert!(CompileGuard::new(&forged, &personal.workspace).is_err());
    aster_okf::cancel_workspace_knowledge_maintenance(job.input.as_ref().unwrap(), &job.scope.job_id)
        .unwrap();
    assert_eq!(
        guard.check("write", &json!({"file_path":"concepts/a.md"})),
        PermissionDecision::Deny
    );
    assert!(CompileGuard::new(&job.scope, &job.workspace).is_err());
}

#[test]
fn workspace_wiki_compile_scope_refuses_unknown_or_untyped_fields_at_the_host_boundary() {
    let valid = json!({"root":"/tmp/wiki/workspace","jobId":"job","workspaceRevision":"scope"});
    let parse = |scope| {
        let meta = [("asterHost".into(), json!({"knowledgeCompile":scope}))]
            .into_iter()
            .collect();
        crate::host_context::studio_host_from_meta(Some(&meta))
    };
    assert!(parse(valid.clone()).unwrap().knowledge_compile.is_some());
    for scope in [
        json!({"root":"/tmp/wiki/workspace","jobId":"job","workspaceRevision":42}),
        json!({"root":"/tmp/wiki/workspace","jobId":"job","workspaceRevision":"scope","allowShell":true}),
        json!({"root":"/tmp/wiki/workspace","workspaceRevision":"scope"}),
    ] {
        assert!(parse(scope).is_err());
    }
}

#[test]
fn live_compiler_admits_candidate_writes_without_asking_and_preserves_explicit_denies() {
    let job = Job::new();
    let guard = job.guard();
    for (name, args) in [
        ("read", json!({"file_path":".wiki-task.md"})),
        (
            "read",
            json!({"files":[{"path":"source.md"},{"path":"concepts/alpha.md"}]}),
        ),
        ("ls", json!({})),
        (
            "write",
            json!({"file_path":"concepts/产品/极光.md","content":"Evidence"}),
        ),
        ("search", json!({"mode":"grep","pattern":"evidence"})),
        (
            "write",
            json!({"file_path":"concepts/alpha.md","content":"Evidence"}),
        ),
        (
            "edit",
            json!({"file_path":".wiki-changes.json","old_string":"a","new_string":"b"}),
        ),
        (
            "write",
            json!({"file_path":job.workspace.join("concepts/absolute.md"),"content":"Evidence"}),
        ),
        (
            "batch",
            json!({"invocations":[{"tool":"write","args":{"file_path":"concepts/alpha.md","content":"Evidence"}},{"tool":"write","args":{"file_path":".wiki-changes.json","content":"{}"}}]}),
        ),
    ] {
        assert_eq!(
            guard.check(name, &args),
            PermissionDecision::Allow,
            "{name}: {args}"
        );
        assert_eq!(policy(&[]).check(name, &args), PermissionDecision::Allow);
    }
    assert_eq!(
        policy(&["write".into()]).check("write", &json!({"file_path":"concepts/alpha.md"})),
        PermissionDecision::Deny
    );
    assert_eq!(
        aster_core::permissions::policy_for_posture("default")
            .check("write", &json!({"file_path":"concepts/alpha.md"})),
        PermissionDecision::Ask
    );
}

#[test]
fn compiler_refuses_source_edits_escape_execution_and_mixed_batches() {
    let job = Job::new();
    let guard = job.guard();
    for (name, args) in [
        ("write", json!({"file_path":"source.md"})),
        ("edit", json!({"file_path":"index.md"})),
        ("write", json!({"file_path":"concepts/index.md"})),
        ("write", json!({"file_path":"concepts/nested/index.md"})),
        ("write", json!({"file_path":"concepts/.hidden/a.md"})),
        ("write", json!({"file_path":"concepts/a/b/c/d/page.md"})),
        ("write", json!({"file_path":"../outside.md"})),
        (
            "write",
            json!({"file_path":job.scope.root.join("concepts/real-vault.md")}),
        ),
        ("read", json!({"file_path":"/etc/passwd"})),
        (
            "read",
            json!({"files":[{"path":"source.md"},{"path":"../secret"}]}),
        ),
        ("read", json!({"file_path":"source.md","files":[]})),
        ("search", json!({"mode":"glob","path":"../","pattern":"*"})),
        ("bash", json!({"command":"echo x"})),
        ("web_fetch", json!({"url":"https://example.com"})),
        ("office_file", json!({"file_path":"concepts/a.md"})),
        (
            "batch",
            json!({"invocations":[{"tool":"write","args":{"file_path":"concepts/alpha.md"}},{"tool":"write","args":{"file_path":"source.md"}}]}),
        ),
        ("batch", json!({"invocations":[{"tool":"batch","args":{}}]})),
        ("write", json!({"file_path":{"$ref":"read.output.path"}})),
        ("read", json!({"files":[]})),
    ] {
        assert_eq!(
            guard.check(name, &args),
            PermissionDecision::Deny,
            "{name}: {args}"
        );
    }
    for name in [
        "bash",
        "web_fetch",
        "task",
        "Skill",
        "office_file",
        "open_local_asset",
    ] {
        assert!(!guard.expose_to_model(name));
    }
    assert_eq!(
        std::fs::read_to_string(job.scope.root.join("source.md")).unwrap(),
        "Frozen source evidence."
    );
    assert!(!job.scope.root.join("concepts/real-vault.md").exists());
}

#[test]
fn compiler_cannot_bind_an_unowned_workspace_or_continue_after_stop_or_failure() {
    let job = Job::new();
    assert!(CompileGuard::new(&job.scope, job.directory.path()).is_err());
    let mut forged = job.scope.clone();
    forged.job_id = "0".repeat(64);
    assert!(CompileGuard::new(&forged, &job.workspace).is_err());
    let guard = job.guard();
    aster_okf::cancel_knowledge_maintenance(&job.scope.root, &job.scope.job_id).unwrap();
    assert_eq!(
        guard.check("write", &json!({"file_path":"concepts/a.md"})),
        PermissionDecision::Deny
    );
    assert!(CompileGuard::new(&job.scope, &job.workspace).is_err());
    let failed = Job::new();
    let guard = failed.guard();
    aster_okf::fail_knowledge_maintenance(&failed.scope.root, &failed.scope.job_id).unwrap();
    assert_eq!(
        guard.check("read", &json!({"file_path":"source.md"})),
        PermissionDecision::Deny
    );
}

#[test]
#[cfg(unix)]
fn compiler_refuses_symlinked_candidate_files_and_read_paths() {
    let job = Job::new();
    let guard = job.guard();
    std::os::unix::fs::symlink(
        job.scope.root.join("source.md"),
        job.workspace.join("concepts/linked.md"),
    )
    .unwrap();
    assert_eq!(
        guard.check("write", &json!({"file_path":"concepts/linked.md"})),
        PermissionDecision::Deny
    );
    assert_eq!(
        guard.check("read", &json!({"file_path":"concepts/linked.md"})),
        PermissionDecision::Deny
    );
}
