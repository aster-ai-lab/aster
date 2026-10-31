//! SQLite proof store. Two processes open one file and share the same rows.
//!
//! Lockout writes compare `version`. Session identity is the `token_hash` primary key.
//! Postgres remains the development and production store.

use std::path::Path;

use aster_orm::{sql_query, Database, SqliteDialect, SqliteExecutor, Value};
use async_trait::async_trait;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use uuid::Uuid;

use crate::contexts::proof::domain::repository::*;
use crate::contexts::proof::domain::rules::LockoutSnapshot;
use crate::contexts::proof::domain::{AuthError, Plane};

const SCHEMA: &str = r#"
create table if not exists auth_credentials (
    plane text not null,
    subject_id text not null,
    password_hash text not null,
    totp_secret text,
    primary key (plane, subject_id)
);
create table if not exists auth_sessions (
    token_hash text primary key,
    plane text not null,
    subject_id text not null,
    org_id text,
    auth_time integer not null,
    amr text not null,
    expires_at integer not null,
    computer_id text
);
create table if not exists auth_lockouts (
    lock_key text primary key,
    failures integer not null,
    locked_until integer not null,
    version integer not null
);
create table if not exists auth_challenges (
    id text primary key,
    plane text not null,
    subject_id text not null,
    org_id text,
    auth_time integer not null,
    expires_at integer not null,
    computer_id text
);
create table if not exists auth_events (
    id integer primary key autoincrement,
    kind text not null,
    plane text not null,
    subject_id text,
    at_epoch integer not null
);
create table if not exists auth_codes (
    code_hash text primary key,
    client_id text not null,
    redirect_uri text not null,
    code_challenge text not null,
    nonce text not null,
    session_json text not null,
    expires_at integer not null
);
create table if not exists auth_used_codes (
    code_hash text primary key
);
create table if not exists auth_refreshes (
    token_hash text primary key,
    client_id text not null,
    session_json text not null,
    expires_at integer not null
);
create table if not exists auth_used_refreshes (
    token_hash text primary key
);
create table if not exists auth_oidc_clients (
    client_id text primary key,
    secret_hash text not null,
    redirect_uris text not null
);
drop table if exists auth_api_keys;
"#;

#[derive(Clone)]
pub struct SqliteAuthStore {
    executor: SqliteExecutor,
}

impl SqliteAuthStore {
    pub async fn open(path: impl AsRef<Path>) -> Result<Self, AuthError> {
        let executor = SqliteExecutor::open(path)
            .await
            .map_err(|err| AuthError::storage(err.to_string()))?;
        executor
            .execute_schema(SCHEMA)
            .await
            .map_err(|err| AuthError::storage(err.to_string()))?;
        // Tables created before computer_id existed keep their rows; a second add is a no-op.
        for statement in [
            "alter table auth_sessions add column computer_id text",
            "alter table auth_challenges add column computer_id text",
        ] {
            if let Err(err) = executor.execute_schema(statement).await {
                let message = err.to_string().to_ascii_lowercase();
                if !message.contains("duplicate column") {
                    return Err(AuthError::storage(err.to_string()));
                }
            }
        }
        Ok(Self { executor })
    }

    fn db(&self) -> Database<SqliteDialect, SqliteExecutor> {
        Database::new(SqliteDialect, self.executor.clone())
    }
}

#[async_trait]
impl AuthStore for SqliteAuthStore {
    async fn put_credential(&self, credential: CredentialRecord) -> Result<(), AuthError> {
        let secret = match credential.totp_secret {
            Some(secret) => Value::String(STANDARD.encode(secret)),
            None => Value::Null,
        };
        self.db()
            .execute(
                sql_query::<()>(
                    "insert into auth_credentials (plane, subject_id, password_hash, totp_secret) values (",
                )
                .bind(credential.plane.as_str())
                .append(", ")
                .bind(credential.subject_id.to_string())
                .append(", ")
                .bind(credential.password_hash)
                .append(", ")
                .bind(secret)
                .append(
                    ") on conflict (plane, subject_id) do update set password_hash = excluded.password_hash, totp_secret = coalesce(excluded.totp_secret, auth_credentials.totp_secret)",
                ),
            )
            .await
            .map_err(storage)?;
        Ok(())
    }

    async fn credential(
        &self,
        plane: Plane,
        subject_id: Uuid,
    ) -> Result<Option<CredentialRecord>, AuthError> {
        let row = self
            .db()
            .fetch_optional_as(
                sql_query::<(String, String)>(
                    "select password_hash, coalesce(totp_secret, '') from auth_credentials where plane = ",
                )
                .bind(plane.as_str())
                .append(" and subject_id = ")
                .bind(subject_id.to_string()),
            )
            .await
            .map_err(storage)?;
        let Some((password_hash, totp_secret)) = row else {
            return Ok(None);
        };
        let totp_secret = if totp_secret.is_empty() {
            None
        } else {
            Some(STANDARD.decode(totp_secret).map_err(storage)?)
        };
        Ok(Some(CredentialRecord {
            plane,
            subject_id,
            password_hash,
            totp_secret,
        }))
    }

