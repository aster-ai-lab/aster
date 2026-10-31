#![cfg(test)]

use std::fs;
use std::net::{TcpListener, TcpStream};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::process::Command;

#[test]
fn worker_cannot_read_write_connect_or_exec_outside_its_grants() {
    let fixture = tempfile::Builder::new()
        .prefix("aster-vmm-\"中文\"-\\-")
        .tempdir_in("/private/tmp")
        .unwrap();
    let root = fixture.path().canonicalize().unwrap();
    let share = root.join("runtime");
    fs::create_dir(&share).unwrap();
    fs::write(share.join("input"), b"authorized").unwrap();
    let executable = share.join("native-executable");
    fs::copy("/usr/bin/true", &executable).unwrap();
    assert!(Command::new(&executable).status().unwrap().success());
    fs::write(root.join("immutable"), b"immutable").unwrap();
    fs::write(root.join("outside"), b"private").unwrap();
    for name in ["caller-ro", "caller-rw"] {
        fs::create_dir(root.join(name)).unwrap();
        fs::write(root.join(name).join("input"), b"caller data").unwrap();
    }
    let writable = root.join("caller-rw");
    fs::write(writable.join(".env"), b"credential").unwrap();
    fs::create_dir_all(writable.join(".aster/loops")).unwrap();
    fs::write(writable.join(".aster/config"), b"control").unwrap();
    let _allowed_socket = UnixListener::bind(root.join("agent.sock")).unwrap();
    let _denied_socket = UnixListener::bind(root.join("denied.sock")).unwrap();
    let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();

    let output = Command::new(std::env::current_exe().unwrap())
        .args([
            "--ignored",
            "--exact",
            "macos_worker_sandbox::tests::confined_worker_child",
            "--nocapture",
        ])
        .env("ASTER_OCI_TEST_VMM_SANDBOX_ROOT", &root)
        .env(
            "ASTER_OCI_TEST_VMM_SANDBOX_TCP",
            listener.local_addr().unwrap().to_string(),
        )
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(fs::read(share.join("output")).unwrap(), b"written");
    assert_eq!(fs::read(writable.join(".env")).unwrap(), b"credential");
    assert_eq!(fs::read(writable.join(".aster/config")).unwrap(), b"control");
    assert_eq!(
        fs::read(writable.join(".aster/loops/output")).unwrap(),
        b"loop state"
    );
    assert_eq!(fs::read(root.join("outside")).unwrap(), b"private");
    assert_eq!(fs::read(root.join("immutable")).unwrap(), b"immutable");
    assert_eq!(
        fs::read(root.join("caller-ro/input")).unwrap(),
        b"caller data"
    );
    assert_eq!(
        fs::read(root.join("caller-rw/output")).unwrap(),
        b"caller output"
    );
}

#[test]
#[ignore = "entered only by the parent test in a disposable subprocess"]
fn confined_worker_child() {
    let root = PathBuf::from(std::env::var_os("ASTER_OCI_TEST_VMM_SANDBOX_ROOT").unwrap());
    let share = root.join("runtime");
    let immutable = root.join("immutable");
    let console = crate::unix_process::prepare_console_output(&root.join("console"), None).unwrap();
    assert!(super::confine(
        &[&share.join("input")],
        &share,
        &console,
        None,
        &[],
        &[],
        &[]
    )
    .is_err());
    super::confine(
        &[&immutable],
        &share,
        &console,
        Some(&root.join("agent.sock")),
        &[&root.join("caller-ro")],
        &[&root.join("caller-rw")],
        &[(
            root.join("caller-rw"),
            aster_oci_sdk::StoragePathRestriction::new(
                aster_oci_sdk::StorageAttachmentId::new("workspace").unwrap(),
                vec![".env".into()],
                vec![".env".into(), ".aster".into()],
                vec![".aster/loops".into()],
            )
            .unwrap(),
        )],
    )
    .unwrap();

    assert_eq!(fs::read(share.join("input")).unwrap(), b"authorized");
    fs::write(share.join("output"), b"written").unwrap();
    assert_eq!(fs::read(&immutable).unwrap(), b"immutable");
    assert_eq!(
        fs::read(root.join("caller-ro/input")).unwrap(),
        b"caller data"
    );
    assert!(fs::write(root.join("caller-ro/input"), b"clobbered").is_err());
    assert!(fs::write(root.join("caller-ro/new"), b"clobbered").is_err());
    fs::write(root.join("caller-rw/output"), b"caller output").unwrap();
    let writable = root.join("caller-rw");
    assert!(fs::read(writable.join(".env")).is_err());
    assert!(fs::write(writable.join(".env"), b"clobbered").is_err());
    assert!(fs::rename(writable.join(".env"), writable.join("exposed")).is_err());
    assert!(fs::rename(writable.join(".aster"), writable.join("moved")).is_err());
    assert!(fs::write(writable.join(".aster/config"), b"clobbered").is_err());
    assert!(fs::write(writable.join(".aster/new"), b"clobbered").is_err());
    fs::write(writable.join(".aster/loops/output"), b"loop state").unwrap();
    fs::write(console.pinned_path(), b"console").unwrap();
    assert!(fs::read(root.join("outside")).is_err());
    assert!(fs::write(root.join("outside"), b"clobbered").is_err());
    assert!(fs::write(&immutable, b"clobbered").is_err());
    assert!(UnixStream::connect(root.join("agent.sock")).is_ok());
    assert!(UnixStream::connect(root.join("denied.sock")).is_err());
    let address = std::env::var("ASTER_OCI_TEST_VMM_SANDBOX_TCP").unwrap();
    assert!(TcpStream::connect(address).is_err());
    assert!(Command::new(share.join("native-executable"))
        .status()
        .is_err());
}
