//! HTTP process adapter. Routes are aster-boot controllers; axum only serves the adapter.

mod controller;

use std::sync::Arc;

use aster_boot::{AxumAdapter, BootApplication, HttpAdapter};
use subtle::ConstantTimeEq;

use super::super::ProofModule;
use crate::contexts::proof::application::service::AuthService;

pub use controller::ProofController;

#[derive(Clone)]
pub struct InternalGuard {
    token: Option<String>,
}

impl InternalGuard {
    pub fn closed() -> Self {
        Self { token: None }
    }

    pub fn from_secret(secret: impl Into<String>) -> Self {
        let secret = secret.into();
        Self {
            token: (!secret.is_empty()).then_some(secret),
        }
    }

    pub(crate) fn authorize(&self, authorization: Option<&str>) -> bool {
        let Some(expected) = &self.token else {
            return false;
        };
        let Some(presented) = authorization
            .and_then(|value| {
                value
                    .trim()
                    .strip_prefix("Bearer ")
                    .or_else(|| value.trim().strip_prefix("bearer "))
            })
            .map(str::trim)
        else {
            return false;
        };
        if presented.is_empty() || presented.len() != expected.len() {
            return false;
        }
        bool::from(presented.as_bytes().ct_eq(expected.as_bytes()))
    }
}

#[derive(Clone)]
pub struct AppState {
    pub auth: Arc<AuthService>,
    pub internal: InternalGuard,
}

pub fn proof_application(state: AppState) -> aster_boot::Result<BootApplication> {
    BootApplication::builder()
        .import(ProofModule {
            auth: state.auth,
            internal: state.internal,
        })
        .build()
}

pub fn router(state: AppState) -> aster_boot::Result<axum::Router> {
    AxumAdapter::new().build(proof_application(state)?)
}

pub async fn serve(
    listener: tokio::net::TcpListener,
    auth: Arc<AuthService>,
) -> std::io::Result<()> {
    serve_with(
        listener,
        AppState {
            auth,
            internal: InternalGuard::closed(),
        },
    )
    .await
}

pub async fn serve_with(listener: tokio::net::TcpListener, state: AppState) -> std::io::Result<()> {
    let router = router(state).map_err(|err| std::io::Error::other(err.to_string()))?;
    axum::serve(listener, router).await
}