    async fn put_session(&self, session: SessionRecord) -> Result<(), AuthError> {
        self.db()
            .execute(
                sql_query::<()>(
                    "insert into auth_sessions (token_hash, plane, subject_id, org_id, auth_time, amr, expires_at, computer_id) values (",
                )
                .bind(session.token_hash)
                .append(", ")
                .bind(session.plane.as_str())
                .append(", ")
                .bind(session.subject_id.to_string())
                .append(", ")
                .bind(opt_id(session.org_id))
                .append(", ")
                .bind(session.auth_time)
                .append(", ")
                .bind(session.amr.join(","))
                .append(", ")
                .bind(session.expires_at)
                .append(", ")
                .bind(opt_text(session.computer_id.as_deref()))
                .append(") on conflict (token_hash) do nothing"),
            )
            .await
            .map_err(storage)?;
        Ok(())
    }

    async fn session_by_hash(&self, token_hash: &str) -> Result<Option<SessionRecord>, AuthError> {
        let row = self
            .db()
            .fetch_optional_as(
                sql_query::<(String, String, String, i64, String, i64, String)>(
                    "select plane, subject_id, coalesce(org_id, ''), auth_time, amr, expires_at, coalesce(computer_id, '') from auth_sessions where token_hash = ",
                )
                .bind(token_hash),
            )
            .await
            .map_err(storage)?;
        let Some((plane, subject_id, org_id, auth_time, amr, expires_at, computer_id)) = row else {
            return Ok(None);
        };
        Ok(Some(SessionRecord {
            token_hash: token_hash.to_owned(),
            plane: Plane::parse(&plane).unwrap_or(Plane::Member),
            subject_id: parse_id(&subject_id)?,
            org_id: parse_opt_id(&org_id)?,
            auth_time,
            amr: split_amr(&amr),
            expires_at,
            computer_id: present_text(&computer_id),
        }))
    }

    async fn delete_session_hash(&self, token_hash: &str) -> Result<(), AuthError> {
        self.db()
            .execute(
                sql_query::<()>("delete from auth_sessions where token_hash = ").bind(token_hash),
            )
            .await
            .map_err(storage)?;
        Ok(())
    }

    async fn delete_subject_sessions(
        &self,
        plane: Plane,
        subject_id: Uuid,
    ) -> Result<(), AuthError> {
        self.db()
            .execute(
                sql_query::<()>("delete from auth_sessions where plane = ")
                    .bind(plane.as_str())
                    .append(" and subject_id = ")
                    .bind(subject_id.to_string()),
            )
            .await
            .map_err(storage)?;
        Ok(())
    }

    async fn delete_other_computer_sessions(
        &self,
        plane: Plane,
        subject_id: Uuid,
        computer_id: &str,
    ) -> Result<(), AuthError> {
        self.db()
            .execute(
                sql_query::<()>("delete from auth_sessions where plane = ")
                    .bind(plane.as_str())
                    .append(" and subject_id = ")
                    .bind(subject_id.to_string())
                    .append(" and (computer_id is null or computer_id <> ")
                    .bind(computer_id)
                    .append(")"),
            )
            .await
            .map_err(storage)?;
        Ok(())
    }

    async fn lockout(&self, key: &str) -> Result<Option<LockoutSnapshot>, AuthError> {
        let row = self
            .db()
            .fetch_optional_as(
                sql_query::<(i64, i64, i64)>(
                    "select failures, locked_until, version from auth_lockouts where lock_key = ",
                )
                .bind(key),
            )
            .await
            .map_err(storage)?;
        Ok(
            row.map(|(failures, locked_until, version)| LockoutSnapshot {
                failures: failures.max(0) as u32,
                locked_until,
                version: version.max(0) as u64,
            }),
        )
    }

