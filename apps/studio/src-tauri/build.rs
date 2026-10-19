use std::path::PathBuf;

fn main() {
    let output = PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR"));
    let profile = output.ancestors().nth(3).expect("Cargo profile directory");
    let resources =
        PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"))
            .join("resources/agent");
    std::fs::create_dir_all(&resources).expect("create companion resource directory");
    for name in ["a3s-code-acp", "a3s-office"] {
        let name = if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
            format!("{name}.exe")
        } else {
            name.to_owned()
        };
        let built = profile.join(&name);
        println!("cargo:rerun-if-changed={}", built.display());
        if built.is_file() {
            std::fs::copy(&built, resources.join(name)).expect("copy companion executable");
        } else if std::env::var("PROFILE").as_deref() == Ok("release") {
            panic!(
                "missing {}; build ACP and Office first with just build-studio-release",
                built.display()
            );
        }
    }
    tauri_build::build();
}
