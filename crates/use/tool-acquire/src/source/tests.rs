use super::*;

#[test]
fn executable_and_companion_bytes_stay_out_of_debug_including_containing_sources() {
    let secret = "synthetic-private-payload-secret";
    let payload = ToolPayload {
        version: "1.0.0".into(),
        sha256: "0".repeat(64),
        bytes: secret.as_bytes().to_vec(),
        companions: vec![CompanionFile {
            relative_path: "lib/private.dat".into(),
            bytes: secret.as_bytes().to_vec(),
            executable: false,
        }],
    };
    let byte_debug = format!("{:?}", secret.as_bytes());
    let install = InstallPayload::Executable(payload.clone());
    let mut source = StaticSource::new();
    source.insert("github:fixture/tool", payload.clone());
    for debug in [
        format!("{payload:?}"),
        format!("{install:?}"),
        format!("{source:?}"),
    ] {
        assert!(!debug.contains(secret));
        assert!(!debug.contains(&byte_debug));
        assert!(debug.contains("private.dat"));
    }
}