    async fn put_lockout(
        &self,
        key: &str,
        snapshot: LockoutSnapshot,
    ) -> Result<LockoutWrite, AuthError> {
        let written = if snapshot.version == 0 {
            self.db()
                .fetch_optional_as(
                    sql_query::<i64>(
                        "insert into auth_lockouts (lock_key, failures, locked_until, version) values (",
                    )
                    .bind(key)
                    .append(", ")
                    .bind(snapshot.failures as i64)
                    .append(", ")
                    .bind(snapshot.locked_until)
                    .append(", 1) on conflict (lock_key) do update set failures = excluded.failures, locked_until = excluded.locked_until, version = auth_lockouts.version + 1 where auth_lockouts.version = ")
                    .bind(0_i64)
                    .append(" returning version"),
                )
                .await
                .map_err(storage)?
        } else {
            self.db()
                .fetch_optional_as(
                    sql_query::<i64>("update auth_lockouts set failures = ")
                        .bind(snapshot.failures as i64)
                        .append(", locked_until = ")
                        .bind(snapshot.locked_until)
                        .append(", version = version + 1 where lock_key = ")
                        .bind(key)
                        .append(" and version = ")
                        .bind(snapshot.version as i64)
                        .append(" returning version"),
                )
                .await
                .map_err(storage)?
        };
        Ok(match written {
            Some(_) => LockoutWrite::Applied,
            None => LockoutWrite::Lost,
        })
    }

    async fn database_unix_secs(&self) -> Result<i64, AuthError> {
        self.db()
            .fetch_one_as(sql_query::<i64>(
                "select cast(strftime('%s','now') as integer)",
            ))
            .await
            .map_err(storage)
    }

    async fn put_challenge(&self, challenge: ChallengeRecord) -> Result<(), AuthError> {
        self.db()
            .execute(
                sql_query::<()>(
                    "insert into auth_challenges (id, plane, subject_id, org_id, auth_time, expires_at, computer_id) values (",
                )
                .bind(challenge.id.to_string())
                .append(", ")
                .bind(challenge.plane.as_str())
                .append(", ")
                .bind(challenge.subject_id.to_string())
                .append(", ")
                .bind(opt_id(challenge.org_id))
                .append(", ")
                .bind(challenge.auth_time)
                .append(", ")
                .bind(challenge.expires_at)
                .append(", ")
                .bind(opt_text(challenge.computer_id.as_deref()))
                .append(")"),
            )
            .await
            .map_err(storage)?;
        Ok(())
    }

    async fn take_challenge(&self, id: Uuid) -> Result<Option<ChallengeRecord>, AuthError> {
        let row = self
            .db()
            .fetch_optional_as(
                sql_query::<(String, String, String, i64, i64, String)>(
                    "delete from auth_challenges where id = ",
                )
                .bind(id.to_string())
                .append(
                    " returning plane, subject_id, coalesce(org_id, ''), auth_time, expires_at, coalesce(computer_id, '')",
                ),
            )
            .await
            .map_err(storage)?;
        match row {
            None => Ok(None),
            Some((plane, subject_id, org_id, auth_time, expires_at, computer_id)) => {
                Ok(Some(ChallengeRecord {
                    id,
                    plane: Plane::parse(&plane).unwrap_or(Plane::Member),
                    subject_id: parse_id(&subject_id)?,
                    org_id: parse_opt_id(&org_id)?,
                    auth_time,
                    expires_at,
                    computer_id: present_text(&computer_id),
                }))
            }
        }
    }

    async fn append_event(&self, event: AuthEvent) -> Result<(), AuthError> {
        self.db()
            .execute(
                sql_query::<()>(
                    "insert into auth_events (kind, plane, subject_id, at_epoch) values (",
                )
                .bind(event.kind)
                .append(", ")
                .bind(event.plane.as_str())
                .append(", ")
                .bind(opt_id(event.subject_id))
                .append(", ")
                .bind(event.at)
                .append(")"),
            )
            .await
            .map_err(storage)?;
        Ok(())
    }

    async fn events(&self) -> Result<Vec<AuthEvent>, AuthError> {
        let rows = self
            .db()
            .fetch_all_as(
                sql_query::<(String, String, String, i64)>(
                    "select kind, plane, coalesce(subject_id, ''), at_epoch from auth_events order by id",
                ),
            )
            .await
            .map_err(storage)?;
        rows.rows
            .into_iter()
            .map(|(kind, plane, subject_id, at)| {
                Ok(AuthEvent {
                    kind,
                    plane: Plane::parse(&plane).unwrap_or(Plane::Member),
                    subject_id: parse_opt_id(&subject_id)?,
                    at,
                })
            })
            .collect()
    }

    async fn put_code(&self, code: AuthCodeRecord) -> Result<(), AuthError> {
        let session = super::session_json::encode_session(&code.session)?;
        self.db()
            .execute(
                sql_query::<()>(
                    "insert into auth_codes (code_hash, client_id, redirect_uri, code_challenge, nonce, session_json, expires_at) values (",
                )
                .bind(code.code_hash)
                .append(", ")
                .bind(code.client_id)
                .append(", ")
                .bind(code.redirect_uri)
                .append(", ")
                .bind(code.code_challenge)
                .append(", ")
                .bind(code.nonce)
                .append(", ")
                .bind(session)
                .append(", ")
                .bind(code.expires_at)
                .append(")"),
            )
            .await
            .map_err(storage)?;
        Ok(())
    }

