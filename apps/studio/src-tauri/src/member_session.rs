//! Member sign-in for the studio shell outlives a single launch.
//! The shell is served from a new loopback port every start, and web storage
//! is partitioned by that origin, so localStorage cannot carry the session.
//! The record sits next to the tenant's provider file.

use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use serde::Deserialize;
use serde_json::json;

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct Stored {
    token: String,
    #[serde(default)]
    display_name: String,
    #[serde(default)]
    phone: String,
    /// Image data URL. Empty when the member has no photo. Old session files omit it.
    #[serde(default)]
    avatar: String,
    /// Member UUID from `/me`. Empty until that response. A client save cannot set it.
    #[serde(default)]
    member_id: String,
}

#[derive(Deserialize)]
struct SaveBody {
    token: String,
    #[serde(default)]
    display_name: String,
    #[serde(default)]
    phone: String,
    #[serde(default)]
    avatar: String,
}

pub struct MemberSessionStore {
    path: Option<PathBuf>,
    computer_id: String,
    session: Mutex<Option<Stored>>,
    /// Token and member UUID from the last relayed org sign-in response.
    signed_in: Mutex<Option<(String, String)>>,
}

impl MemberSessionStore {
    pub fn open(slug: &str) -> Self {
        // Library tests call the live listener. Keeping them in memory avoids
        // writing the tenant file under Application Support.
        if cfg!(test) {
            return Self::open_with(None, None);
        }
        let Some(root) = machine_root() else {
            return Self::open_with(None, None);
        };
        let instance = crate::tenant::instance_name().ok().flatten();
        let dir = crate::tenant::profile_dir_under(&root, slug, instance.as_deref());
        if dir.parent() == Some(root.as_path()) {
            return Self::open_app(&root, slug);
        }
        let session = if app_id_ok(slug) {
            Some(dir.join("member-session.json"))
        } else {
            None
        };
        Self::open_with(session, Some(root.join("computer-id")))
    }

    /// One directory per app under a shared machine root. Every app reads the same computer id.
    pub fn open_app(root: &std::path::Path, app_id: &str) -> Self {
        let session = if app_id_ok(app_id) {
            Some(root.join(app_id).join("member-session.json"))
        } else {
            None
        };
        Self::open_with(session, Some(root.join("computer-id")))
    }

    pub fn open_at(path: Option<PathBuf>) -> Self {
        Self::open_with(path, None)
    }

    fn open_with(path: Option<PathBuf>, computer_path: Option<PathBuf>) -> Self {
        let session = path
            .as_ref()
            .and_then(|path| fs::read(path).ok())
            .and_then(|bytes| serde_json::from_slice::<Stored>(&bytes).ok())
            .filter(|stored| valid_token(&stored.token));
        Self {
            path,
            computer_id: load_or_create_computer_id(computer_path.as_deref()),
            session: Mutex::new(session),
            signed_in: Mutex::new(None),
        }
    }

    /// The org sign-in response names the member. The page saves that token next,
    /// so preferences can resolve their scope before the main window first reads them.
    pub fn note_sign_in(&self, token: &str, member_id: &str) {
        let (token, id) = (token.trim(), member_id.trim());
        if valid_token(token) && member_uuid_ok(id) {
            *self.signed_in.lock().expect("member sign-in") =
                Some((token.to_owned(), id.to_owned()));
        }
    }

    /// Stable id for this machine. Apps opened from the same root share it.
    pub fn computer_id(&self) -> &str {
        &self.computer_id
    }

    pub fn has_session(&self) -> bool {
        self.session.lock().expect("member session").is_some()
    }

    /// Bearer for upstream member calls. Stays in this process.
    pub fn token(&self) -> Option<String> {
        self.session
            .lock()
            .expect("member session")
            .as_ref()
            .map(|stored| stored.token.clone())
    }

