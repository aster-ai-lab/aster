#![cfg(test)]

#[cfg(unix)]
mod unix {
    use std::fs;
    use std::os::unix::fs::{PermissionsExt, symlink};
    use std::path::PathBuf;
    use std::sync::{Arc, Barrier};

    use super::super::WorkspaceSnapshots;

    struct Workspace(PathBuf);

    impl Workspace {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!("a3s-snapshot-{}", uuid::Uuid::now_v7()));
            fs::create_dir(&root).unwrap();
            fs::write(root.join("note.txt"), b"original").unwrap();
            Self(root)
        }
        fn root(&self) -> &str {
            self.0.to_str().unwrap()
        }
        fn path(&self) -> String {
            self.0.join("note.txt").to_str().unwrap().to_owned()
        }
    }
    impl Drop for Workspace {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn saves_against_exact_observed_bytes_and_keeps_the_new_revision() {
        let workspace = Workspace::new();
        let authority = WorkspaceSnapshots::default();
        let first = authority.read(workspace.root(), &workspace.path()).unwrap();
        assert_eq!(first.bytes, b"original");
        let next = authority
            .write(
                workspace.root(),
                &workspace.path(),
                &first.revision,
                b"changed",
            )
            .unwrap();
        assert_ne!(next.revision, first.revision);
        assert_eq!(fs::read(workspace.path()).unwrap(), b"changed");
        authority
            .write(
                workspace.root(),
                &workspace.path(),
                &next.revision,
                b"again",
            )
            .unwrap();
        assert_eq!(fs::read(workspace.path()).unwrap(), b"again");
        assert_eq!(fs::read_dir(&workspace.0).unwrap().count(), 1);
    }

    #[test]
    fn portable_adapter_preserves_bytes_across_conflicts_and_exact_address_changes() {
        use super::super::portable;
        let workspace = Workspace::new();
        let first = portable::read(workspace.root(), &workspace.path()).unwrap();
        let next =
            portable::write(workspace.root(), &workspace.path(), &first.revision, b"new").unwrap();
        assert_eq!(fs::read(workspace.path()).unwrap(), b"new");
        assert_eq!(
            portable::write(
                workspace.root(),
                &workspace.path(),
                &first.revision,
                b"stale"
            )
            .err(),
            Some(409)
        );
        let replacement = workspace.0.join("replacement");
        fs::write(&replacement, b"new").unwrap();
        fs::rename(replacement, workspace.path()).unwrap();
        assert_eq!(
            portable::write(
                workspace.root(),
                &workspace.path(),
                &next.revision,
                b"stale"
            )
            .err(),
            Some(409)
        );
        let outside = Workspace::new();
        assert_eq!(
            portable::read(workspace.root(), &outside.path()).err(),
            Some(404)
        );
        assert_eq!(
            portable::write(outside.root(), &outside.path(), &next.revision, b"foreign").err(),
            Some(409)
        );
        assert_eq!(fs::read(outside.path()).unwrap(), b"original");
        assert_eq!(fs::read(workspace.path()).unwrap(), b"new");
        assert_eq!(fs::read_dir(&workspace.0).unwrap().count(), 1);
    }

    #[test]
    fn independent_observers_cannot_both_replace_the_same_version() {
        let workspace = Workspace::new();
        let authority = Arc::new(WorkspaceSnapshots::default());
        let barrier = Arc::new(Barrier::new(2));
        let revision = authority
            .read(workspace.root(), &workspace.path())
            .unwrap()
            .revision;
        let jobs: Vec<_> = [b"first".as_slice(), b"second".as_slice()]
            .into_iter()
            .map(|bytes| {
                let authority = authority.clone();
                let barrier = barrier.clone();
                let revision = revision.clone();
                let root = workspace.root().to_owned();
                let path = workspace.path();
                std::thread::spawn(move || {
                    barrier.wait();
                    authority.write(&root, &path, &revision, bytes).map(|_| ())
                })
            })
            .collect();
        let results: Vec<_> = jobs.into_iter().map(|job| job.join().unwrap()).collect();
        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
        assert_eq!(
            results.iter().filter(|result| **result == Err(409)).count(),
            1
        );
        assert!(
            [b"first".as_slice(), b"second".as_slice()]
                .contains(&fs::read(workspace.path()).unwrap().as_slice())
        );
    }

    #[test]
    fn external_content_change_refuses_the_old_save_without_touching_bytes() {
        let workspace = Workspace::new();
        let authority = WorkspaceSnapshots::default();
        let revision = authority
            .read(workspace.root(), &workspace.path())
            .unwrap()
            .revision;
        fs::write(workspace.path(), b"external").unwrap();
        assert_eq!(
            authority
                .write(workspace.root(), &workspace.path(), &revision, b"stale")
                .err(),
            Some(409)
        );
        assert_eq!(fs::read(workspace.path()).unwrap(), b"external");
        assert_eq!(fs::read_dir(&workspace.0).unwrap().count(), 1);
    }

    #[test]
    fn restoring_original_bytes_does_not_restore_a_consumed_revision() {
        let workspace = Workspace::new();
        let authority = WorkspaceSnapshots::default();
        let first = authority.read(workspace.root(), &workspace.path()).unwrap();
        let second = authority
            .write(
                workspace.root(),
                &workspace.path(),
                &first.revision,
                b"intermediate",
            )
            .unwrap();
        authority
            .write(
                workspace.root(),
                &workspace.path(),
                &second.revision,
                b"original",
            )
            .unwrap();
        assert_eq!(
            authority
                .write(
                    workspace.root(),
                    &workspace.path(),
                    &first.revision,
                    b"stale"
                )
                .err(),
            Some(409)
        );
        assert_eq!(fs::read(workspace.path()).unwrap(), b"original");
    }

    #[test]
    fn a_same_content_file_replacement_invalidates_observation() {
        let workspace = Workspace::new();
        let authority = WorkspaceSnapshots::default();
        let revision = authority
            .read(workspace.root(), &workspace.path())
            .unwrap()
            .revision;
        let other = workspace.0.join("replacement");
        fs::write(&other, b"original").unwrap();
        fs::rename(other, workspace.path()).unwrap();
        assert_eq!(
            authority
                .write(workspace.root(), &workspace.path(), &revision, b"stale")
                .err(),
            Some(409)
        );
        assert_eq!(fs::read(workspace.path()).unwrap(), b"original");
    }

    #[test]
    fn root_replacement_and_alias_retarget_cannot_reuse_an_observation() {
        let first = Workspace::new();
        let second = Workspace::new();
        let authority = WorkspaceSnapshots::default();
        let alias = first.0.join("alias");
        symlink(&first.0, &alias).unwrap();
        let path = alias.join("note.txt").to_str().unwrap().to_owned();
        let revision = authority
            .read(alias.to_str().unwrap(), &path)
            .unwrap()
            .revision;
        fs::remove_file(&alias).unwrap();
        symlink(&second.0, &alias).unwrap();
        assert_eq!(
            authority
                .write(alias.to_str().unwrap(), &path, &revision, b"stale")
                .err(),
            Some(409)
        );
        let revision = authority
            .read(first.root(), &first.path())
            .unwrap()
            .revision;
        let displaced = first.0.with_extension("old");
        fs::rename(&first.0, &displaced).unwrap();
        fs::create_dir(&first.0).unwrap();
        fs::write(first.path(), b"original").unwrap();
        assert_eq!(
            authority
                .write(first.root(), &first.path(), &revision, b"stale")
                .err(),
            Some(409)
        );
        assert_eq!(fs::read(first.path()).unwrap(), b"original");
        assert_eq!(fs::read(second.path()).unwrap(), b"original");
        fs::remove_dir_all(displaced).unwrap();
    }

    #[test]
    fn revisions_are_bound_to_workspace_and_address() {
        let first = Workspace::new();
        let second = Workspace::new();
        let authority = WorkspaceSnapshots::default();
        let revision = authority
            .read(first.root(), &first.path())
            .unwrap()
            .revision;
        assert_eq!(
            authority
                .write(second.root(), &second.path(), &revision, b"foreign")
                .err(),
            Some(409)
        );
        assert_eq!(
            authority.read(first.root(), &second.path()).err(),
            Some(404)
        );
        assert_eq!(fs::read(second.path()).unwrap(), b"original");
    }

    #[test]
    fn escaping_symlink_and_fifo_share_the_not_found_contract() {
        let workspace = Workspace::new();
        let outside = Workspace::new();
        let authority = WorkspaceSnapshots::default();
        let link = workspace.0.join("escape");
        symlink(outside.path(), &link).unwrap();
        assert_eq!(
            authority
                .read(workspace.root(), link.to_str().unwrap())
                .err(),
            Some(404)
        );
        let fifo = workspace.0.join("fifo");
        assert!(
            std::process::Command::new("mkfifo")
                .arg(&fifo)
                .status()
                .unwrap()
                .success()
        );
        assert_eq!(
            authority
                .read(workspace.root(), fifo.to_str().unwrap())
                .err(),
            Some(404)
        );
    }

    #[test]
    fn saves_preserve_access_permissions_and_do_not_follow_old_temporary_links() {
        let workspace = Workspace::new();
        let outside = Workspace::new();
        fs::set_permissions(workspace.path(), fs::Permissions::from_mode(0o640)).unwrap();
        symlink(outside.path(), workspace.0.join(".note.txt.a3s-tmp")).unwrap();
        let authority = WorkspaceSnapshots::default();
        let revision = authority
            .read(workspace.root(), &workspace.path())
            .unwrap()
            .revision;
        authority
            .write(workspace.root(), &workspace.path(), &revision, b"new")
            .unwrap();
        assert_eq!(
            fs::metadata(workspace.path()).unwrap().permissions().mode() & 0o777,
            0o640
        );
        assert_eq!(fs::read(outside.path()).unwrap(), b"original");
    }

    #[test]
    fn required_revision_and_byte_budget_refuse_without_mutation() {
        let workspace = Workspace::new();
        let authority = WorkspaceSnapshots::default();
        assert_eq!(
            authority
                .write(workspace.root(), &workspace.path(), "", b"bad")
                .err(),
            Some(400)
        );
        let large = fs::OpenOptions::new()
            .write(true)
            .open(workspace.path())
            .unwrap();
        large.set_len(crate::workspace_file::RAW_LIMIT + 1).unwrap();
        assert_eq!(
            authority.read(workspace.root(), &workspace.path()).err(),
            Some(413)
        );
        assert_eq!(
            fs::metadata(workspace.path()).unwrap().len(),
            crate::workspace_file::RAW_LIMIT + 1
        );
    }
}
