#![cfg(test)]

#[cfg(unix)]
mod unix {
    use crate::authority::workspace_snapshot;
    use crate::workspace_snapshot::WorkspaceSnapshots;

    #[test]
    fn snapshot_route_requires_one_strong_precondition_and_returns_the_next_observation() {
        let root =
            std::env::temp_dir().join(format!("a3s-snapshot-route-{}", uuid::Uuid::now_v7()));
        std::fs::create_dir(&root).unwrap();
        let path = root.join("note.txt");
        std::fs::write(&path, b"original").unwrap();
        let query = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("root", root.to_str().unwrap())
            .append_pair("path", path.to_str().unwrap())
            .finish();
        let snapshots = WorkspaceSnapshots::default();
        let first = workspace_snapshot("GET", &query, &[], None, &snapshots).unwrap();
        assert_eq!(first.bytes, b"original");
        assert_eq!(
            workspace_snapshot("PUT", &query, b"new", None, &snapshots).err(),
            Some(428)
        );
        for invalid in [
            "*".to_owned(),
            first.revision.clone(),
            format!("W/\"{}\"", first.revision),
            format!("\"{}\", \"{}\"", first.revision, first.revision),
        ] {
            assert_eq!(
                workspace_snapshot("PUT", &query, b"new", Some(&invalid), &snapshots).err(),
                Some(400)
            );
        }
        let expected = format!("\"{}\"", first.revision);
        let next = workspace_snapshot("PUT", &query, b"new", Some(&expected), &snapshots).unwrap();
        assert!(next.bytes.is_empty());
        assert_ne!(next.revision, first.revision);
        assert_eq!(std::fs::read(&path).unwrap(), b"new");
        assert_eq!(
            workspace_snapshot("PUT", &query, b"stale", Some(&expected), &snapshots).err(),
            Some(409)
        );
        assert_eq!(
            workspace_snapshot("GET", &query, &[], None, &snapshots)
                .unwrap()
                .revision,
            next.revision
        );
        assert_eq!(std::fs::read(path).unwrap(), b"new");
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn snapshot_route_does_not_grant_an_outside_address_or_an_unsupported_method() {
        let root =
            std::env::temp_dir().join(format!("a3s-snapshot-route-{}", uuid::Uuid::now_v7()));
        std::fs::create_dir(&root).unwrap();
        let snapshots = WorkspaceSnapshots::default();
        let query = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("root", root.to_str().unwrap())
            .append_pair("path", "/etc/hosts")
            .finish();
        assert_eq!(
            workspace_snapshot("GET", &query, &[], None, &snapshots).err(),
            Some(404)
        );
        assert_eq!(
            workspace_snapshot("DELETE", &query, &[], None, &snapshots).err(),
            Some(405)
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}
