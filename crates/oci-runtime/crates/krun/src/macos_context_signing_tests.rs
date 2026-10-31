use super::*;
use std::process::Command;

#[test]
fn resigned_runtime_keeps_pinned_code_and_rejects_modified_payloads() {
    let temporary = tempfile::tempdir().unwrap();
    for (name, size, original, code) in [
        (
            LIBKRUN_NAME,
            LIBKRUN_SIZE,
            LIBKRUN_SHA256,
            "e2664058cb5282fc99475ec33ae68404c401392b28812864abeba7c5164e93ed",
        ),
        (
            LIBKRUNFW_NAME,
            LIBKRUNFW_SIZE,
            LIBKRUNFW_SHA256,
            "a661ecaaa8310276968d029a3f518cbde32de9bd2fdea0d366d57b63958cc5d8",
        ),
    ] {
        let path = temporary.path().join(name);
        fs::copy(
            Path::new(env!("ASTER_OCI_KRUN_RUNTIME_DIR")).join(name),
            &path,
        )
        .unwrap();
        let original_file = PinnedFile::open(&path, "runtime fixture").unwrap();
        assert_eq!(original_file.sha256(), original);
        let mut bytes = fs::read(&path).unwrap();
        assert_eq!(signing_neutral_sha256(&mut bytes).as_deref(), Some(code));
        assert!(signing_neutral_sha256(&mut bytes[..31]).is_none());
        assert!(Command::new("codesign")
            .args(["--force", "--sign", "-", "--options", "runtime"])
            .arg(&path)
            .status()
            .unwrap()
            .success());
        let signed = PinnedFile::open(&path, "runtime fixture").unwrap();
        assert_ne!(signed.sha256(), original);
        verify_pinned_runtime_file(&signed, size, original, code).unwrap();

        let mut bytes = fs::read(&path).unwrap();
        bytes[8192] ^= 1;
        fs::write(&path, bytes).unwrap();
        let tampered = PinnedFile::open(&path, "runtime fixture").unwrap();
        assert!(verify_pinned_runtime_file(&tampered, size, original, code).is_err());
    }
}