    /// Keeps authoring commits ordered before a member replacement or logout completes.
    pub(crate) fn with_resource_session<T>(
        &self,
        action: impl FnOnce(&str, &str) -> std::io::Result<T>,
    ) -> std::io::Result<T> {
        let guard = self
            .session
            .lock()
            .map_err(|_| std::io::Error::other("member session unavailable"))?;
        let stored = guard
            .as_ref()
            .filter(|stored| member_uuid_ok(&stored.member_id))
            .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::NotFound))?;
        action(&stored.token, &stored.member_id)
    }

    pub fn get_json(&self) -> String {
        let guard = self.session.lock().expect("member session");
        match guard.as_ref() {
            Some(stored) => session_json(stored),
            None => json!({ "token": null }).to_string(),
        }
    }

    /// Name and phone for the renderer. The session token is not a field.
    pub fn profile_json(&self) -> String {
        let guard = self.session.lock().expect("member session");
        match guard.as_ref() {
            Some(stored) => json!({
                "signedIn": true,
                "display_name": stored.display_name,
                "phone": stored.phone,
                "avatar": stored.avatar,
            })
            .to_string(),
            None => json!({
                "signedIn": false,
                "display_name": "",
                "phone": "",
                "avatar": "",
            })
            .to_string(),
        }
    }

    /// Name only. The applet window does not receive the phone, the photo, or the session token.
    /// The photo is on `applet_profile_json`, after the member allows that applet to show it.
    pub fn applet_session_json(&self) -> String {
        let guard = self.session.lock().expect("member session");
        match guard.as_ref() {
            Some(stored) => json!({
                "signedIn": true,
                "displayName": stored.display_name,
            })
            .to_string(),
            None => json!({
                "signedIn": false,
                "displayName": "",
            })
            .to_string(),
        }
    }

    /// Name and photo for an applet the member already allowed.
    /// The phone and the session token are not fields.
    pub fn applet_profile_json(&self) -> String {
        let guard = self.session.lock().expect("member session");
        match guard.as_ref() {
            Some(stored) => json!({
                "displayName": stored.display_name,
                "avatar": accepted_avatar(&stored.avatar).unwrap_or_default(),
            })
            .to_string(),
            None => json!({
                "displayName": "",
                "avatar": "",
            })
            .to_string(),
        }
    }

    pub fn save(&self, body: &[u8]) -> Result<String, u16> {
        let parsed: SaveBody = serde_json::from_slice(body).map_err(|_| 400u16)?;
        let token = parsed.token.trim();
        if !valid_token(token) {
            return Err(400u16);
        }
        let member_id = {
            let guard = self.session.lock().expect("member session");
            guard
                .as_ref()
                .filter(|stored| stored.token == token)
                .map(|stored| stored.member_id.clone())
                .filter(|id| member_uuid_ok(id))
                .or_else(|| {
                    self.signed_in
                        .lock()
                        .expect("member sign-in")
                        .take()
                        .filter(|(signed, _)| signed == token)
                        .map(|(_, id)| id)
                })
                .unwrap_or_default()
        };
        let stored = Stored {
            token: token.to_owned(),
            display_name: clean_label(&parsed.display_name, 80)?,
            phone: clean_phone(&parsed.phone)?,
            avatar: accepted_avatar(parsed.avatar.trim()).unwrap_or_default(),
            member_id,
        };
        self.write(&stored)?;
        let json = session_json(&stored);
        *self.session.lock().expect("member session") = Some(stored);
        Ok(json)
    }

    /// Keeps the saved name and photo in step with the OS member record.
    /// A missing name or an invalid photo leaves that field as it was.
    /// An empty photo clears it. The phone and the token stay untouched.
    pub fn remember_profile(
        &self,
        display_name: Option<&str>,
        avatar: Option<&str>,
    ) -> Result<(), u16> {
        let mut guard = self.session.lock().expect("member session");
        let Some(stored) = guard.as_mut() else {
            return Ok(());
        };
        if let Some(name) = display_name {
            let trimmed = name.trim();
            if !trimmed.is_empty() {
                stored.display_name = clean_label(trimmed, 80)?;
            }
        }
        if let Some(raw) = avatar {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                stored.avatar.clear();
            } else if let Some(clean) = accepted_avatar(trimmed) {
                stored.avatar = clean;
            }
        }
        self.write(stored)
    }

    /// UUID from a successful `/me` payload. Anything else is refused.
    pub fn remember_member_id(&self, member_id: &str) -> Result<(), u16> {
        let id = member_id.trim();
        if !member_uuid_ok(id) {
            return Err(400);
        }
        let mut guard = self.session.lock().expect("member session");
        let Some(stored) = guard.as_mut() else {
            return Ok(());
        };
        if stored.member_id == id {
            return Ok(());
        }
        stored.member_id = id.to_owned();
        self.write(stored)
    }

    /// Signed-in member UUID. Absent until `/me` has been seen for this token.
    pub fn member_id(&self) -> Option<String> {
        let guard = self.session.lock().expect("member session");
        guard.as_ref().and_then(|stored| {
            let id = stored.member_id.as_str();
            member_uuid_ok(id).then(|| id.to_owned())
        })
    }

    pub(crate) fn with_preference_scope<T>(
        &self,
        action: impl FnOnce(&str) -> Result<T, crate::preferences::PreferenceError>,
    ) -> Result<T, crate::preferences::PreferenceError> {
        let guard = self
            .session
            .lock()
            .map_err(|_| crate::preferences::PreferenceError::Unavailable)?;
        let id = guard
            .as_ref()
            .map(|stored| stored.member_id.as_str())
            .filter(|id| member_uuid_ok(id))
            .ok_or(crate::preferences::PreferenceError::Denied)?;
        action(id)
    }

    /// Drops the saved member sign-in. The next launch shows the login card.
    pub fn clear(&self) -> Result<String, u16> {
        if let Some(path) = &self.path {
            match fs::remove_file(path) {
                Ok(()) => {}
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                Err(_) => return Err(500),
            }
        }
        *self.session.lock().expect("member session") = None;
        Ok(json!({ "token": null }).to_string())
    }
}

