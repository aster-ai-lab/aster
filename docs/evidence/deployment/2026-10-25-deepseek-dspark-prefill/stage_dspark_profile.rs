use a3s_acl::Value;
use std::os::unix::fs::{MetadataExt, PermissionsExt, chown};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, ExitCode},
};

fn run() -> Result<(), String> {
    let a: Vec<String> = std::env::args().skip(1).collect();
    if a.len() != 13 {
        return Err("expected Power, bridge, engine, sidecar, digest, on/off, depth, confidence, eviction, batch PCIe, chunk, batch, indexer tile or upstream".into());
    }
    for path in &a[..4] {
        if !Path::new(path).is_absolute() || !Path::new(path).is_file() {
            return Err("runtime artifact must be an absolute existing file".into());
        }
    }
    let on = match a[5].as_str() {
        "on" => true,
        "off" => false,
        _ => return Err("draft mode must be on or off".into()),
    };
    let n = a[6..12]
        .iter()
        .map(|s| {
            s.parse::<u32>()
                .map_err(|_| "canonical integer required".to_owned())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let tile = if a[12] == "upstream" {
        None
    } else {
        let value = a[12]
            .parse::<u64>()
            .map_err(|_| "indexer tile must be an integer or upstream")?;
        if !((1_u64 << 24)..=(1_u64 << 28)).contains(&value) || !value.is_power_of_two() {
            return Err("indexer tile exceeds admitted bounds".into());
        }
        Some(value)
    };
    if !(1..=5).contains(&n[0])
        || n[1] > 10000
        || n[2] > 1048576
        || n[2] % 128 != 0
        || ![0, 2500, 5000, 7500, 10000].contains(&n[3])
        || ![256, 512, 1024].contains(&n[4])
        || ![4096, 8192].contains(&n[5])
    {
        return Err("unqualified experiment bounds".into());
    }
    if a[4].len() != 64
        || !a[4]
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err("draft digest must be lowercase SHA-256".into());
    }
    let config = Path::new("/etc/a3s/power/deepseek-gpu6.acl");
    let original = fs::read_to_string(config).map_err(|_| "native ACL unavailable")?;
    let meta = fs::metadata(config).map_err(|_| "native ACL metadata unavailable")?;
    let mut document = a3s_acl::parse(&original).map_err(|_| "native ACL parse failed")?;
    for (key, expected) in [
        ("max_concurrent_requests", 1.0),
        ("max_queued_requests", 8.0),
        ("queue_timeout_ms", 120000.0),
    ] {
        let blocks = document
            .blocks
            .iter()
            .filter(|b| b.name == key)
            .collect::<Vec<_>>();
        if blocks.len() != 1 || blocks[0].attributes.get(key) != Some(&Value::Number(expected)) {
            return Err("admission policy changed".into());
        }
    }
    if document
        .blocks
        .iter()
        .filter(|b| b.name == "xolotl")
        .count()
        != 1
    {
        return Err("one Xolotl block required".into());
    }
    let x = document
        .blocks
        .iter_mut()
        .find(|b| b.name == "xolotl")
        .ok_or("Xolotl missing")?;
    if x.attributes.get("gpus") != Some(&Value::List(vec![Value::Number(6.0)]))
        || x.attributes.get("kv_fp4") != Some(&Value::Bool(true))
    {
        return Err("GPU6 packed-KV profile changed".into());
    }
    let Some(Value::Object(t)) = x.attributes.get_mut("tuning") else {
        return Err("typed tuning missing".into());
    };
    for (key, expected) in [
        ("context_tokens", 1048576.0),
        ("cpu_threads", 24.0),
        ("arena_gib", 144.0),
        ("pcie_bp", 0.0),
    ] {
        if t.iter().find(|(k, _)| k == key).map(|(_, v)| v) != Some(&Value::Number(expected)) {
            return Err("baseline context or decode resources changed".into());
        }
    }
    for (key, v) in [
        ("tier_batch_pcie_bp", n[3]),
        ("prefill_chunk", n[4]),
        ("prompt_batch", n[5]),
    ] {
        t.iter_mut()
            .find(|(k, _)| k == key)
            .ok_or("required tuning missing")?
            .1 = Value::Number(v as f64);
    }
    let Some(Value::Object(m)) = x.attributes.get_mut("managed") else {
        return Err("managed binding missing".into());
    };
    m.iter_mut()
        .find(|(k, _)| k == "bridge")
        .ok_or("managed bridge missing")?
        .1 = Value::String(a[1].clone());
    x.attributes
        .insert("bin".into(), Value::String(a[2].clone()));
    if let Some(tile) = tile {
        x.attributes
            .insert("indexer_tile_elements".into(), Value::Number(tile as f64));
    } else {
        x.attributes.remove("indexer_tile_elements");
    }
    if on {
        x.attributes.insert(
            "dspark".into(),
            Value::Object(vec![
                ("sidecar".into(), Value::String(a[3].clone())),
                ("sidecar_sha256".into(), Value::String(a[4].clone())),
                ("depth".into(), Value::Number(n[0] as f64)),
                ("confidence_bp".into(), Value::Number(n[1] as f64)),
                ("evict_prompt_tokens".into(), Value::Number(n[2] as f64)),
            ]),
        );
    } else {
        x.attributes.remove("dspark");
    }
    let stage = PathBuf::from(format!(
        "/etc/a3s/power/.deepseek-gpu6-exact-resource-{}.acl",
        std::process::id()
    ));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&stage)
        .map_err(|_| "staging unavailable")?;
    file.set_permissions(fs::Permissions::from_mode(meta.permissions().mode()))
        .map_err(|_| "permission preservation failed")?;
    chown(&stage, Some(meta.uid()), Some(meta.gid()))
        .map_err(|_| "ownership preservation failed")?;
    file.write_all(a3s_acl::generate(&document).as_bytes())
        .map_err(|_| "staging write failed")?;
    file.sync_all().map_err(|_| "staging sync failed")?;
    let check = Command::new(&a[0])
        .args(["serve", "--config"])
        .arg(&stage)
        .arg("--check-config")
        .output()
        .map_err(|_| "candidate validator unavailable")?;
    if !check.status.success() {
        let _ = fs::remove_file(&stage);
        return Err("candidate refused its staged ACL".into());
    }
    if fs::read_to_string(config).map_err(|_| "base reread failed")? != original {
        let _ = fs::remove_file(&stage);
        return Err("concurrent ACL change".into());
    }
    println!("{}", stage.display());
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
