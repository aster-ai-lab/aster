//! In-memory proof store for tests and a directory-less boot.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::contexts::proof::domain::repository::*;
use crate::contexts::proof::domain::rules::LockoutSnapshot;
use crate::contexts::proof::domain::{AuthError, Clock, Plane, SystemClock};

#[derive(Default)]
struct MemoryState {
    credentials: HashMap<(Plane, Uuid), CredentialRecord>,
    sessions: HashMap<String, SessionRecord>,
    lockouts: HashMap<String, LockoutSnapshot>,
    challenges: HashMap<Uuid, ChallengeRecord>,
    events: Vec<AuthEvent>,
    codes: HashMap<String, AuthCodeRecord>,
    used_codes: HashSet<String>,
    refreshes: HashMap<String, RefreshRecord>,
    used_refreshes: HashSet<String>,
    clients: HashMap<String, OidcClientRecord>,
}

#[derive(Clone, Default)]
pub struct MemoryAuthStore {
    inner: Arc<Mutex<MemoryState>>,
}

impl MemoryAuthStore {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl AuthStore for MemoryAuthStore {
    async fn put_credential(&self, credential: CredentialRecord) -> Result<(), AuthError> {
        let mut state = self.inner.lock().await;
        state
            .credentials
            .insert((credential.plane, credential.subject_id), credential);
        Ok(())
    }

    async fn credential(
        &self,
        plane: Plane,
        subject_id: Uuid,
    ) -> Result<Option<CredentialRecord>, AuthError> {
        Ok(self
            .inner
            .lock()
            .await
            .credentials
            .get(&(plane, subject_id))
            .cloned())
    }

    async fn put_session(&self, session: SessionRecord) -> Result<(), AuthError> {
        let mut state = self.inner.lock().await;
        state.sessions.insert(session.token_hash.clone(), session);
        Ok(())
    }

    async fn session_by_hash(&self, token_hash: &str) -> Result<Option<SessionRecord>, AuthError> {
        Ok(self.inner.lock().await.sessions.get(token_hash).cloned())
    }

    async fn delete_session_hash(&self, token_hash: &str) -> Result<(), AuthError> {
        self.inner.lock().await.sessions.remove(token_hash);
        Ok(())
    }

    async fn delete_subject_sessions(
        &self,
        plane: Plane,
        subject_id: Uuid,
    ) -> Result<(), AuthError> {
        let mut state = self.inner.lock().await;
        state
            .sessions
            .retain(|_, session| !(session.plane == plane && session.subject_id == subject_id));
        Ok(())
    }

    async fn delete_other_computer_sessions(
        &self,
        plane: Plane,
        subject_id: Uuid,
        computer_id: &str,
    ) -> Result<(), AuthError> {
        let mut state = self.inner.lock().await;
        state.sessions.retain(|_, session| {
            !(session.plane == plane
                && session.subject_id == subject_id
                && session.computer_id.as_deref() != Some(computer_id))
        });
        Ok(())
    }

    async fn lockout(&self, key: &str) -> Result<Option<LockoutSnapshot>, AuthError> {
        Ok(self.inner.lock().await.lockouts.get(key).copied())
    }

    async fn put_lockout(
        &self,
        key: &str,
        snapshot: LockoutSnapshot,
    ) -> Result<LockoutWrite, AuthError> {
        let mut state = self.inner.lock().await;
        match state.lockouts.get(key).copied() {
            None if snapshot.version == 0 => {
                state.lockouts.insert(
                    key.to_owned(),
                    LockoutSnapshot {
                        version: 1,
                        ..snapshot
                    },
                );
                Ok(LockoutWrite::Applied)
            }
            Some(current) if current.version == snapshot.version => {
                state.lockouts.insert(
                    key.to_owned(),
                    LockoutSnapshot {
                        version: current.version + 1,
                        ..snapshot
                    },
                );
                Ok(LockoutWrite::Applied)
            }
            _ => Ok(LockoutWrite::Lost),
        }
    }

    async fn database_unix_secs(&self) -> Result<i64, AuthError> {
        Ok(SystemClock.unix_secs())
    }

    async fn put_challenge(&self, challenge: ChallengeRecord) -> Result<(), AuthError> {
        self.inner
            .lock()
            .await
            .challenges
            .insert(challenge.id, challenge);
        Ok(())
    }

    async fn take_challenge(&self, id: Uuid) -> Result<Option<ChallengeRecord>, AuthError> {
        Ok(self.inner.lock().await.challenges.remove(&id))
    }

    async fn append_event(&self, event: AuthEvent) -> Result<(), AuthError> {
        self.inner.lock().await.events.push(event);
        Ok(())
    }

    async fn events(&self) -> Result<Vec<AuthEvent>, AuthError> {
        Ok(self.inner.lock().await.events.clone())
    }

    async fn put_code(&self, code: AuthCodeRecord) -> Result<(), AuthError> {
        let mut state = self.inner.lock().await;
        state.codes.insert(code.code_hash.clone(), code);
        Ok(())
    }

    async fn take_code(&self, code_hash: &str) -> Result<Option<AuthCodeRecord>, AuthError> {
        Ok(self.inner.lock().await.codes.remove(code_hash))
    }

    async fn mark_code_used(&self, code_hash: &str) -> Result<(), AuthError> {
        self.inner
            .lock()
            .await
            .used_codes
            .insert(code_hash.to_owned());
        Ok(())
    }

    async fn code_was_used(&self, code_hash: &str) -> Result<bool, AuthError> {
        Ok(self.inner.lock().await.used_codes.contains(code_hash))
    }

    async fn put_refresh(&self, refresh: RefreshRecord) -> Result<(), AuthError> {
        let mut state = self.inner.lock().await;
        state.refreshes.insert(refresh.token_hash.clone(), refresh);
        Ok(())
    }

    async fn take_refresh(&self, token_hash: &str) -> Result<Option<RefreshRecord>, AuthError> {
        Ok(self.inner.lock().await.refreshes.remove(token_hash))
    }

    async fn mark_refresh_used(&self, token_hash: &str) -> Result<(), AuthError> {
        self.inner
            .lock()
            .await
            .used_refreshes
            .insert(token_hash.to_owned());
        Ok(())
    }

    async fn refresh_was_used(&self, token_hash: &str) -> Result<bool, AuthError> {
        Ok(self.inner.lock().await.used_refreshes.contains(token_hash))
    }

    async fn put_client(&self, client: OidcClientRecord) -> Result<(), AuthError> {
        self.inner
            .lock()
            .await
            .clients
            .insert(client.client_id.clone(), client);
        Ok(())
    }

    async fn client(&self, client_id: &str) -> Result<Option<OidcClientRecord>, AuthError> {
        Ok(self.inner.lock().await.clients.get(client_id).cloned())
    }
}
