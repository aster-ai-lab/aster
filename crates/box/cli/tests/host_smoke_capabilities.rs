#![cfg(test)]

use super::*;
use aster_box_core::pty::{parse_frame, PtyFrame, PtyRequest};

const NONROOT_PROBE: &str = r#"
set -eu
test "$(id -u)" = 10001
test "$(id -g)" = 10001
for field in CapInh CapPrm CapEff CapBnd CapAmb; do
  actual=$(awk -v wanted="$field:" '$1 == wanted { print $2 }' /proc/self/status)
  test "$actual" = 0000000000000000
done
test "$(awk '$1 == "NoNewPrivs:" { print $2 }' /proc/self/status)" = 1
echo nonroot-capabilities-ok
"#;

#[test]
#[ignore = "requires HVF/KVM and real guest assets"]
fn nonroot_drop_all_applies_to_main_exec_and_pty() {
    let cli = CliTest::new();
    let image = host_smoke_image();
    seed_runnable_alpine_image(&cli, &image);
    let boxes = ["nonroot-cap-main", "nonroot-cap-stream"];
    let mut resources = HostResourceCleanup {
        cli: &cli,
        boxes: &boxes,
        networks: &[],
        armed: true,
    };
    let output = cli.ok(&[
        "run",
        "--network",
        "none",
        "--cap-drop",
        "ALL",
        "--security-opt",
        "no-new-privileges",
        "--user",
        "10001:10001",
        "--name",
        boxes[0],
        &image,
        "--",
        "sh",
        "-c",
        NONROOT_PROBE,
    ]);
    assert!(output.contains("nonroot-capabilities-ok"));
    cli.ok(&["rm", boxes[0]]);
    cli.ok(&[
        "run",
        "--detach",
        "--network",
        "none",
        "--cap-drop",
        "ALL",
        "--security-opt",
        "no-new-privileges",
        "--user",
        "10001:10001",
        "--name",
        boxes[1],
        &image,
        "--",
        "sh",
        "-c",
        "trap 'exit 0' TERM; while :; do sleep 1; done",
    ]);
    let output = cli.ok(&[
        "exec",
        "--user",
        "10001:10001",
        "--env",
        "ASTER_SEC_CAP_DROP=ALL",
        "--env",
        "ASTER_SEC_NO_NEW_PRIVS=1",
        boxes[1],
        "--",
        "sh",
        "-c",
        NONROOT_PROBE,
    ]);
    assert!(output.contains("nonroot-capabilities-ok"));

    let record = inspect_box(&cli, boxes[1]);
    let socket = Path::new(record["exec_socket_path"].as_str().unwrap()).with_file_name("pty.sock");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let output = runtime.block_on(async {
        tokio::time::timeout(Duration::from_secs(10), async {
            let mut client = aster_box_runtime::grpc::PtyClient::connect(&socket)
                .await
                .unwrap();
            client
                .send_request(&PtyRequest {
                    cmd: vec!["sh".into(), "-c".into(), NONROOT_PROBE.into()],
                    env: vec![
                        "ASTER_SEC_CAP_DROP=ALL".into(),
                        "ASTER_SEC_NO_NEW_PRIVS=1".into(),
                    ],
                    working_dir: None,
                    rootfs: None,
                    user: Some("10001:10001".into()),
                    cols: 80,
                    rows: 24,
                })
                .await
                .unwrap();
            let mut output = Vec::new();
            loop {
                let (kind, payload) = client.read_frame().await.unwrap().expect("PTY exit frame");
                match parse_frame(kind, payload).unwrap() {
                    PtyFrame::Data(data) => {
                        output.extend(data);
                        assert!(output.len() <= 64 * 1024);
                    }
                    PtyFrame::Exit(exit) => {
                        assert_eq!(exit.exit_code, 0, "{}", String::from_utf8_lossy(&output));
                        break output;
                    }
                    PtyFrame::Error(error) => panic!("PTY refused non-root profile: {error}"),
                    _ => {}
                }
            }
        })
        .await
        .expect("bounded PTY completion")
    });
    assert!(String::from_utf8_lossy(&output).contains("nonroot-capabilities-ok"));
    cli.ok(&["stop", "--timeout", "2", boxes[1]]);
    cli.ok(&["rm", boxes[1]]);
    resources.disarm();
}
