//! Studio-owned locations. Aster never adopts the legacy Work profile.

use std::path::{Path, PathBuf};

use super::is_slug;

#[derive(Clone, Copy)]
pub(super) enum ProfileOs {
    Windows,
    Macos,
    Linux,
}

pub(super) struct ProfileEnv {
    pub home: Option<PathBuf>,
    pub user_profile: Option<PathBuf>,
    pub appdata: Option<PathBuf>,
    pub xdg_data_home: Option<PathBuf>,
    pub os: ProfileOs,
}

impl ProfileEnv {
    fn from_process() -> Self {
        fn path(name: &str) -> Option<PathBuf> {
            std::env::var_os(name)
                .filter(|value| !value.is_empty())
                .map(PathBuf::from)
        }
        Self {
            home: path("HOME"),
            user_profile: path("USERPROFILE"),
            appdata: path("APPDATA"),
            xdg_data_home: path("XDG_DATA_HOME"),
            os: if cfg!(windows) {
                ProfileOs::Windows
            } else if cfg!(target_os = "macos") {
                ProfileOs::Macos
            } else {
                ProfileOs::Linux
            },
        }
    }

    fn platform_root(&self) -> Option<PathBuf> {
        let home = self.home.as_deref().or(self.user_profile.as_deref());
        let root = match self.os {
            ProfileOs::Windows => self.appdata.clone().or_else(|| {
                self.user_profile
                    .as_deref()
                    .map(|path| path.join("AppData/Roaming"))
            })?,
            ProfileOs::Macos => home?.join("Library/Application Support"),
            ProfileOs::Linux => self
                .xdg_data_home
                .clone()
                .or_else(|| home.map(|path| path.join(".local/share")))?,
        };
        Some(root.join("dev.a3s.studio"))
    }
}

pub(crate) struct ProfileLocation {
    pub directory: PathBuf,
    pub machine_root: PathBuf,
}

fn brand<'a>(storage: &'a str, instance: Option<&str>) -> &'a str {
    instance
        .and_then(|name| storage.strip_suffix(&format!("-{name}")))
        .filter(|name| is_slug(name))
        .unwrap_or(storage)
}

pub fn profile_dir_under(root: &Path, storage: &str, instance: Option<&str>) -> PathBuf {
    let brand = brand(storage, instance);
    if brand == "aster" {
        let root = root.join("studio/aster");
        return match instance.filter(|name| is_slug(name)) {
            Some(name) if storage != brand => root.join("instances").join(name),
            _ => root,
        };
    }
    let legacy = root.join(storage);
    if legacy.is_dir() {
        return legacy;
    }
    match instance.filter(|name| is_slug(name)) {
        Some(name) if brand != storage => root.join(brand).join("instances").join(name),
        _ => legacy,
    }
}

pub(super) fn resolve_profile_location(
    storage: &str,
    instance: Option<&str>,
    env: &ProfileEnv,
) -> Option<ProfileLocation> {
    if storage.is_empty()
        || storage.len() > 64
        || !storage
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        || instance.is_some_and(|name| !is_slug(name))
    {
        return None;
    }
    let aster = brand(storage, instance) == "aster";
    let legacy_root = env
        .home
        .as_deref()
        .map(|home| home.join("Library/Application Support/dev.a3s.studio"));
    let root = if !aster {
        legacy_root
            .filter(|root| profile_dir_under(root, storage, instance).is_dir())
            .or_else(|| env.platform_root())?
    } else {
        env.platform_root()?
    };
    Some(ProfileLocation {
        directory: profile_dir_under(&root, storage, instance),
        machine_root: if aster {
            root.join("studio/aster")
        } else {
            root
        },
    })
}

pub(crate) fn profile_location(storage: &str) -> Option<ProfileLocation> {
    let instance = super::instance_name().ok()?;
    resolve_profile_location(storage, instance.as_deref(), &ProfileEnv::from_process())
}

pub fn profile_dir(storage: &str) -> Option<PathBuf> {
    profile_location(storage).map(|location| location.directory)
}

pub(crate) fn runtime_path_under(
    root: &Path,
    tenant: &str,
    instance: Option<&str>,
) -> std::io::Result<PathBuf> {
    if !is_slug(tenant) || instance.is_some_and(|name| !is_slug(name)) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "invalid runtime identity",
        ));
    }
    let root = root.join("a3s-studio").join(tenant);
    Ok(match instance {
        Some(name) => root.join("instances").join(name).join("runtime.json"),
        None => root.join("runtime.json"),
    })
}
