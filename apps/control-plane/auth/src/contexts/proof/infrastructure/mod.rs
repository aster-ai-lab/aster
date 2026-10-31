pub mod directory;
pub mod memory;
pub mod postgres;
mod session_json;
pub mod sqlite;

#[cfg(test)]
mod store_campaign {
    use std::sync::Arc;

    use uuid::Uuid;

    use super::memory::MemoryAuthStore;
    use super::postgres::PostgresAuthStore;
    use crate::contexts::proof::domain::repository::{AuthStore, CredentialRecord};
    use crate::contexts::proof::domain::Plane;

    async fn prove_credential_roundtrip(store: Arc<dyn AuthStore>) {
        let subject_id = Uuid::new_v4();
        store
            .put_credential(CredentialRecord {
                plane: Plane::User,
                subject_id,
                password_hash: "campaign-hash".into(),
                totp_secret: None,
            })
            .await
            .expect("put credential");
        let stored = store
            .credential(Plane::User, subject_id)
            .await
            .expect("read credential")
            .expect("row");
        assert_eq!(stored.password_hash, "campaign-hash");
        assert!(stored.totp_secret.is_none());
    }

    #[tokio::test]
    async fn credential_roundtrip_matches_on_memory_and_postgres() {
        prove_credential_roundtrip(Arc::new(MemoryAuthStore::new())).await;
        let Ok(url) = std::env::var("ASTER_AUTH_TEST_POSTGRES_URL") else {
            return;
        };
        if url.is_empty() {
            return;
        }
        let store = PostgresAuthStore::connect(&url)
            .await
            .expect("postgres auth store");
        prove_credential_roundtrip(Arc::new(store)).await;
    }
}
