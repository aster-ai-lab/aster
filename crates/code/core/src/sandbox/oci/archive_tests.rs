#![cfg(test)]

use super::*;

fn entry(name: &str, body: &[u8], kind: u8, link: &str) -> Vec<u8> {
    let mut header = [0u8; 512];
    header[..name.len()].copy_from_slice(name.as_bytes());
    header[100..108].copy_from_slice(b"0000755\0");
    header[124..136].copy_from_slice(format!("{:011o}\0", body.len()).as_bytes());
    header[148..156].fill(b' ');
    header[156] = kind;
    header[157..157 + link.len()].copy_from_slice(link.as_bytes());
    header[257..263].copy_from_slice(b"ustar\0");
    let sum: u32 = header.iter().map(|value| u32::from(*value)).sum();
    header[148..156].copy_from_slice(format!("{sum:06o}\0 ").as_bytes());
    let mut bytes = header.to_vec();
    bytes.extend(body);
    bytes.resize(bytes.len().div_ceil(512) * 512, 0);
    bytes
}

#[tokio::test]
async fn packaged_guest_extracts_bash_and_retains_guest_symlinks() {
    let directory = tempfile::tempdir().unwrap();
    let mut bytes = entry("rootfs/bin/bash", b"guest-bash", b'0', "");
    bytes.extend(entry("rootfs/bin/sh", b"", b'2', "/bin/bash"));
    bytes.resize(bytes.len() + 1024, 0);
    std::fs::write(directory.path().join("rootfs.tar"), bytes).unwrap();
    bundle::unpack_rootfs(directory.path(), &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(
        std::fs::read(directory.path().join("rootfs/bin/bash")).unwrap(),
        b"guest-bash"
    );
    assert_eq!(
        std::fs::read_link(directory.path().join("rootfs/bin/sh")).unwrap(),
        Path::new("/bin/bash")
    );
    assert!(!directory.path().join("rootfs.tar").exists());
}

#[tokio::test]
async fn packaged_guest_rejects_parent_traversal_and_symlink_writes() {
    for symlink in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().join("staged");
        std::fs::create_dir(&directory).unwrap();
        let outside = root.path().join("outside");
        let mut bytes = if symlink {
            let mut data = entry("rootfs/bin", b"", b'2', outside.to_str().unwrap());
            data.extend(entry("rootfs/bin/bash", b"escape", b'0', ""));
            data
        } else {
            entry("../outside", b"escape", b'0', "")
        };
        bytes.resize(bytes.len() + 1024, 0);
        std::fs::write(directory.join("rootfs.tar"), bytes).unwrap();
        assert!(bundle::unpack_rootfs(&directory, &CancellationToken::new())
            .await
            .is_err());
        assert!(!outside.exists());
    }
}
