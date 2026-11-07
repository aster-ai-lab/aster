use super::{asset_name_for, channel_for, parse_manifest, Version};

#[test]
fn download_progress_reports_bytes_and_never_invents_an_unknown_percentage() {
    use std::time::Duration;
    let mib = 1024 * 1024;
    let known = super::render_download(84 * mib, Some(200 * mib), Duration::from_secs(10), 119);
    assert!(known.starts_with("Downloading ["), "{known}");
    assert!(known.contains("42%  84.0/200.0 MiB  8.4 MiB/s"), "{known}");
    let unknown = super::render_download(84 * mib, None, Duration::from_secs(10), 119);
    assert_eq!(unknown, "Downloading 84.0 MiB  8.4 MiB/s");
    for total in [None, Some(0), Some(200 * mib)] {
        for width in [0, 1, 10, 19, 39, 79, 119] {
            let frame = super::render_download(84 * mib, total, Duration::ZERO, width);
            assert!(frame.len() <= width, "{width}: {frame}");
            assert!(!frame.contains(['\n', '\r', '\u{1b}']), "{frame}");
        }
    }
}

#[test]
fn channel_manifest_reads_are_bounded_before_allocating_the_whole_input() {
    use std::io::{self, Read};
    let limit = super::MAX_MANIFEST as usize;
    assert_eq!(
        super::read_manifest_body(vec![b' '; limit].as_slice())
            .unwrap()
            .len(),
        limit
    );
    let mut source = io::Cursor::new(vec![b' '; limit + 4096]);
    assert!(super::read_manifest_body(&mut source)
        .unwrap_err()
        .to_string()
        .contains("too large"));
    assert_eq!(source.position(), super::MAX_MANIFEST + 1);
    struct Broken;
    impl Read for Broken {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::other("synthetic read failure"))
        }
    }
    assert!(
        format!("{:#}", super::read_manifest_body(Broken).unwrap_err())
            .contains("synthetic read failure")
    );
    let root = super::WorkDir::new().unwrap();
    std::fs::write(root.path.join(super::MANIFEST_NAME), vec![b' '; limit + 1]).unwrap();
    assert!(super::read_local_manifest(&root.path)
        .err()
        .unwrap()
        .to_string()
        .contains("too large"));
}

#[test]
fn standard_platforms_use_one_canonical_archive_each() {
    for (os, extension) in [("macos", "tar.gz"), ("linux", "tar.gz"), ("windows", "zip")] {
        assert_eq!(channel_for(os), os);
        assert_eq!(
            asset_name_for(Version(0, 1, 3), channel_for(os)),
            format!("aster-cli-v0.1.3-{os}.{extension}")
        );
    }
    assert_eq!(channel_for("freebsd"), "unsupported");
}

#[test]
fn updater_accepts_the_three_platform_manifest_and_refuses_a_retired_channel() {
    let mut manifest = serde_json::json!({
        "version": "0.1.3",
        "tag": "aster-cli-v0.1.3",
        "assets": {}
    });
    for (os, extension) in [("macos", "tar.gz"), ("linux", "tar.gz"), ("windows", "zip")] {
        manifest
            .get_mut("assets")
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert(
                os.into(),
                serde_json::json!({
                    "name": format!("aster-cli-v0.1.3-{os}.{extension}"),
                    "sha256": "a".repeat(64)
                }),
            );
    }
    let parsed = parse_manifest(&serde_json::to_vec(&manifest).unwrap(), "").unwrap();
    assert_eq!(parsed.assets.len(), 3);
    manifest
        .get_mut("assets")
        .unwrap()
        .as_object_mut()
        .unwrap()
        .insert(
            "kylin".into(),
            serde_json::json!({
                "name": "aster-cli-v0.1.3-kylin.tar.gz",
                "sha256": "a".repeat(64)
            }),
        );
    let error = parse_manifest(&serde_json::to_vec(&manifest).unwrap(), "")
        .err()
        .unwrap();
    assert!(error.to_string().contains("unsupported platform kylin"));
}

#[test]
fn release_members_reject_escape_paths_and_sandbox_companions() {
    use std::path::Path;
    for name in [
        "../aster",
        "/aster",
        "sandbox/../aster",
        "sandbox/extra",
        "sandbox/aster-oci",
        "sandbox",
        "aster/nested",
    ] {
        assert!(super::archive_member(Path::new(name)).is_none(), "{name}");
        assert!(!super::allowed_member(name), "{name}");
    }
    let (aster_name, _) = super::binary_names();
    assert_eq!(
        super::archive_member(Path::new(&format!("./{aster_name}"))).as_deref(),
        Some(aster_name)
    );
}

