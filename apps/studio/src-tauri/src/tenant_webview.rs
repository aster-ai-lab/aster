//! Keep each new product/profile's browser storage within its ownership boundary.

use std::io;

use sha2::{Digest, Sha256};
use tauri::{Manager, Runtime, WebviewWindowBuilder};

pub(super) fn isolated_store_id(
    storage: &str,
    instance: Option<&str>,
    legacy: [u8; 16],
) -> Option<[u8; 16]> {
    let aster = storage == "aster"
        || instance.is_some_and(|name| storage.strip_suffix(&format!("-{name}")) == Some("aster"));
    if !aster && instance.is_none() {
        return None;
    }
    let digest = Sha256::new()
        .chain_update(b"studio-profile\0")
        .chain_update(storage.as_bytes())
        .chain_update([0])
        .chain_update(legacy)
        .finalize();
    let mut id = [0; 16];
    id.copy_from_slice(&digest[..16]);
    Some(id)
}

pub(crate) fn isolate_webview<'a, R: Runtime, M: Manager<R>>(
    builder: WebviewWindowBuilder<'a, R, M>,
    legacy_store: [u8; 16],
) -> io::Result<WebviewWindowBuilder<'a, R, M>> {
    let instance = super::instance_name()?;
    let storage = super::storage_slug_for(&super::selected_slug()?, instance.as_deref())?;
    let Some(id) = isolated_store_id(&storage, instance.as_deref(), legacy_store) else {
        return Ok(builder);
    };
    #[cfg(target_os = "macos")]
    {
        // Older WKWebView cannot select a persistent store for a side instance.
        if instance.is_some()
            && objc2_foundation::NSProcessInfo::processInfo()
                .operatingSystemVersion()
                .majorVersion
                < 14
        {
            return Ok(builder.incognito(true));
        }
        Ok(builder.data_store_identifier(id))
    }
    #[cfg(not(target_os = "macos"))]
    {
        let directory = super::profile_dir(&storage).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "studio browser profile unavailable",
            )
        })?;
        let name: String = id.iter().map(|byte| format!("{byte:02x}")).collect();
        Ok(builder.data_directory(directory.join("webview").join(name)))
    }
}
