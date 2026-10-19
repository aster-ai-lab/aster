//! Tickets and submissions for an office applet window.
//!
//! A ticket is bound to one applet id and one window label. It lives in
//! memory until the window is destroyed. The shell bearer is rejected on
//! these routes. Submissions are idempotent on `(appId, idempotencyKey)`.

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Deserialize;
use serde_json::json;

use crate::applet_product::{self, AppletProductStore};

struct Ticket {
    app_id: String,
    label: String,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct Submission {
    id: String,
    #[serde(rename = "appId")]
    app_id: String,
    version: String,
    digest: String,
    #[serde(rename = "idempotencyKey")]
    idempotency_key: String,
    values: BTreeMap<String, String>,
    #[serde(rename = "createdAt")]
    created_at: u64,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct SubmissionFile {
    submissions: Vec<Submission>,
}

#[derive(Deserialize)]
struct SubmitBody {
    version: String,
    digest: String,
    #[serde(rename = "idempotencyKey")]
    idempotency_key: String,
    values: BTreeMap<String, String>,
}

pub struct AppletAccess {
    path: Option<PathBuf>,
    tickets: Mutex<HashMap<String, Ticket>>,
    submissions: Mutex<Vec<Submission>>,
}

impl AppletAccess {
    pub fn open(slug: &str) -> Self {
        let path = submissions_path(slug);
        let submissions = path
            .as_ref()
            .and_then(|path| fs::read(path).ok())
            .and_then(|bytes| serde_json::from_slice::<SubmissionFile>(&bytes).ok())
            .map(|file| file.submissions)
            .unwrap_or_default();
        Self {
            path,
            tickets: Mutex::new(HashMap::new()),
            submissions: Mutex::new(submissions),
        }
    }

    pub fn mint(&self, app_id: &str, label: &str, shell_token: &str) -> Result<String, String> {
        if !applet_product::is_office_app(app_id)
            && app_id != "pdf"
            && app_id != "review"
            && app_id != "inbox"
            && app_id != "plan"
        {
            return Err("Unknown workbench applet.".to_owned());
        }
        let mut tickets = self.tickets.lock().expect("applet tickets");
        if let Some((token, _)) = tickets
            .iter()
            .find(|(_, ticket)| ticket.label == label && ticket.app_id == app_id)
        {
            return Ok(token.clone());
        }
        for _ in 0..4 {
            let mut bytes = [0u8; 32];
            if getrandom::getrandom(&mut bytes).is_err() {
                break;
            }
            let token = hex_encode(&bytes);
            if tokens_match(&token, shell_token) || tickets.contains_key(&token) {
                continue;
            }
            tickets.insert(
                token.clone(),
                Ticket {
                    app_id: app_id.to_owned(),
                    label: label.to_owned(),
                },
            );
            return Ok(token);
        }
        Err("Could not open the applet window.".to_owned())
    }

    pub fn revoke_label(&self, label: &str) {
        self.tickets
            .lock()
            .expect("applet tickets")
            .retain(|_, ticket| ticket.label != label);
    }

    /// The shell bearer is never a ticket, and a ticket only matches its app id.
    pub fn allows(&self, bearer: &str, app_id: &str, shell_token: &str) -> bool {
        if bearer.is_empty() || tokens_match(bearer, shell_token) {
            return false;
        }
        self.tickets
            .lock()
            .expect("applet tickets")
            .get(bearer)
            .is_some_and(|ticket| ticket.app_id == app_id)
    }

    pub fn submit(
        &self,
        app_id: &str,
        products: &AppletProductStore,
        body: &[u8],
    ) -> Result<String, u16> {
        if !applet_product::is_office_app(app_id) {
            return Err(404);
        }
        let parsed: SubmitBody = serde_json::from_slice(body).map_err(|_| 400u16)?;
        let key = clean_idempotency_key(&parsed.idempotency_key)?;
        {
            let submissions = self.submissions.lock().expect("applet submissions");
            if let Some(existing) = submissions
                .iter()
                .find(|item| item.app_id == app_id && item.idempotency_key == key)
            {
                return Ok(submission_json(existing));
            }
        }
        let package = products.package_for_version(app_id, &parsed.version)?;
        let digest = applet_product::sha256_hex(package.as_bytes());
        if !tokens_match(&digest, parsed.digest.trim()) {
            return Err(400);
        }
        let fields = applet_product::fields_of_package(&package)?;
        let values = clean_values(&fields, &parsed.values)?;
        let mut submissions = self.submissions.lock().expect("applet submissions");
        if let Some(existing) = submissions
            .iter()
            .find(|item| item.app_id == app_id && item.idempotency_key == key)
        {
            return Ok(submission_json(existing));
        }
        if submissions
            .iter()
            .filter(|item| item.app_id == app_id)
            .count()
            >= 500
        {
            return Err(409);
        }
        let record = Submission {
            id: hex_encode(&random_bytes()?),
            app_id: app_id.to_owned(),
            version: parsed.version.trim().to_owned(),
            digest,
            idempotency_key: key,
            values,
            created_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_millis() as u64)
                .unwrap_or(0),
        };
        let json = submission_json(&record);
        submissions.push(record);
        if let Err(status) = self.persist(&submissions) {
            submissions.pop();
            return Err(status);
        }
        Ok(json)
    }
}

impl AppletAccess {
    fn persist(&self, submissions: &[Submission]) -> Result<(), u16> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        let body = serde_json::to_vec(&SubmissionFile {
            submissions: submissions.to_vec(),
        })
        .map_err(|_| 500u16)?;
        write_atomic(path, &body)
    }
}