impl MemberSessionStore {
    fn write(&self, stored: &Stored) -> Result<(), u16> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|_| 400u16)?;
        }
        let body = serde_json::to_vec(stored).map_err(|_| 400u16)?;
        let temporary = path.with_extension("json.tmp");
        fs::write(&temporary, body).map_err(|_| 400u16)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&temporary, fs::Permissions::from_mode(0o600))
                .map_err(|_| 400u16)?;
        }
        fs::rename(&temporary, path).map_err(|_| 400u16)?;
        Ok(())
    }
}

fn session_json(stored: &Stored) -> String {
    json!({
        "token": stored.token,
        "display_name": stored.display_name,
        "phone": stored.phone,
        "avatar": stored.avatar,
    })
    .to_string()
}

/// A member photo is an image data URL, same cap as the OS profile update.
fn accepted_avatar(value: &str) -> Option<String> {
    if value.is_empty()
        || value.len() > 400_000
        || !value.starts_with("data:image/")
        || !value.contains(";base64,")
        || value.chars().any(|ch| ch.is_control())
    {
        return None;
    }
    Some(value.to_owned())
}

fn valid_token(token: &str) -> bool {
    let rest = token.strip_prefix("a3s_ms_").unwrap_or("");
    (16..=256).contains(&token.len())
        && !rest.is_empty()
        && rest.bytes().all(
            |byte| matches!(byte, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_' | b'-' | b'.'),
        )
}

fn clean_label(value: &str, max: usize) -> Result<String, u16> {
    let trimmed = value.trim();
    if trimmed.chars().count() > max || trimmed.chars().any(|ch| ch.is_control()) {
        return Err(400u16);
    }
    Ok(trimmed.to_owned())
}

fn clean_phone(value: &str) -> Result<String, u16> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(String::new());
    }
    if trimmed.len() > 20 || !trimmed.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(400u16);
    }
    Ok(trimmed.to_owned())
}

fn machine_root() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join("Library/Application Support/dev.a3s.studio"))
}

fn member_uuid_ok(id: &str) -> bool {
    let bytes = id.as_bytes();
    if bytes.len() != 36 {
        return false;
    }
    let groups = [8usize, 4, 4, 4, 12];
    let mut index = 0;
    for (group, length) in groups.iter().enumerate() {
        if group > 0 {
            if bytes[index] != b'-' {
                return false;
            }
            index += 1;
        }
        for _ in 0..*length {
            let byte = bytes[index];
            if !byte.is_ascii_hexdigit() || byte.is_ascii_uppercase() {
                return false;
            }
            index += 1;
        }
    }
    true
}

fn app_id_ok(app_id: &str) -> bool {
    !app_id.is_empty()
        && app_id.len() <= 64
        && app_id
            .bytes()
            .all(|byte| matches!(byte, b'a'..=b'z' | b'0'..=b'9' | b'.' | b'-'))
}