/// Serve the same aster-boot router over HTTPS so a relying party that refuses
/// plain HTTP can complete discovery and the token endpoint.
pub async fn serve_https(
    listener: tokio::net::TcpListener,
    cert_pem: &str,
    key_pem: &str,
    auth: Arc<AuthService>,
) -> std::io::Result<()> {
    use hyper_util::rt::{TokioExecutor, TokioIo};
    use hyper_util::server::conn::auto::Builder;
    use rustls::ServerConfig;
    use tokio_rustls::TlsAcceptor;
    use tower::ServiceExt;

    let _ = rustls::crypto::ring::default_provider().install_default();
    let certs = rustls_pemfile::certs(&mut std::io::Cursor::new(cert_pem.as_bytes()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?;
    let key = rustls_pemfile::private_key(&mut std::io::Cursor::new(key_pem.as_bytes()))
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?
        .ok_or_else(|| std::io::Error::other("TLS private key is missing"))?;
    let config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|err| std::io::Error::other(err.to_string()))?;
    let acceptor = TlsAcceptor::from(Arc::new(config));
    let app = router(AppState {
        auth,
        internal: InternalGuard::closed(),
    })
    .map_err(|err| std::io::Error::other(err.to_string()))?;
    loop {
        let (tcp, _) = listener.accept().await?;
        let acceptor = acceptor.clone();
        let app = app.clone();
        tokio::spawn(async move {
            let Ok(tls) = acceptor.accept(tcp).await else {
                return;
            };
            let io = TokioIo::new(tls);
            let service = hyper::service::service_fn(
                move |request: hyper::Request<hyper::body::Incoming>| {
                    let app = app.clone();
                    async move {
                        let (parts, incoming) = request.into_parts();
                        let body = axum::body::Body::new(http_body_util::Limited::new(
                            incoming,
                            1024 * 1024,
                        ));
                        let request = http::Request::from_parts(parts, body);
                        app.oneshot(request)
                            .await
                            .map_err(|err| std::io::Error::other(err.to_string()))
                    }
                },
            );
            let _ = Builder::new(TokioExecutor::new())
                .serve_connection(io, service)
                .await;
        });
    }
}

#[cfg(test)]
mod tests {
    use super::serve;
    use crate::contexts::proof::domain::rules::{claims_within_allow_list, pkce_s256};
    use crate::Plane;

    fn urlencoding(value: &str) -> String {
        value.replace(':', "%3A").replace('/', "%2F")
    }
    use crate::contexts::proof::application::service::AuthService;
    use crate::contexts::proof::domain::{
        Admission, AdmitQuery, DirectoryAdmission, ManualClock, SubjectStatus,
    };
    use crate::contexts::proof::infrastructure::memory::MemoryAuthStore;
    use crate::OidcClient;
    use serde_json::{json, Value};
    use std::sync::Arc;
    use std::sync::Mutex;
    use uuid::Uuid;

    struct Dir {
        people: Mutex<Vec<(String, Uuid, bool, bool, Option<Uuid>)>>,
    }

    #[async_trait::async_trait]
    impl DirectoryAdmission for Dir {
        async fn admit(&self, query: AdmitQuery) -> Result<Admission, crate::AuthError> {
            let people = self.people.lock().expect("dir");
            let found = people.iter().find(|(identifier, _, _, _, org)| {
                identifier == &query.identifier
                    && (query.plane != Plane::User || *org == query.org_id)
            });
            Ok(match found {
                None => Admission::unknown(),
                Some((_, subject, active, console, org)) => Admission {
                    subject_id: Some(*subject),
                    org_id: *org,
                    active: *active,
                    console_login: *console,
                },
            })
        }

        async fn subject_status(
            &self,
            _plane: Plane,
            subject_id: Uuid,
        ) -> Result<SubjectStatus, crate::AuthError> {
            let people = self.people.lock().expect("dir");
            let found = people.iter().find(|(_, id, _, _, _)| *id == subject_id);
            Ok(match found {
                None => SubjectStatus {
                    active: false,
                    console_login: false,
                    org_id: None,
                },
                Some((_, _, active, console, org)) => SubjectStatus {
                    active: *active,
                    console_login: *console,
                    org_id: *org,
                },
            })
        }
    }

    async fn spawn(auth: Arc<AuthService>) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            serve(listener, auth).await.unwrap();
        });
        format!("http://{address}")
    }

    #[tokio::test]
    async fn oidc_and_forward_auth_honor_the_session() {
        let subject = Uuid::new_v4();
        let org = Uuid::new_v4();
        let dir = Arc::new(Dir {
            people: Mutex::new(vec![(
                "admin@example.com".into(),
                subject,
                true,
                true,
                Some(org),
            )]),
        });
        let clock = Arc::new(ManualClock::new(1_700_000_000));
        let auth = Arc::new(AuthService::new(
            Arc::new(MemoryAuthStore::new()),
            dir,
            clock,
            "https://auth.aster.test",
        ));
        auth.enroll_password(Plane::User, subject, "correct-horse")
            .await
            .unwrap();
        auth.register_oidc_client(OidcClient {
            client_id: "console".into(),
            client_secret: "client-secret".into(),
            redirect_uris: vec!["https://console.example/callback".into()],
        })
        .await
        .unwrap();
        let base = spawn(Arc::clone(&auth)).await;
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap();

        let spoofed = client
            .get(format!("{base}/auth/verify"))
            .header("x-aster-subject", subject.to_string())
            .header("x-aster-plane", "user")
            .header("x-aster-org", org.to_string())
            .send()
            .await
            .unwrap();
        assert_eq!(spoofed.status(), 401);
        assert!(spoofed.headers().get("x-aster-subject").is_none());
        let spoofed_request = spoofed
            .headers()
            .get("x-request-id")
            .unwrap()
            .to_str()
            .unwrap()
            .to_owned();
        let spoofed_body: Value = spoofed.json().await.unwrap();
        assert_eq!(spoofed_body["success"], false);
        assert_eq!(spoofed_body["code"], 40100);
        assert_eq!(spoofed_body["message"], "revoked");
        assert!(spoofed_body["data"].is_null());
        assert_eq!(spoofed_body["request_id"], spoofed_request);

        let request_id = Uuid::new_v4();
        let login_response = client
            .post(format!("{base}/v1/login"))
            .header("x-request-id", request_id.to_string())
            .json(&json!({
                "plane": "user",
                "identifier": "admin@example.com",
                "org_id": org,
                "password": "correct-horse",
            }))
            .send()
            .await
            .unwrap();
        assert_eq!(login_response.status(), 200);
        assert_eq!(
            login_response
                .headers()
                .get("x-request-id")
                .unwrap()
                .to_str()
                .unwrap(),
            request_id.to_string()
        );
        let login: Value = login_response.json().await.unwrap();
        assert_eq!(login["success"], true);
        assert_eq!(login["code"], 0);
        assert_eq!(login["message"], "OK");
        assert_eq!(login["request_id"], request_id.to_string());
        assert!(login["timestamp"].as_str().unwrap().ends_with('Z'));
        assert_eq!(login["data"]["subject_id"], subject.to_string());
        assert_eq!(login["data"]["plane"], "user");
        let token = login["data"]["token"].as_str().unwrap().to_owned();

        let verified = client
            .get(format!("{base}/auth/verify"))
            .header("authorization", format!("Bearer {token}"))
            .header("x-aster-subject", "00000000-0000-0000-0000-000000000000")
            .send()
            .await
            .unwrap();
        assert_eq!(verified.status(), 200);
        assert_eq!(
            verified
                .headers()
                .get("x-aster-subject")
                .unwrap()
                .to_str()
                .unwrap(),
            subject.to_string()
        );
        assert_eq!(
            verified
                .headers()
                .get("x-aster-plane")
                .unwrap()
                .to_str()
                .unwrap(),
            "user"
        );
        assert_eq!(
            verified
                .headers()
                .get("x-aster-org")
                .unwrap()
                .to_str()
                .unwrap(),
            org.to_string()
        );

        let verifier = "abcdefghijklmnopqrstuvwxyz0123456789ABCDEFG";
        let challenge = pkce_s256(verifier);
        let authorized = client
            .get(format!("{base}/oauth/authorize"))
            .header("cookie", format!("aster_us={token}"))
            .query(&[
                ("response_type", "code"),
                ("client_id", "console"),
                ("redirect_uri", "https://console.example/callback"),
                ("code_challenge", challenge.as_str()),
                ("code_challenge_method", "S256"),
                ("nonce", "nonce-1"),
                ("state", "state-1"),
            ])
            .send()
            .await
            .unwrap();
        assert_eq!(authorized.status(), 302);
        let location = authorized
            .headers()
            .get("location")
            .unwrap()
            .to_str()
            .unwrap()
            .to_owned();
        let redirected = url::Url::parse(&location).unwrap();
        assert!(redirected
            .as_str()
            .starts_with("https://console.example/callback?"));
        let redirected_query: std::collections::HashMap<String, String> =
            redirected.query_pairs().into_owned().collect();
        assert_eq!(redirected_query["state"], "state-1");
        let code = redirected_query["code"].clone();
        let tokens: Value = client
            .post(format!("{base}/oauth/token"))
            .header("content-type", "application/x-www-form-urlencoded")
            .body(format!(
                "grant_type=authorization_code&code={code}&redirect_uri={}&code_verifier={verifier}&client_id=console&client_secret=client-secret",
                urlencoding("https://console.example/callback")
            ))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        let id_token = tokens["id_token"].as_str().unwrap();
        let claims = crate::decode_claims(id_token).unwrap();
        let keys = claims.keys().cloned().collect::<Vec<_>>();
        assert!(claims_within_allow_list(&keys));
        assert_eq!(claims["sub"], subject.to_string());
        assert_eq!(claims["plane"], "user");
        assert_eq!(claims["org_id"], org.to_string());
        assert!(claims.get("roles").is_none());
        assert!(claims.get("grant").is_none());
        assert!(tokens["expires_in"].as_i64().unwrap() <= 300);
        let refresh = tokens["refresh_token"].as_str().unwrap().to_owned();

        let replay = client
            .post(format!("{base}/oauth/token"))
            .header("content-type", "application/x-www-form-urlencoded")
            .body(format!(
                "grant_type=authorization_code&code={code}&redirect_uri={}&code_verifier={verifier}&client_id=console&client_secret=client-secret",
                urlencoding("https://console.example/callback")
            ))
            .send()
            .await
            .unwrap();
        assert_eq!(replay.status(), 401);
        let replay_body: Value = replay.json().await.unwrap();
        assert_eq!(replay_body["error"], "code_replay");

        let rotated: Value = client
            .post(format!("{base}/oauth/token"))
            .header("content-type", "application/x-www-form-urlencoded")
            .body(format!(
                "grant_type=refresh_token&refresh_token={refresh}&client_id=console&client_secret=client-secret"
            ))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert!(rotated["refresh_token"].as_str().unwrap() != refresh);
        let stale = client
            .post(format!("{base}/oauth/token"))
            .header("content-type", "application/x-www-form-urlencoded")
            .body(format!(
                "grant_type=refresh_token&refresh_token={refresh}&client_id=console&client_secret=client-secret"
            ))
            .send()
            .await
            .unwrap();
        assert_eq!(stale.status(), 401);
        let stale_body: Value = stale.json().await.unwrap();
        assert_eq!(stale_body["error"], "refresh_replay");
        assert_eq!(auth.resource_grant_count(), 0);
    }
}