    async fn take_code(&self, code_hash: &str) -> Result<Option<AuthCodeRecord>, AuthError> {
        let row = self
            .db()
            .fetch_optional_as(
                sql_query::<(String, String, String, String, String, i64)>(
                    "delete from auth_codes where code_hash = ",
                )
                .bind(code_hash)
                .append(
                    " returning client_id, redirect_uri, code_challenge, nonce, session_json, expires_at",
                ),
            )
            .await
            .map_err(storage)?;
        match row {
            None => Ok(None),
            Some((client_id, redirect_uri, code_challenge, nonce, session_json, expires_at)) => {
                let session = super::session_json::decode_session(&session_json)?;
                Ok(Some(AuthCodeRecord {
                    code_hash: code_hash.to_owned(),
                    client_id,
                    redirect_uri,
                    code_challenge,
                    nonce,
                    session,
                    expires_at,
                }))
            }
        }
    }

    async fn mark_code_used(&self, code_hash: &str) -> Result<(), AuthError> {
        self.db()
            .execute(
                sql_query::<()>("insert into auth_used_codes (code_hash) values (")
                    .bind(code_hash)
                    .append(") on conflict do nothing"),
            )
            .await
            .map_err(storage)?;
        Ok(())
    }

    async fn code_was_used(&self, code_hash: &str) -> Result<bool, AuthError> {
        let count = self
            .db()
            .fetch_one_as(
                sql_query::<i64>("select count(*) from auth_used_codes where code_hash = ")
                    .bind(code_hash),
            )
            .await
            .map_err(storage)?;
        Ok(count > 0)
    }

    async fn put_refresh(&self, refresh: RefreshRecord) -> Result<(), AuthError> {
        let session = super::session_json::encode_session(&refresh.session)?;
        self.db()
            .execute(
                sql_query::<()>(
                    "insert into auth_refreshes (token_hash, client_id, session_json, expires_at) values (",
                )
                .bind(refresh.token_hash)
                .append(", ")
                .bind(refresh.client_id)
                .append(", ")
                .bind(session)
                .append(", ")
                .bind(refresh.expires_at)
                .append(")"),
            )
            .await
            .map_err(storage)?;
        Ok(())
    }

    async fn take_refresh(&self, token_hash: &str) -> Result<Option<RefreshRecord>, AuthError> {
        let row = self
            .db()
            .fetch_optional_as(
                sql_query::<(String, String, i64)>(
                    "delete from auth_refreshes where token_hash = ",
                )
                .bind(token_hash)
                .append(" returning client_id, session_json, expires_at"),
            )
            .await
            .map_err(storage)?;
        match row {
            None => Ok(None),
            Some((client_id, session_json, expires_at)) => Ok(Some(RefreshRecord {
                token_hash: token_hash.to_owned(),
                client_id,
                session: super::session_json::decode_session(&session_json)?,
                expires_at,
            })),
        }
    }

    async fn mark_refresh_used(&self, token_hash: &str) -> Result<(), AuthError> {
        self.db()
            .execute(
                sql_query::<()>("insert into auth_used_refreshes (token_hash) values (")
                    .bind(token_hash)
                    .append(") on conflict do nothing"),
            )
            .await
            .map_err(storage)?;
        Ok(())
    }

    async fn refresh_was_used(&self, token_hash: &str) -> Result<bool, AuthError> {
        let count = self
            .db()
            .fetch_one_as(
                sql_query::<i64>("select count(*) from auth_used_refreshes where token_hash = ")
                    .bind(token_hash),
            )
            .await
            .map_err(storage)?;
        Ok(count > 0)
    }

    async fn put_client(&self, client: OidcClientRecord) -> Result<(), AuthError> {
        let redirect_uris = serde_json::to_string(&client.redirect_uris).map_err(storage)?;
        self.db()
            .execute(
                sql_query::<()>(
                    "insert into auth_oidc_clients (client_id, secret_hash, redirect_uris) values (",
                )
                .bind(client.client_id)
                .append(", ")
                .bind(client.secret_hash)
                .append(", ")
                .bind(redirect_uris)
                .append(
                    ") on conflict (client_id) do update set secret_hash = excluded.secret_hash, redirect_uris = excluded.redirect_uris",
                ),
            )
            .await
            .map_err(storage)?;
        Ok(())
    }

