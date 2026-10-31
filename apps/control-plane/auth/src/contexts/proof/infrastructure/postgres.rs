//! Postgres proof store through aster-orm. Schema changes go through Migrator.

use aster_orm::{sql_query, Database, Migration, Migrator, PostgresDialect, PostgresExecutor, Value};
use async_trait::async_trait;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde::Deserialize;
use uuid::Uuid;

use crate::contexts::proof::domain::repository::*;
use crate::contexts::proof::domain::rules::LockoutSnapshot;
use crate::contexts::proof::domain::{AuthError, Plane};

pub struct PostgresAuthStore {
    executor: PostgresExecutor,
}

impl PostgresAuthStore {
    pub async fn connect(database_url: &str) -> Result<Self, AuthError> {
        let executor = PostgresExecutor::connect_no_tls(database_url, 8)
            .map_err(|err| AuthError::storage(err.to_string()))?;
        Migrator::new(executor.clone())
            .run([
                Migration::new(
                    "001",
                    "auth proof",
                    include_str!("../../../../migrations/001_init.sql"),
                ),
                Migration::new(
                    "002",
                    "lockout version",
                    include_str!("../../../../migrations/002_lockout_version.sql"),
                ),
                Migration::new(
                    "003",
                    "session computer",
                    include_str!("../../../../migrations/003_session_computer.sql"),
                ),
                Migration::new(
                    "004",
                    "api keys",
                    include_str!("../../../../migrations/004_api_keys.sql"),
                ),
                Migration::new(
                    "005",
                    "drop api keys",
                    include_str!("../../../../migrations/005_drop_api_keys.sql"),
                ),
            ])
            .await
            .map_err(|err| AuthError::storage(err.to_string()))?;
        Ok(Self { executor })
    }

    fn db(&self) -> Database<PostgresDialect, PostgresExecutor> {
        Database::new(PostgresDialect, self.executor.clone())
    }
}