#[cfg(unix)]
#[test]
fn matching_bytes_with_missing_execution_permission_need_reinstallation() {
    use std::os::unix::fs::PermissionsExt;
    let work = super::WorkDir::new().unwrap();
    let source = work.path.join("source");
    let installed = work.path.join("installed");
    std::fs::write(&source, "same bytes").unwrap();
    std::fs::write(&installed, "same bytes").unwrap();
    std::fs::set_permissions(&source, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::fs::set_permissions(&installed, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert!(!super::files_identical(&source, &installed));
    super::make_executable(&installed).unwrap();
    assert!(super::files_identical(&source, &installed));
}

#[test]
fn rollback_restores_the_pair_before_or_after_second_binary_promotion() {
    use std::fs;
    for acp_promoted in [false, true] {
        let work = super::WorkDir::new().unwrap();
        let stage_dir = work.path.join("stage");
        let lock = work.path.join("lock");
        fs::create_dir(&stage_dir).unwrap();
        fs::create_dir(&lock).unwrap();
        let dest_aster = work.path.join("aster");
        let dest_acp = work.path.join("aster-acp");
        fs::write(&dest_aster, "new aster").unwrap();
        if acp_promoted {
            fs::write(&dest_acp, "new acp").unwrap();
        } else {
            fs::write(&dest_acp, "previous acp").unwrap();
        }
        fs::write(stage_dir.join("old-aster"), "previous aster").unwrap();
        fs::write(stage_dir.join("old-acp"), "previous acp").unwrap();
        drop(super::Stage {
            dir: stage_dir.clone(),
            lock: lock.clone(),
            dest_aster: dest_aster.clone(),
            dest_acp: dest_acp.clone(),
            had_aster: true,
            had_acp: true,
            aster_promoted: true,
            acp_promoted,
            committed: false,
        });
        assert_eq!(fs::read_to_string(dest_aster).unwrap(), "previous aster");
        assert_eq!(fs::read_to_string(dest_acp).unwrap(), "previous acp");
        assert!(!stage_dir.exists());
        assert!(!lock.exists());
    }
}

#[test]
fn a_failed_pair_restore_retains_the_backup_and_install_lock() {
    use std::fs;
    let work = super::WorkDir::new().unwrap();
    let stage_dir = work.path.join("stage");
    let lock = work.path.join("lock");
    let dest_aster = work.path.join("aster");
    fs::create_dir(&stage_dir).unwrap();
    fs::create_dir(&lock).unwrap();
    // Leave old-aster missing so restore_one fails after promotion.
    fs::write(&dest_aster, "new aster").unwrap();
    drop(super::Stage {
        dir: stage_dir.clone(),
        lock: lock.clone(),
        dest_aster: dest_aster.clone(),
        dest_acp: work.path.join("aster-acp"),
        had_aster: true,
        had_acp: false,
        aster_promoted: true,
        acp_promoted: false,
        committed: false,
    });
    // replace_file may already have removed the promoted dest while the
    // missing backup rename failed; the stage and lock must still be kept.
    assert!(stage_dir.is_dir());
    assert!(lock.is_dir());
}

#[test]
fn unpacking_rejects_duplicate_and_linked_release_members() {
    use std::{fs::File, io::Cursor};
    for linked in [false, true] {
        let work = super::WorkDir::new().unwrap();
        let archive = work.path.join("release.tar.gz");
        let encoder = flate2::write::GzEncoder::new(
            File::create(&archive).unwrap(),
            flate2::Compression::default(),
        );
        let mut tar = tar::Builder::new(encoder);
        for index in 0..2 {
            let mut header = tar::Header::new_gnu();
            header.set_mode(0o755);
            if linked && index == 1 {
                header.set_entry_type(tar::EntryType::Symlink);
                header.set_size(0);
                header.set_link_name("../foreign").unwrap();
            } else {
                header.set_size(0);
            }
            header.set_cksum();
            tar.append_data(&mut header, super::binary_names().0, Cursor::new([]))
                .unwrap();
        }
        tar.into_inner().unwrap().finish().unwrap();
        let output = work.path.join("unpack");
        std::fs::create_dir(&output).unwrap();
        let error = super::unpack_tar(&archive, &output)
            .unwrap_err()
            .to_string();
        assert!(
            error.contains(if linked {
                "not a release file"
            } else {
                "duplicate"
            }),
            "{error}"
        );
    }
}

#[test]
fn unpacking_rejects_sandbox_archive_members() {
    use std::{fs::File, io::Cursor};
    let work = super::WorkDir::new().unwrap();
    let archive = work.path.join("release.tar.gz");
    let encoder = flate2::write::GzEncoder::new(
        File::create(&archive).unwrap(),
        flate2::Compression::default(),
    );
    let mut tar = tar::Builder::new(encoder);
    for (name, body) in [
        (super::binary_names().0, b"aster".as_slice()),
        (super::binary_names().1, b"acp".as_slice()),
        ("sandbox/aster-oci", b"unexpected".as_slice()),
    ] {
        let mut header = tar::Header::new_gnu();
        header.set_mode(0o755);
        header.set_size(body.len() as u64);
        header.set_cksum();
        tar.append_data(&mut header, name, Cursor::new(body))
            .unwrap();
    }
    tar.into_inner().unwrap().finish().unwrap();
    let output = work.path.join("unpack");
    std::fs::create_dir(&output).unwrap();
    let error = super::unpack_tar(&archive, &output)
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("not a release file") || error.contains("unexpected sandbox"),
        "{error}"
    );
}