    async fn client(&self, client_id: &str) -> Result<Option<OidcClientRecord>, AuthError> {
        let row = self
            .db()
            .fetch_optional_as(
                sql_query::<(String, String)>(
                    "select secret_hash, redirect_uris from auth_oidc_clients where client_id = ",
                )
                .bind(client_id),
            )
            .await
            .map_err(storage)?;
        match row {
            None => Ok(None),
            Some((secret_hash, redirect_uris)) => Ok(Some(OidcClientRecord {
                client_id: client_id.to_owned(),
                secret_hash,
                redirect_uris: serde_json::from_str(&redirect_uris).map_err(storage)?,
            })),
        }
    }
}

fn opt_id(value: Option<Uuid>) -> Value {
    match value {
        Some(value) => Value::String(value.to_string()),
        None => Value::Null,
    }
}

fn opt_text(value: Option<&str>) -> Value {
    match value {
        Some(value) => Value::String(value.to_owned()),
        None => Value::Null,
    }
}

fn present_text(value: &str) -> Option<String> {
    if value.is_empty() {
        None
    } else {
        Some(value.to_owned())
    }
}

fn parse_id(value: &str) -> Result<Uuid, AuthError> {
    Uuid::parse_str(value).map_err(storage)
}

fn parse_opt_id(value: &str) -> Result<Option<Uuid>, AuthError> {
    if value.is_empty() {
        Ok(None)
    } else {
        parse_id(value).map(Some)
    }
}

fn split_amr(value: &str) -> Vec<String> {
    if value.is_empty() {
        Vec::new()
    } else {
        value.split(',').map(str::to_owned).collect()
    }
}

