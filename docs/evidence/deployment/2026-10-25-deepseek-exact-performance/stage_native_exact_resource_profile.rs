use a3s_acl::Value;
use std::os::unix::fs::{MetadataExt, PermissionsExt, chown};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, ExitCode},
};
fn run() -> Result<(), String> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 5 && args.len() != 6 {
        return Err("expected current Power/bridge and typed threads/arena/PCIe settings and optional bf16/fp4 mode".into());
    }
    for path in &args[..2] {
        let path = Path::new(path);
        if !path.is_absolute() || !path.starts_with("/opt/a3s/releases") || !path.is_file() {
            return Err("candidate must be a release artifact".into());
        }
    }
    let packed = match args.get(5).map(String::as_str) {
        None | Some("fp4") => true,
        Some("bf16") => false,
        _ => return Err("cache mode must be bf16 or fp4".into()),
    };
    let numbers = args[2..5]
        .iter()
        .map(|v| v.parse::<u32>().map_err(|_| "numeric tuning required"))
        .collect::<Result<Vec<_>, _>>()?;
    let [threads, arena, pcie]: [u32; 3] = numbers
        .try_into()
        .map_err(|_| "three tuning values required")?;
    if ![8, 16, 24, 32].contains(&threads)
        || ![60, 96, 120, 144].contains(&arena)
        || ![0, 2500, 5000, 7500, 10000].contains(&pcie)
    {
        return Err("unqualified resource cohort".into());
    }
    let config = Path::new("/etc/a3s/power/deepseek-gpu6.acl");
    let original = fs::read_to_string(config).map_err(|_| "native ACL unavailable")?;
    let metadata = fs::metadata(config).map_err(|_| "native ACL metadata unavailable")?;
    let mut document = a3s_acl::parse(&original).map_err(|_| "native ACL parse failed")?;
    if document
        .blocks
        .iter()
        .filter(|b| b.name == "xolotl")
        .count()
        != 1
    {
        return Err("native ACL must declare one Xolotl block".into());
    }
    for (key, expected) in [
        ("max_concurrent_requests", 1.0),
        ("max_queued_requests", 8.0),
        ("queue_timeout_ms", 120000.0),
    ] {
        let entries = document
            .blocks
            .iter()
            .filter(|block| block.name == key)
            .collect::<Vec<_>>();
        if entries.len() != 1 || entries[0].attributes.get(key) != Some(&Value::Number(expected)) {
            return Err("native admission policy changed concurrently".into());
        }
    }
    let xolotl = document
        .blocks
        .iter_mut()
        .find(|b| b.name == "xolotl")
        .ok_or("Xolotl missing")?;
    if !matches!(xolotl.attributes.get("kv_fp4"), Some(Value::Bool(_))) {
        return Err("KV packing changed concurrently".into());
    }
    if xolotl.attributes.get("gpus") != Some(&Value::List(vec![Value::Number(6.0)])) {
        return Err("native GPU placement is no longer exactly GPU 6".into());
    }
    let Some(Value::Object(tuning)) = xolotl.attributes.get_mut("tuning") else {
        return Err("typed tuning missing".into());
    };
    if !tuning
        .iter()
        .any(|(k, v)| k == "context_tokens" && v == &Value::Number(1048576.0))
    {
        return Err("native context is no longer 1048576".into());
    }
    for (key, value) in [
        ("cpu_threads", threads),
        ("arena_gib", arena),
        ("pcie_bp", pcie),
    ] {
        let entry = tuning
            .iter_mut()
            .find(|(k, _)| k == key)
            .ok_or("required tuning key missing")?;
        entry.1 = Value::Number(value as f64);
    }
    let Some(Value::Object(managed)) = xolotl.attributes.get("managed") else {
        return Err("managed binding missing".into());
    };
    if managed.iter().find(|(k, _)| k == "bridge").map(|(_, v)| v)
        != Some(&Value::String(args[1].clone()))
    {
        return Err("exact routing bridge changed".into());
    }
    xolotl
        .attributes
        .insert("kv_fp4".into(), Value::Bool(packed));
    let staged = PathBuf::from(format!(
        "/etc/a3s/power/.deepseek-gpu6-exact-resource-{}.acl",
        std::process::id()
    ));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&staged)
        .map_err(|_| "staging unavailable")?;
    file.set_permissions(fs::Permissions::from_mode(metadata.permissions().mode()))
        .map_err(|_| "permission preservation failed")?;
    chown(&staged, Some(metadata.uid()), Some(metadata.gid()))
        .map_err(|_| "ownership preservation failed")?;
    file.write_all(a3s_acl::generate(&document).as_bytes())
        .map_err(|_| "staging write failed")?;
    file.sync_all().map_err(|_| "staging sync failed")?;
    let checked = Command::new(&args[0])
        .args(["serve", "--config"])
        .arg(&staged)
        .arg("--check-config")
        .output();
    if !checked
        .map_err(|_| "candidate validator unavailable")?
        .status
        .success()
    {
        let _ = fs::remove_file(&staged);
        return Err("candidate Power refused its native ACL".into());
    }
    if fs::read_to_string(config).map_err(|_| "base reread failed")? != original {
        let _ = fs::remove_file(&staged);
        return Err("native ACL changed concurrently; candidate not activated".into());
    }
    println!("{}", staged.display());
    Ok(())
}
fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}
