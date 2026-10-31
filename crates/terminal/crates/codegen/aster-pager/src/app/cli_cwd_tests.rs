#![cfg(test)]

use super::*;

#[test]
fn cwd_application_retains_absolute_paths_and_fails_closed() {
    const CHILD: &str = "ASTER_CLI_CWD_TEST_CHILD";
    if std::env::var_os(CHILD).is_none() {
        // Cwd is process-wide; exercise the real implementation in a child so
        // other parallel tests never observe this test's directory changes.
        let launch = tempfile::tempdir().unwrap();
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "app::cli::cwd_tests::cwd_application_retains_absolute_paths_and_fails_closed",
                "--nocapture",
            ])
            .env(CHILD, "1")
            .current_dir(launch.path())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
        return;
    }

    let launch = dunce::canonicalize(std::env::current_dir().unwrap()).unwrap();
    let workspace = launch.join("workspace 中文 with spaces");
    std::fs::create_dir(&workspace).unwrap();
    let args = PagerArgs::try_parse_from([
        "aster",
        "--cwd",
        "./workspace 中文 with spaces",
        "--leader-socket",
        "relative.sock",
        "--debug-file",
        "relative.log",
    ])
    .unwrap()
    .apply_cwd()
    .unwrap();
    assert_eq!(args.cwd.as_deref(), Some(workspace.as_path()));
    assert_eq!(
        dunce::canonicalize(std::env::current_dir().unwrap()).unwrap(),
        workspace
    );
    assert_eq!(
        dunce::canonicalize(args.cwd.as_ref().unwrap()).unwrap(),
        workspace
    );
    assert_eq!(args.leader_socket, Some(launch.join("relative.sock")));
    assert_eq!(args.debug_file, Some(launch.join("relative.log")));
    assert_eq!(
        args.apply_cwd().unwrap().cwd.as_deref(),
        Some(workspace.as_path())
    );

    for invalid in ["missing directory", "not a directory"] {
        if invalid == "not a directory" {
            std::fs::write(workspace.join(invalid), b"file").unwrap();
        }
        let result = PagerArgs::try_parse_from(["aster", "--cwd", invalid])
            .unwrap()
            .apply_cwd();
        assert!(result.is_err());
        assert_eq!(
            dunce::canonicalize(std::env::current_dir().unwrap()).unwrap(),
            workspace
        );
    }
    let args = PagerArgs::try_parse_from(["aster"])
        .unwrap()
        .apply_cwd()
        .unwrap();
    assert!(args.cwd.is_none());
    assert_eq!(
        dunce::canonicalize(std::env::current_dir().unwrap()).unwrap(),
        workspace
    );
}