fn submission_json(record: &Submission) -> String {
    json!({
        "id": record.id,
        "appId": record.app_id,
        "version": record.version,
        "digest": record.digest,
        "idempotencyKey": record.idempotency_key,
        "values": record.values,
        "createdAt": record.created_at,
    })
    .to_string()
}

fn clean_values(
    fields: &[applet_product::PackageField],
    values: &BTreeMap<String, String>,
) -> Result<BTreeMap<String, String>, u16> {
    if values.len() > fields.len() {
        return Err(400);
    }
    let mut cleaned = BTreeMap::new();
    for field in fields {
        let raw = values.get(&field.name).map(String::as_str).unwrap_or("");
        let value = raw.trim();
        if value.chars().any(|ch| ch.is_control()) {
            return Err(400);
        }
        if field.required && value.is_empty() {
            return Err(400);
        }
        if value.chars().count() > field.max {
            return Err(400);
        }
        if values.contains_key(&field.name) {
            cleaned.insert(field.name.clone(), value.to_owned());
        }
    }
    if values
        .keys()
        .any(|key| !fields.iter().any(|field| field.name == *key))
    {
        return Err(400);
    }
    Ok(cleaned)
}

fn clean_idempotency_key(value: &str) -> Result<String, u16> {
    let trimmed = value.trim();
    let bytes = trimmed.as_bytes();
    let groups = [8, 4, 4, 4, 12];
    let mut offset = 0;
    for (index, length) in groups.iter().enumerate() {
        if index > 0 {
            if bytes.get(offset) != Some(&b'-') {
                return Err(400);
            }
            offset += 1;
        }
        let end = offset + length;
        let Some(part) = bytes.get(offset..end) else {
            return Err(400);
        };
        if part.len() != *length
            || !part
                .iter()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        {
            return Err(400);
        }
        offset = end;
    }
    if offset != bytes.len() {
        return Err(400);
    }
    Ok(trimmed.to_owned())
}

fn submissions_path(slug: &str) -> Option<PathBuf> {
    if cfg!(test) {
        return None;
    }
    if slug.is_empty()
        || slug.len() > 64
        || !slug
            .bytes()
            .all(|byte| matches!(byte, b'a'..=b'z' | b'0'..=b'9' | b'.' | b'-'))
    {
        return None;
    }
    Some(crate::tenant::profile_dir(slug)?.join("applet-submissions.json"))
}

fn write_atomic(path: &Path, body: &[u8]) -> Result<(), u16> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|_| 500u16)?;
    }
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, body).map_err(|_| 500u16)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&temporary, fs::Permissions::from_mode(0o600)).map_err(|_| 500u16)?;
    }
    fs::rename(&temporary, path).map_err(|_| 500u16)?;
    Ok(())
}

fn random_bytes() -> Result<[u8; 16], u16> {
    let mut bytes = [0u8; 16];
    getrandom::getrandom(&mut bytes).map_err(|_| 500u16)?;
    Ok(bytes)
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0xf) as usize] as char);
    }
    out
}

fn tokens_match(presented: &str, expected: &str) -> bool {
    let presented = presented.as_bytes();
    let expected = expected.as_bytes();
    if presented.len() != expected.len() {
        return false;
    }
    let mut diff = 0u8;
    for (left, right) in presented.iter().zip(expected.iter()) {
        diff |= left ^ right;
    }
    diff == 0
}

pub fn runtime_app_id(path: &str) -> Option<&str> {
    let mut parts = path.split('/').filter(|part| !part.is_empty());
    if parts.next() != Some("api") || parts.next() != Some("applet-runtime") {
        return None;
    }
    let app_id = parts.next()?;
    if applet_product::applet_id_ok(app_id) {
        Some(app_id)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::AppletAccess;
    use crate::applet_product::{AppletProductStore, sha256_hex};
    use serde_json::Value;

    #[test]
    fn a_ticket_only_opens_its_own_applet() {
        let access = AppletAccess::open("toyou");
        let ticket = access.mint("leave", "applet-leave", "ab").unwrap();
        assert!(access.allows(&ticket, "leave", "ab"));
        assert!(!access.allows(&ticket, "travel", "ab"));
        assert!(!access.allows("ab", "leave", "ab"));
        access.revoke_label("applet-leave");
        assert!(!access.allows(&ticket, "leave", "ab"));
    }

    #[test]
    fn submitting_the_same_key_returns_the_first_record() {
        let products = AppletProductStore::open_at(None);
        let package = products.package("leave", "release").unwrap();
        let digest = sha256_hex(package.as_bytes());
        let access = AppletAccess::open("toyou");
        let body = format!(
            "{{\"version\":\"1.1.0\",\"digest\":\"{digest}\",\"idempotencyKey\":\"11111111-1111-4111-8111-111111111111\",\"values\":{{\"title\":\"明天\",\"detail\":\"事假\"}}}}"
        );
        let first: Value =
            serde_json::from_str(&access.submit("leave", &products, body.as_bytes()).unwrap())
                .unwrap();
        let second: Value =
            serde_json::from_str(&access.submit("leave", &products, body.as_bytes()).unwrap())
                .unwrap();
        assert_eq!(first["id"], second["id"]);
        assert_eq!(first["values"]["title"], "明天");
        assert!(first.get("phone").is_none());
        assert!(!first.to_string().contains("a3s_ms_"));
        let missing = format!(
            "{{\"version\":\"1.1.0\",\"digest\":\"{digest}\",\"idempotencyKey\":\"22222222-2222-4222-8222-222222222222\",\"values\":{{\"detail\":\"事假\"}}}}"
        );
        assert!(
            access
                .submit("leave", &products, missing.as_bytes())
                .is_err()
        );
    }
}