fn computer_id_ok(id: &str) -> bool {
    (8..=128).contains(&id.len())
        && id
            .bytes()
            .all(|byte| matches!(byte, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_' | b'-'))
}

fn new_computer_id() -> String {
    let mut bytes = [0u8; 16];
    if let Ok(mut file) = fs::File::open("/dev/urandom") {
        use std::io::Read;
        let _ = file.read_exact(&mut bytes);
    }
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn read_computer_id(path: &std::path::Path) -> Option<String> {
    let text = fs::read_to_string(path).ok()?;
    let id = text.trim();
    computer_id_ok(id).then(|| id.to_owned())
}

fn load_or_create_computer_id(path: Option<&std::path::Path>) -> String {
    let Some(path) = path else {
        return new_computer_id();
    };
    if let Some(id) = read_computer_id(path) {
        return id;
    }
    let id = new_computer_id();
    if let Some(parent) = path.parent() {
        if fs::create_dir_all(parent).is_err() {
            return read_computer_id(path).unwrap_or(id);
        }
    }
    let temporary = path.with_extension("id.tmp");
    if fs::write(&temporary, format!("{id}\n")).is_err() {
        return read_computer_id(path).unwrap_or(id);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&temporary, fs::Permissions::from_mode(0o600));
    }
    if fs::rename(&temporary, path).is_err() {
        let _ = fs::remove_file(&temporary);
        return read_computer_id(path).unwrap_or(id);
    }
    read_computer_id(path).unwrap_or(id)
}

#[cfg(test)]
mod tests {
    use super::MemberSessionStore;

    #[test]
    fn a_saved_session_survives_a_new_store() {
        let dir = std::env::temp_dir().join(format!(
            "a3s-member-session-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let path = dir.join("member-session.json");
        let store = MemberSessionStore::open_at(Some(path.clone()));
        assert!(!store.has_session());
        assert_eq!(store.get_json(), "{\"token\":null}");
        let unsigned: serde_json::Value = serde_json::from_str(&store.profile_json()).unwrap();
        assert_eq!(unsigned["signedIn"], false);
        assert!(unsigned.get("token").is_none());
        let saved = store
            .save(
                r#"{"token":"a3s_ms_session.token","display_name":" 林 ","phone":"13800138000"}"#
                    .as_bytes(),
            )
            .expect("save");
        assert!(saved.contains("\"display_name\":\"林\""));
        drop(store);

        let again = MemberSessionStore::open_at(Some(path.clone()));
        assert!(again.has_session());
        assert_eq!(again.token().as_deref(), Some("a3s_ms_session.token"));
        let json: serde_json::Value = serde_json::from_str(&again.get_json()).unwrap();
        assert_eq!(json["token"], "a3s_ms_session.token");
        assert_eq!(json["phone"], "13800138000");
        let profile = again.profile_json();
        let profile_json: serde_json::Value = serde_json::from_str(&profile).unwrap();
        assert_eq!(profile_json["signedIn"], true);
        assert_eq!(profile_json["display_name"], "林");
        assert_eq!(profile_json["phone"], "13800138000");
        assert!(profile_json.get("token").is_none());
        assert!(!profile.contains("a3s_ms_"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }
        assert!(again.save(br#"{"token":"nope"}"#).is_err());
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&again.get_json()).unwrap()["token"],
            "a3s_ms_session.token"
        );
        assert_eq!(again.clear().expect("clear"), "{\"token\":null}");
        assert!(!again.has_session());
        assert!(!path.exists());
        assert_eq!(again.get_json(), "{\"token\":null}");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&again.profile_json()).unwrap()["signedIn"],
            false
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn two_apps_share_one_computer_and_keep_their_accounts() {
        let root = std::env::temp_dir().join(format!(
            "a3s-member-apps-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let app1 = MemberSessionStore::open_app(&root, "app-1");
        let app2 = MemberSessionStore::open_app(&root, "app-2");
        let computer = app1.computer_id().to_owned();
        assert_eq!(app2.computer_id(), computer);
        assert!(!computer.is_empty());
        app1.save(
            r#"{"token":"a3s_ms_account-a.token","display_name":"甲","phone":"13800138101"}"#
                .as_bytes(),
        )
        .expect("save A");
        app2.save(
            r#"{"token":"a3s_ms_account-b.token","display_name":"乙","phone":"13800138102"}"#
                .as_bytes(),
        )
        .expect("save B");
        assert_eq!(app1.token().as_deref(), Some("a3s_ms_account-a.token"));
        assert_eq!(app2.token().as_deref(), Some("a3s_ms_account-b.token"));
        drop(app1);
        drop(app2);

        let again1 = MemberSessionStore::open_app(&root, "app-1");
        let again2 = MemberSessionStore::open_app(&root, "app-2");
        assert_eq!(again1.token().as_deref(), Some("a3s_ms_account-a.token"));
        assert_eq!(again2.token().as_deref(), Some("a3s_ms_account-b.token"));
        assert_eq!(again1.computer_id(), computer);
        assert_eq!(again2.computer_id(), computer);
        let first: serde_json::Value = serde_json::from_str(&again1.get_json()).unwrap();
        let second: serde_json::Value = serde_json::from_str(&again2.get_json()).unwrap();
        assert_eq!(first["phone"], "13800138101");
        assert_eq!(second["phone"], "13800138102");
        assert_ne!(first["token"], second["token"]);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn leave_profile_avatar_survives_in_the_session_file() {
        let dir = std::env::temp_dir().join(format!(
            "a3s-member-avatar-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let path = dir.join("member-session.json");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            &path,
            r#"{"token":"a3s_ms_session.token","display_name":"林","phone":"13800138000"}"#,
        )
        .unwrap();
        let store = MemberSessionStore::open_at(Some(path.clone()));
        let session = store.applet_session_json();
        assert!(session.contains("\"displayName\":\"林\""));
        assert!(!session.contains("avatar"));
        assert!(!session.contains("13800138000"));
        assert!(!session.contains("a3s_ms_"));
        let unsigned = store.applet_profile_json();
        let profile: serde_json::Value = serde_json::from_str(&unsigned).unwrap();
        assert_eq!(profile["displayName"], "林");
        assert_eq!(profile["avatar"], "");
        assert!(profile.get("phone").is_none());
        assert!(profile.get("token").is_none());
        assert!(!unsigned.contains("13800138000"));
        store
            .remember_profile(Some(" 林 "), Some("data:image/png;base64,AAAA"))
            .unwrap();
        store
            .remember_profile(None, Some("https://evil.example/a.png"))
            .unwrap();
        let shown = store.applet_profile_json();
        let shown_json: serde_json::Value = serde_json::from_str(&shown).unwrap();
        assert_eq!(shown_json["displayName"], "林");
        assert_eq!(shown_json["avatar"], "data:image/png;base64,AAAA");
        let shell: serde_json::Value = serde_json::from_str(&store.profile_json()).unwrap();
        assert_eq!(shell["avatar"], "data:image/png;base64,AAAA");
        assert!(shell.get("token").is_none());
        assert!(!shown.contains("13800138000"));
        assert!(!shown.contains("a3s_ms_"));
        assert!(!store.applet_session_json().contains("data:image"));
        store.remember_profile(None, Some("")).unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&store.applet_profile_json()).unwrap()["avatar"],
            ""
        );
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&store.profile_json()).unwrap()["avatar"],
            ""
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn member_id_comes_from_the_profile_refresh_and_stays_off_the_session_json() {
        let dir = std::env::temp_dir().join(format!(
            "a3s-member-id-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let path = dir.join("member-session.json");
        let store = MemberSessionStore::open_at(Some(path.clone()));
        let member = "11111111-1111-4111-8111-111111111111";
        store
            .save(
                format!(
                    r#"{{"token":"a3s_ms_session.token","display_name":"林","phone":"13800138000","member_id":"{member}"}}"#
                )
                .as_bytes(),
            )
            .expect("save");
        assert!(store.member_id().is_none());
        assert!(store.remember_member_id("13800138000").is_err());
        assert!(
            store
                .remember_member_id("11111111-1111-4111-8111-11111111111G")
                .is_err()
        );
        assert!(store.member_id().is_none());
        store.remember_member_id(member).unwrap();
        assert_eq!(store.member_id().as_deref(), Some(member));
        let session = store.get_json();
        let profile = store.profile_json();
        let applet = store.applet_session_json();
        assert!(!session.contains(member));
        assert!(!profile.contains(member));
        assert!(!applet.contains(member));
        assert!(!session.contains("member_id"));
        store
            .save(
                r#"{"token":"a3s_ms_session.token","display_name":"林","phone":"13800138000"}"#
                    .as_bytes(),
            )
            .unwrap();
        assert_eq!(store.member_id().as_deref(), Some(member));
        store
            .save(
                r#"{"token":"a3s_ms_account-b.token","display_name":"乙","phone":"13800138102"}"#
                    .as_bytes(),
            )
            .unwrap();
        assert!(store.member_id().is_none());
        let _ = std::fs::remove_dir_all(dir);
    }
}
