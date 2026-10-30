//! Test-only stdio relay. Protocol bytes are forwarded unchanged; traces stay in the fixture.
use std::io::{self, BufRead, Write};
use std::process::{Command, Stdio};

fn main() -> io::Result<()> {
    let target = std::env::var_os("A3S_CODE_ACP_PROXY_TARGET")
        .ok_or_else(|| io::Error::other("A3S_CODE_ACP_PROXY_TARGET is required"))?;
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.iter().any(|arg| arg == "--build-id") {
        let status = Command::new(target).args(args).status()?;
        std::process::exit(status.code().unwrap_or(1));
    }
    let path = std::env::var_os("A3S_CODE_ACP_TRACE")
        .ok_or_else(|| io::Error::other("A3S_CODE_ACP_TRACE is required"))?;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut trace = options.open(path)?;
    let mut child = Command::new(target)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()?;
    let mut input = child.stdin.take().unwrap();
    std::thread::spawn(move || {
        let _ = io::copy(&mut io::stdin().lock(), &mut input);
    });
    let mut reader = io::BufReader::new(child.stdout.take().unwrap());
    let mut output = io::stdout().lock();
    let mut line = Vec::new();
    while reader.read_until(b'\n', &mut line)? != 0 {
        if let Ok(message) = serde_json::from_slice::<serde_json::Value>(&line) {
            serde_json::to_writer(&mut trace, &serde_json::json!({"message": message}))?;
            trace.write_all(b"\n")?;
            trace.flush()?;
        }
        output.write_all(&line)?;
        output.flush()?;
        line.clear();
    }
    let status = child.wait()?;
    std::process::exit(status.code().unwrap_or(1));
}