#[async_trait]
impl AuthStore for PostgresAuthStore {
    async fn put_credential(&self, credential: CredentialRecord) -> Result<(), AuthError> {
        self.db()
            .execute(
                sql_query::<()>(
                    "insert into auth_credentials (plane, subject_id, password_hash, totp_secret) values (",
                )
                .bind(credential.plane.as_str())
                .append(", ")
                .bind(credential.subject_id)
                .append(", ")
                .bind(credential.password_hash)
                .append(", ")
                .bind(opt_bytes(credential.totp_secret))
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
        let row = fetch_json::<CredentialRow>(
            &self.db(),
            sql_query::<String>(
                "select row_to_json(t)::text from (select password_hash, case when totp_secret is null then null else encode(totp_secret, 'base64') end as totp_secret from auth_credentials where plane = ",
            )
            .bind(plane.as_str())
            .append(" and subject_id = ")
            .bind(subject_id)
            .append(") t"),
        )
        .await?;
        let Some(row) = row else {
            return Ok(None);
        };
        let totp_secret = match row.totp_secret {
            Some(secret) => Some(
                STANDARD
                    .decode(secret)
                    .map_err(|err| AuthError::storage(err.to_string()))?,
            ),
            None => None,
        };
        Ok(Some(CredentialRecord {
            plane,
            subject_id,
            password_hash: row.password_hash,
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
                .bind(session.subject_id)
                .append(", ")
                .bind(opt_uuid(session.org_id))
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
        let row = fetch_json::<SessionRow>(
            &self.db(),
            sql_query::<String>(
                "select row_to_json(t)::text from (select plane, subject_id, org_id, auth_time, amr, expires_at, computer_id from auth_sessions where token_hash = ",
            )
            .bind(token_hash)
            .append(") t"),
        )
        .await?;
        Ok(row.map(|row| session_from_row(token_hash, row)))
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
                    .bind(subject_id),
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
                    .bind(subject_id)
                    .append(" and (computer_id is null or computer_id <> ")
                    .bind(computer_id)
                    .append(")"),
            )
            .await
            .map_err(storage)?;
        Ok(())
    }

    async fn lockout(&self, key: &str) -> Result<Option<LockoutSnapshot>, AuthError> {
        let row = fetch_json::<LockoutRow>(
            &self.db(),
            sql_query::<String>(
                "select row_to_json(t)::text from (select failures, locked_until, version from auth_lockouts where lock_key = ",
            )
            .bind(key)
            .append(") t"),
        )
        .await?;
        Ok(row.map(|row| LockoutSnapshot {
            failures: row.failures.max(0) as u32,
            locked_until: row.locked_until,
            version: row.version.max(0) as u64,
        }))
    }

    async fn put_lockout(
        &self,
        key: &str,
        snapshot: LockoutSnapshot,
    ) -> Result<LockoutWrite, AuthError> {
        // A lost compare-and-swap leaves the first row unchanged and is not success.
        let written = if snapshot.version == 0 {
            self.db()
                .fetch_optional_as(
                    sql_query::<i64>(
                        "insert into auth_lockouts (lock_key, failures, locked_until, version) values (",
                    )
                    .bind(key)
                    .append(", ")
                    .bind(snapshot.failures as i32)
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
                        .bind(snapshot.failures as i32)
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
                "select floor(extract(epoch from clock_timestamp()))::bigint",
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
                .bind(challenge.id)
                .append(", ")
                .bind(challenge.plane.as_str())
                .append(", ")
                .bind(challenge.subject_id)
                .append(", ")
                .bind(opt_uuid(challenge.org_id))
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
        let row = fetch_json::<ChallengeRow>(
            &self.db(),
            sql_query::<String>(
                "select row_to_json(t)::text from (delete from auth_challenges where id = ",
            )
            .bind(id)
            .append(" returning plane, subject_id, org_id, auth_time, expires_at, computer_id) t"),
        )
        .await?;
        Ok(row.map(|row| ChallengeRecord {
            id,
            plane: Plane::parse(&row.plane).unwrap_or(Plane::Member),
            subject_id: row.subject_id,
            org_id: row.org_id,
            auth_time: row.auth_time,
            expires_at: row.expires_at,
            computer_id: row.computer_id,
        }))
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
                .bind(opt_uuid(event.subject_id))
                .append(", ")
                .bind(event.at)
                .append(")"),
            )
            .await
            .map_err(storage)?;
        Ok(())
    }

    async fn events(&self) -> Result<Vec<AuthEvent>, AuthError> {
        let raw = self
            .db()
            .fetch_one_as(sql_query::<String>(
                "select coalesce(json_agg(row_to_json(t) order by t.id), '[]'::json)::text from (select id, kind, plane, subject_id, at_epoch from auth_events) t",
            ))
            .await
            .map_err(storage)?;
        let rows: Vec<EventRow> =
            serde_json::from_str(&raw).map_err(|err| AuthError::storage(err.to_string()))?;
        Ok(rows
            .into_iter()
            .map(|row| AuthEvent {
                kind: row.kind,
                plane: Plane::parse(&row.plane).unwrap_or(Plane::Member),
                subject_id: row.subject_id,
                at: row.at_epoch,
            })
            .collect())
    }

    async fn put_code(&self, code: AuthCodeRecord) -> Result<(), AuthError> {
        let payload = super::session_json::encode_session_value(&code.session)?;
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
                .bind(payload)
                .append(", ")
                .bind(code.expires_at)
                .append(")"),
            )
            .await
            .map_err(storage)?;
        Ok(())
    }

    async fn take_code(&self, code_hash: &str) -> Result<Option<AuthCodeRecord>, AuthError> {
        let row = fetch_json::<CodeRow>(
            &self.db(),
            sql_query::<String>(
                "select row_to_json(t)::text from (delete from auth_codes where code_hash = ",
            )
            .bind(code_hash)
            .append(
                " returning client_id, redirect_uri, code_challenge, nonce, session_json, expires_at) t",
            ),
        )
        .await?;
        Ok(match row {
            Some(row) => Some(AuthCodeRecord {
                code_hash: code_hash.to_owned(),
                client_id: row.client_id,
                redirect_uri: row.redirect_uri,
                code_challenge: row.code_challenge,
                nonce: row.nonce,
                session: super::session_json::decode_session_value(row.session_json)?,
                expires_at: row.expires_at,
            }),
            None => None,
        })
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
        exists(
            &self.db(),
            sql_query::<String>(
                "select case when exists (select 1 from auth_used_codes where code_hash = ",
            )
            .bind(code_hash)
            .append(") then '1' else '0' end"),
        )
        .await
    }

    async fn put_refresh(&self, refresh: RefreshRecord) -> Result<(), AuthError> {
        let payload = super::session_json::encode_session_value(&refresh.session)?;
        self.db()
            .execute(
                sql_query::<()>(
                    "insert into auth_refreshes (token_hash, client_id, session_json, expires_at) values (",
                )
                .bind(refresh.token_hash)
                .append(", ")
                .bind(refresh.client_id)
                .append(", ")
                .bind(payload)
                .append(", ")
                .bind(refresh.expires_at)
                .append(")"),
            )
            .await
            .map_err(storage)?;
        Ok(())
    }

    async fn take_refresh(&self, token_hash: &str) -> Result<Option<RefreshRecord>, AuthError> {
        let row = fetch_json::<RefreshRow>(
            &self.db(),
            sql_query::<String>(
                "select row_to_json(t)::text from (delete from auth_refreshes where token_hash = ",
            )
            .bind(token_hash)
            .append(" returning client_id, session_json, expires_at) t"),
        )
        .await?;
        Ok(match row {
            Some(row) => Some(RefreshRecord {
                token_hash: token_hash.to_owned(),
                client_id: row.client_id,
                session: super::session_json::decode_session_value(row.session_json)?,
                expires_at: row.expires_at,
            }),
            None => None,
        })
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
        exists(
            &self.db(),
            sql_query::<String>(
                "select case when exists (select 1 from auth_used_refreshes where token_hash = ",
            )
            .bind(token_hash)
            .append(") then '1' else '0' end"),
        )
        .await
    }

    async fn put_client(&self, client: OidcClientRecord) -> Result<(), AuthError> {
        self.db()
            .execute(
                sql_query::<()>(
                    "insert into auth_oidc_clients (client_id, secret_hash, redirect_uris) values (",
                )
                .bind(client.client_id)
                .append(", ")
                .bind(client.secret_hash)
                .append(", ")
                .bind(aster_orm::SqlArray(client.redirect_uris))
                .append(
                    ") on conflict (client_id) do update set secret_hash = excluded.secret_hash, redirect_uris = excluded.redirect_uris",
                ),
            )
            .await
            .map_err(storage)?;
        Ok(())
    }

    async fn client(&self, client_id: &str) -> Result<Option<OidcClientRecord>, AuthError> {
        let row = fetch_json::<ClientRow>(
            &self.db(),
            sql_query::<String>(
                "select row_to_json(t)::text from (select secret_hash, redirect_uris from auth_oidc_clients where client_id = ",
            )
            .bind(client_id)
            .append(") t"),
        )
        .await?;
        Ok(row.map(|row| OidcClientRecord {
            client_id: client_id.to_owned(),
            secret_hash: row.secret_hash,
            redirect_uris: row.redirect_uris,
        }))
    }
}

#[derive(Deserialize)]
struct CredentialRow {
    password_hash: String,
    totp_secret: Option<String>,
}

#[derive(Deserialize)]
struct SessionRow {
    plane: String,
    subject_id: Uuid,
    org_id: Option<Uuid>,
    auth_time: i64,
    amr: String,
    expires_at: i64,
    #[serde(default)]
    computer_id: Option<String>,
}

#[derive(Deserialize)]
struct LockoutRow {
    failures: i32,
    locked_until: i64,
    version: i64,
}

#[derive(Deserialize)]
struct ChallengeRow {
    plane: String,
    subject_id: Uuid,
    org_id: Option<Uuid>,
    auth_time: i64,
    expires_at: i64,
    #[serde(default)]
    computer_id: Option<String>,
}

#[derive(Deserialize)]
struct EventRow {
    kind: String,
    plane: String,
    subject_id: Option<Uuid>,
    at_epoch: i64,
}

#[derive(Deserialize)]
struct CodeRow {
    client_id: String,
    redirect_uri: String,
    code_challenge: String,
    nonce: String,
    session_json: serde_json::Value,
    expires_at: i64,
}

#[derive(Deserialize)]
struct RefreshRow {
    client_id: String,
    session_json: serde_json::Value,
    expires_at: i64,
}

#[derive(Deserialize)]
struct ClientRow {
    secret_hash: String,
    redirect_uris: Vec<String>,
}

fn session_from_row(token_hash: &str, row: SessionRow) -> SessionRecord {
    SessionRecord {
        token_hash: token_hash.to_owned(),
        plane: Plane::parse(&row.plane).unwrap_or(Plane::Member),
        subject_id: row.subject_id,
        org_id: row.org_id,
        auth_time: row.auth_time,
        amr: if row.amr.is_empty() {
            Vec::new()
        } else {
            row.amr.split(',').map(str::to_owned).collect()
        },
        expires_at: row.expires_at,
        computer_id: row.computer_id,
    }
}

fn opt_text(value: Option<&str>) -> Value {
    match value {
        Some(text) => Value::String(text.to_owned()),
        None => Value::Null,
    }
}

fn opt_uuid(value: Option<Uuid>) -> Value {
    value.map(Value::from).unwrap_or(Value::Null)
}

fn opt_bytes(value: Option<Vec<u8>>) -> Value {
    value.map(Value::from).unwrap_or(Value::Null)
}

fn storage(err: impl ToString) -> AuthError {
    AuthError::storage(err.to_string())
}

async fn fetch_json<T: for<'de> Deserialize<'de>>(
    db: &Database<PostgresDialect, PostgresExecutor>,
    query: aster_orm::SqlQuery<String>,
) -> Result<Option<T>, AuthError> {
    let raw = db.fetch_optional_as(query).await.map_err(storage)?;
    raw.map(|raw| serde_json::from_str(&raw).map_err(|err| AuthError::storage(err.to_string())))
        .transpose()
}

async fn exists(
    db: &Database<PostgresDialect, PostgresExecutor>,
    query: aster_orm::SqlQuery<String>,
) -> Result<bool, AuthError> {
    let raw = db.fetch_one_as(query).await.map_err(storage)?;
    Ok(raw == "1")
}