fn storage(err: impl ToString) -> AuthError {
    AuthError::storage(err.to_string())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use async_trait::async_trait;
    use uuid::Uuid;

    use std::time::Duration;

    use tokio::sync::{oneshot, Mutex, Notify};

    use super::SqliteAuthStore;
    use crate::contexts::proof::application::service::AuthService;
    use crate::contexts::proof::domain::repository::AuthStore;

    use crate::contexts::proof::domain::repository::{
        AuthCodeRecord, AuthEvent, ChallengeRecord, CredentialRecord, LockoutWrite,
        OidcClientRecord, RefreshRecord, SessionRecord,
    };
    use crate::contexts::proof::domain::rules::{hash_token, LockoutSnapshot, LOCKOUT_SECS};
    use crate::contexts::proof::domain::{
        Admission, AdmitQuery, AuthCode, AuthError, DirectoryAdmission, IntrospectOutcome,
        LoginCommand, LoginOutcome, ManualClock, Plane, SubjectStatus,
    };

    struct FixedDir {
        subject_id: Uuid,
        identifier: String,
    }

    #[async_trait]
    impl DirectoryAdmission for FixedDir {
        async fn admit(&self, query: AdmitQuery) -> Result<Admission, AuthError> {
            if query.identifier == self.identifier
                || (self.identifier == "overlap@example.com"
                    && query.identifier == "clear@example.com")
            {
                Ok(Admission {
                    subject_id: Some(self.subject_id),
                    org_id: None,
                    active: true,
                    console_login: false,
                })
            } else {
                Ok(Admission::unknown())
            }
        }

        async fn subject_status(
            &self,
            _plane: Plane,
            subject_id: Uuid,
        ) -> Result<SubjectStatus, AuthError> {
            Ok(SubjectStatus {
                active: subject_id == self.subject_id,
                console_login: false,
                org_id: None,
            })
        }
    }

    #[tokio::test]
    async fn two_auth_instances_share_one_database() {
        let path = std::env::temp_dir().join(format!("aster-auth-fence-{}.sqlite", Uuid::new_v4()));
        let store_a = Arc::new(SqliteAuthStore::open(&path).await.expect("store a"));
        let store_b = Arc::new(SqliteAuthStore::open(&path).await.expect("store b"));
        let subject_id = Uuid::new_v4();
        let directory = Arc::new(FixedDir {
            subject_id,
            identifier: "member@example.com".into(),
        });
        let clock = Arc::new(ManualClock::new(1_700_000_000));
        let service_a = AuthService::new(
            store_a.clone(),
            directory.clone(),
            clock.clone(),
            "https://auth.aster.test",
        );
        let service_b =
            AuthService::new(store_b.clone(), directory, clock, "https://auth.aster.test");

        service_a
            .enroll_password(Plane::Member, subject_id, "correct-horse")
            .await
            .expect("enroll");
        let session = match service_a
            .login(LoginCommand {
                plane: Plane::Member,
                identifier: "member@example.com".into(),
                org_id: None,
                password: "correct-horse".into(),
                computer_id: Some("computer-sqlite-1".into()),
            })
            .await
            .expect("login")
        {
            LoginOutcome::Session(session) => session,
            other => panic!("expected a session, got {other:?}"),
        };
        match service_b
            .introspect(&session.token, Plane::Member)
            .await
            .expect("introspect")
        {
            IntrospectOutcome::Active(attestation) => {
                assert_eq!(attestation.subject_id, subject_id)
            }
            other => panic!("second instance did not see the session: {other:?}"),
        }

        let hash = hash_token(&session.token);
        let original = store_a
            .session_by_hash(&hash)
            .await
            .expect("session")
            .expect("row");
        let mut spoof = original.clone();
        spoof.subject_id = Uuid::new_v4();
        store_b.put_session(spoof).await.expect("second insert");
        let kept = store_a
            .session_by_hash(&hash)
            .await
            .expect("session")
            .expect("row");
        assert_eq!(kept.subject_id, original.subject_id);

        let denied = service_a
            .login(LoginCommand {
                plane: Plane::Member,
                identifier: "nobody@example.com".into(),
                org_id: None,
                password: "nope".to_string(),

                computer_id: None,
            })
            .await
            .expect("failure");
        assert!(matches!(
            denied,
            LoginOutcome::Denied(AuthCode::InvalidCredentials)
        ));
        let shared = store_b
            .lockout("member:-:nobody@example.com")
            .await
            .expect("lockout")
            .expect("row");
        assert_eq!(shared.failures, 1);

        let applied = store_a
            .put_lockout(
                "member:-:lost@example.com",
                LockoutSnapshot {
                    failures: 1,
                    locked_until: 0,
                    version: 0,
                },
            )
            .await
            .expect("first lockout");
        assert_eq!(applied, LockoutWrite::Applied);
        let lost = store_b
            .put_lockout(
                "member:-:lost@example.com",
                LockoutSnapshot {
                    failures: 9,
                    locked_until: 0,
                    version: 0,
                },
            )
            .await
            .expect("lost compare-and-swap");
        assert_eq!(lost, LockoutWrite::Lost);
        let fenced = store_b
            .lockout("member:-:lost@example.com")
            .await
            .expect("lockout")
            .expect("row");
        assert_eq!(fenced.failures, 1);
        assert_eq!(fenced.version, 1);

        let _ = std::fs::remove_file(&path);
        let mut wal = path.clone().into_os_string();
        wal.push("-wal");
        let _ = std::fs::remove_file(wal);
        let mut shm = path.into_os_string();
        shm.push("-shm");
        let _ = std::fs::remove_file(shm);
    }

    /// Stalls the first `put_lockout` so two logins read one version and write in order.
    struct HeldStore {
        inner: SqliteAuthStore,
        hold: Mutex<Option<oneshot::Receiver<()>>>,
        entered: Notify,
        writes: Mutex<Vec<LockoutWrite>>,
    }

    impl HeldStore {
        fn new(inner: SqliteAuthStore, first: oneshot::Receiver<()>) -> Self {
            Self {
                inner,
                hold: Mutex::new(Some(first)),
                entered: Notify::new(),
                writes: Mutex::new(Vec::new()),
            }
        }

        async fn arm(&self, next: oneshot::Receiver<()>) {
            *self.hold.lock().await = Some(next);
        }
    }

    #[async_trait]
    impl AuthStore for HeldStore {
        async fn put_credential(&self, credential: CredentialRecord) -> Result<(), AuthError> {
            self.inner.put_credential(credential).await
        }

        async fn credential(
            &self,
            plane: Plane,
            subject_id: Uuid,
        ) -> Result<Option<CredentialRecord>, AuthError> {
            self.inner.credential(plane, subject_id).await
        }

        async fn put_session(&self, session: SessionRecord) -> Result<(), AuthError> {
            self.inner.put_session(session).await
        }

        async fn session_by_hash(
            &self,
            token_hash: &str,
        ) -> Result<Option<SessionRecord>, AuthError> {
            self.inner.session_by_hash(token_hash).await
        }

        async fn delete_session_hash(&self, token_hash: &str) -> Result<(), AuthError> {
            self.inner.delete_session_hash(token_hash).await
        }

        async fn delete_subject_sessions(
            &self,
            plane: Plane,
            subject_id: Uuid,
        ) -> Result<(), AuthError> {
            self.inner.delete_subject_sessions(plane, subject_id).await
        }

        async fn delete_other_computer_sessions(
            &self,
            plane: Plane,
            subject_id: Uuid,
            computer_id: &str,
        ) -> Result<(), AuthError> {
            self.inner
                .delete_other_computer_sessions(plane, subject_id, computer_id)
                .await
        }

        async fn lockout(&self, key: &str) -> Result<Option<LockoutSnapshot>, AuthError> {
            self.inner.lockout(key).await
        }

        async fn put_lockout(
            &self,
            key: &str,
            snapshot: LockoutSnapshot,
        ) -> Result<LockoutWrite, AuthError> {
            let waiter = self.hold.lock().await.take();
            if let Some(waiter) = waiter {
                self.entered.notify_one();
                let _ = waiter.await;
            }
            let written = self.inner.put_lockout(key, snapshot).await?;
            self.writes.lock().await.push(written);
            Ok(written)
        }

        async fn database_unix_secs(&self) -> Result<i64, AuthError> {
            self.inner.database_unix_secs().await
        }

        async fn put_challenge(&self, challenge: ChallengeRecord) -> Result<(), AuthError> {
            self.inner.put_challenge(challenge).await
        }

        async fn take_challenge(&self, id: Uuid) -> Result<Option<ChallengeRecord>, AuthError> {
            self.inner.take_challenge(id).await
        }

        async fn append_event(&self, event: AuthEvent) -> Result<(), AuthError> {
            self.inner.append_event(event).await
        }

        async fn events(&self) -> Result<Vec<AuthEvent>, AuthError> {
            self.inner.events().await
        }

        async fn put_code(&self, code: AuthCodeRecord) -> Result<(), AuthError> {
            self.inner.put_code(code).await
        }

        async fn take_code(&self, code_hash: &str) -> Result<Option<AuthCodeRecord>, AuthError> {
            self.inner.take_code(code_hash).await
        }

        async fn mark_code_used(&self, code_hash: &str) -> Result<(), AuthError> {
            self.inner.mark_code_used(code_hash).await
        }

        async fn code_was_used(&self, code_hash: &str) -> Result<bool, AuthError> {
            self.inner.code_was_used(code_hash).await
        }

        async fn put_refresh(&self, refresh: RefreshRecord) -> Result<(), AuthError> {
            self.inner.put_refresh(refresh).await
        }

        async fn take_refresh(&self, token_hash: &str) -> Result<Option<RefreshRecord>, AuthError> {
            self.inner.take_refresh(token_hash).await
        }

        async fn mark_refresh_used(&self, token_hash: &str) -> Result<(), AuthError> {
            self.inner.mark_refresh_used(token_hash).await
        }

        async fn refresh_was_used(&self, token_hash: &str) -> Result<bool, AuthError> {
            self.inner.refresh_was_used(token_hash).await
        }

        async fn put_client(&self, client: OidcClientRecord) -> Result<(), AuthError> {
            self.inner.put_client(client).await
        }

        async fn client(&self, client_id: &str) -> Result<Option<OidcClientRecord>, AuthError> {
            self.inner.client(client_id).await
        }
    }

    fn remove_sqlite(path: &std::path::Path) {
        let _ = std::fs::remove_file(path);
        let mut wal = path.to_path_buf().into_os_string();
        wal.push("-wal");
        let _ = std::fs::remove_file(wal);
        let mut shm = path.to_path_buf().into_os_string();
        shm.push("-shm");
        let _ = std::fs::remove_file(shm);
    }

    #[tokio::test]
    async fn overlapping_lockout_failures_keep_the_winner_and_a_behind_clock_stays_locked() {
        let path = std::env::temp_dir().join(format!("aster-auth-lockout-{}.sqlite", Uuid::new_v4()));
        let inner = SqliteAuthStore::open(&path).await.expect("store");
        let second = SqliteAuthStore::open(&path)
            .await
            .expect("second connection");
        let key = "member:-:overlap@example.com";
        second
            .put_lockout(
                key,
                LockoutSnapshot {
                    failures: 4,
                    locked_until: 0,
                    version: 0,
                },
            )
            .await
            .expect("seed");
        let (release, hold) = oneshot::channel();
        let held = Arc::new(HeldStore::new(inner, hold));
        let subject_id = Uuid::new_v4();
        let directory = Arc::new(FixedDir {
            subject_id,
            identifier: "overlap@example.com".into(),
        });
        // Far ahead of the database clock and of any lock window this test stores.
        let ahead = Arc::new(ManualClock::new(2_100_000_000));
        let process_clock: Arc<dyn crate::contexts::proof::domain::Clock> = ahead;
        let stalled = AuthService::new(
            held.clone(),
            directory.clone(),
            process_clock.clone(),
            "https://auth.aster.test",
        );
        stalled
            .enroll_password(Plane::Member, subject_id, "correct-horse")
            .await
            .expect("enroll");

        let arrived = held.entered.notified();
        let stalled_login = tokio::spawn({
            let stalled = stalled;
            async move {
                stalled
                    .login(LoginCommand {
                        plane: Plane::Member,
                        identifier: "overlap@example.com".into(),
                        org_id: None,
                        password: "correct-horse".into(),

                        computer_id: None,
                    })
                    .await
            }
        });
        tokio::time::timeout(Duration::from_secs(60), arrived)
            .await
            .expect("stalled login reached the lockout write");

        let read = second.lockout(key).await.expect("read").expect("row");
        let db_now = second.database_unix_secs().await.expect("database clock");
        let locked_until = db_now.saturating_add(LOCKOUT_SECS);
        assert!(locked_until > db_now);
        assert!(2_100_000_000 > locked_until);
        let applied = second
            .put_lockout(
                key,
                LockoutSnapshot {
                    failures: read.failures.saturating_add(1).max(5),
                    locked_until,
                    version: read.version,
                },
            )
            .await
            .expect("winner write");
        assert_eq!(applied, LockoutWrite::Applied);
        let committed = second.lockout(key).await.expect("winner row").expect("row");
        assert_eq!(committed.locked_until, locked_until);
        assert!(committed.failures >= read.failures);

        let _ = release.send(());
        let lost_outcome = stalled_login
            .await
            .expect("stalled task")
            .expect("stalled login");
        assert!(
            !matches!(
                lost_outcome,
                LoginOutcome::Session(_) | LoginOutcome::Challenge(_)
            ),
            "lost lockout clear must not come back as a session or challenge: {lost_outcome:?}"
        );
        let writes = held.writes.lock().await.clone();
        assert!(
            writes.contains(&LockoutWrite::Lost),
            "lost compare was reported as applied: {writes:?}"
        );
        let kept = second.lockout(key).await.expect("kept row").expect("row");
        assert_eq!(kept.failures, committed.failures);
        assert_eq!(kept.locked_until, committed.locked_until);

        let probe = SqliteAuthStore::open(&path).await.expect("probe");
        let db_now = probe.database_unix_secs().await.expect("database clock");
        assert!(
            committed.locked_until > db_now,
            "database clock must still be inside the stored lock window"
        );
        let early = AuthService::new(
            Arc::new(second),
            directory.clone(),
            process_clock,
            "https://auth.aster.test",
        );
        let denied = early
            .login(LoginCommand {
                plane: Plane::Member,
                identifier: "overlap@example.com".into(),
                org_id: None,
                password: "correct-horse".into(),

                computer_id: None,
            })
            .await
            .expect("ahead clock");
        assert!(matches!(denied, LoginOutcome::Denied(AuthCode::Locked)));
        let after = probe.lockout(key).await.expect("row").expect("present");
        assert_eq!(after.failures, committed.failures);
        assert_eq!(after.locked_until, committed.locked_until);

        // A newer failure count with no lock window must survive a lost clear.
        let clear_key = "member:-:clear@example.com";
        probe
            .put_lockout(
                clear_key,
                LockoutSnapshot {
                    failures: 1,
                    locked_until: 0,
                    version: 0,
                },
            )
            .await
            .expect("seed clear key");
        let (release_clear, hold_clear) = oneshot::channel();
        held.arm(hold_clear).await;
        let arrived = held.entered.notified();
        let stale_clear = tokio::spawn({
            let stalled = AuthService::new(
                held.clone(),
                directory.clone(),
                Arc::new(ManualClock::new(2_100_000_000)),
                "https://auth.aster.test",
            );
            async move {
                stalled
                    .login(LoginCommand {
                        plane: Plane::Member,
                        identifier: "clear@example.com".into(),
                        org_id: None,
                        password: "correct-horse".into(),

                        computer_id: None,
                    })
                    .await
            }
        });
        tokio::time::timeout(Duration::from_secs(60), arrived)
            .await
            .expect("stale clear reached the lockout write");
        let read = probe.lockout(clear_key).await.expect("read").expect("row");
        let winner = probe
            .put_lockout(
                clear_key,
                LockoutSnapshot {
                    failures: read.failures.saturating_add(2),
                    locked_until: 0,
                    version: read.version,
                },
            )
            .await
            .expect("non-lock winner");
        assert_eq!(winner, LockoutWrite::Applied);
        let committed = probe
            .lockout(clear_key)
            .await
            .expect("winner row")
            .expect("row");
        assert_eq!(committed.locked_until, 0);
        assert!(committed.failures > read.failures);
        let _ = release_clear.send(());
        let lost_clear = stale_clear
            .await
            .expect("stale clear task")
            .expect("stale clear login");
        assert!(
            !matches!(
                lost_clear,
                LoginOutcome::Session(_) | LoginOutcome::Challenge(_)
            ),
            "lost clear of a non-lock row must not open a session: {lost_clear:?}"
        );
        let kept = probe
            .lockout(clear_key)
            .await
            .expect("kept row")
            .expect("row");
        assert_eq!(kept.failures, committed.failures);
        assert_eq!(kept.locked_until, 0);

        remove_sqlite(&path);
    }
}
