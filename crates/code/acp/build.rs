#[path = "../../../scripts/cli_pair_build.rs"]
mod cli_pair_build;
fn main() {
    cli_pair_build::stamp(&std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.."))
        .unwrap_or_else(|error| panic!("CLI pair identity: {error}"));
}
