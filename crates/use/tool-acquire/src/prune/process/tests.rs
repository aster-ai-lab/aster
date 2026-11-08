use super::*;

#[cfg(target_os = "macos")]
#[test]
fn macos_argc_bounds_exclude_environment_fields() {
    let mut bytes = 3i32.to_ne_bytes().to_vec();
    bytes.extend_from_slice(b"/bin/sh\0\0/bin/sh\0/tmp/tool with spaces/\xe8\x84\x9a\xe6\x9c\xac\0flag\0/tmp/not-an-argument\0SECRET=synthetic-secret\0");
    assert_eq!(
        macos_paths(&bytes).unwrap(),
        vec![
            PathBuf::from("/bin/sh"),
            PathBuf::from("/tmp/tool with spaces/脚本")
        ]
    );
    assert!(macos_paths(&bytes[..3]).is_err());
    assert!(macos_paths(&(-1i32).to_ne_bytes()).is_err());
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
#[test]
fn unix_argument_paths_preserve_non_utf8_units_without_interpreting_flags() {
    use std::os::unix::ffi::OsStrExt;
    let fields = [
        b"/tmp/native-\xff".as_slice(),
        b"--file=/tmp/other",
        b"relative/file",
    ];
    let paths = unix_paths(fields.into_iter());
    assert_eq!(paths.len(), 1);
    assert_eq!(paths[0].as_os_str().as_bytes(), b"/tmp/native-\xff");
}
