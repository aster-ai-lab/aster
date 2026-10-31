//! Prepare a digest-pinned image for ACP guest qualification without host state.

use aster_box_runtime::oci::{
    ImagePuller, ImageReference, ImageStore, OciRootfsBuilder, RegistryAuth, RegistryPullPolicy,
};
use oci_spec::image::Sha256Digest;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

const IMAGE: &str = "docker.io/library/rust@sha256:b28e5606d830400fabf789f910f9ed2ea22cdd6d51d463c5d0baa30bb2bedb2d";

fn pinned_image(image: &str) -> Result<(), Box<dyn std::error::Error>> {
    let reference = ImageReference::parse(image)?;
    let digest = reference
        .digest
        .as_deref()
        .ok_or("an image digest is required")?;
    let hex = digest
        .strip_prefix("sha256:")
        .ok_or("SHA-256 is required")?;
    let _: Sha256Digest = hex.parse()?;
    Ok(())
}

fn new_output(path: &Path) -> std::io::Result<()> {
    if !path.is_absolute() {
        return Err(std::io::Error::other(
            "explicit absolute output directory required",
        ));
    }
    std::fs::create_dir(path)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let root = PathBuf::from(
        args.next()
            .ok_or("usage: prepare_acp_rootfs NEW_ABSOLUTE_DIRECTORY [IMAGE@sha256:DIGEST]")?,
    );
    let image = args
        .next()
        .map(|value| value.into_string())
        .transpose()
        .map_err(|_| "image reference must be UTF-8")?
        .unwrap_or_else(|| IMAGE.into());
    if args.next().is_some() {
        return Err("only an output directory and optional pinned image are accepted".into());
    }
    pinned_image(&image)?;
    if std::env::var_os("ASTER_REGISTRY_MIRRORS").is_some()
        || std::env::var_os("ASTER_REGISTRY_PROTOCOL").is_some()
    {
        return Err("this qualification requires the unoverridden HTTPS registry".into());
    }
    new_output(&root)?;
    let store = Arc::new(ImageStore::new(
        &root.join("image-store"),
        1024 * 1024 * 1024,
    )?);
    let policy = RegistryPullPolicy::try_new(
        2,
        Duration::from_millis(250),
        Duration::from_secs(1),
        Duration::from_secs(30),
        2,
    )
    .map_err(std::io::Error::other)?;
    let puller =
        ImagePuller::with_platform(store, RegistryAuth::anonymous(), Some("linux/arm64".into()))
            .with_pull_policy(policy);
    let pulled = tokio::time::timeout(Duration::from_secs(900), puller.pull(&image)).await??;
    let rootfs = root.join("rootfs");
    OciRootfsBuilder::new(&rootfs)
        .with_image(pulled.root_dir())
        .build()?;
    println!(
        "{}",
        serde_json::json!({"source":image,"manifest_digest":pulled.manifest_digest(),"rootfs":rootfs,"credentials_imported":false})
    );
    Ok(())
}

#[cfg(test)]
#[path = "prepare_acp_rootfs/tests.rs"]
mod tests;
