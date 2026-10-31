//! The shipped binary admits a person through the directory HTTP contract and
//! returns that subject on login.

use aster_auth::{DirectoryAdmitRequest, DirectoryDecision, DirectoryStatusRequest, Plane};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::post;
use axum::{Json, Router};
use uuid::Uuid;

#[derive(Clone)]
struct AdmitState {
    subject_id: Uuid,
    token: String,
}

#[tokio::test]
async fn binary_login_uses_the_directory() {
    let subject_id = Uuid::new_v4();
    let token = "internal-directory-token".to_owned();
    let admit_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let admit_addr = admit_listener.local_addr().unwrap();
    let state = AdmitState {
        subject_id,
        token: token.clone(),
    };
    tokio::spawn(async move {
        let app = Router::new()
            .route("/api/internal/auth/admit", post(admit))
            .route("/api/internal/auth/subjects/{id}/status", post(status))
            .with_state(state);
        axum::serve(admit_listener, app).await.unwrap();
    });

    let port = 17651u16;
    let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_aster-auth"))
        .env("ASTER_AUTH_LISTEN", format!("127.0.0.1:{port}"))
        .env("ASTER_AUTH_ISSUER", format!("https://127.0.0.1:{port}"))
        .env("ASTER_AUTH_DIRECTORY_URL", format!("http://{admit_addr}/api"))
        .env("ASTER_AUTH_INTERNAL_TOKEN", &token)
        .kill_on_drop(true)
        .spawn()
        .unwrap();

    let client = reqwest::Client::new();
    let base = format!("http://127.0.0.1:{port}");
    let mut logged_in = None;
    for _ in 0..80 {
        let enrolled = client
            .post(format!("{base}/v1/credentials"))
            .json(&serde_json::json!({
                "plane": "member",
                "subject_id": subject_id,
                "password": "directory-password",
            }))
            .send()
            .await;
        if let Ok(response) = &enrolled {
            if response.status() == StatusCode::UNAUTHORIZED {
                let with_token = client
                    .post(format!("{base}/v1/credentials"))
                    .bearer_auth(&token)
                    .json(&serde_json::json!({
                        "plane": "member",
                        "subject_id": subject_id,
                        "password": "directory-password",
                    }))
                    .send()
                    .await
                    .unwrap();
                assert!(with_token.status().is_success());
                let missing = client
                    .post(format!("{base}/v1/login"))
                    .json(&serde_json::json!({
                        "plane": "member",
                        "identifier": "person@example.com",
                        "password": "directory-password",
                    }))
                    .send()
                    .await
                    .unwrap();
                assert_eq!(missing.status(), 400);
                let missing_body: serde_json::Value = missing.json().await.unwrap();
                assert_eq!(missing_body["success"], false);
                assert!(missing_body["data"].get("token").is_none());
                let login = client
                    .post(format!("{base}/v1/login"))
                    .json(&serde_json::json!({
                        "plane": "member",
                        "identifier": "person@example.com",
                        "password": "directory-password",
                        "computer_id": "computer-directory-1",
                    }))
                    .send()
                    .await
                    .unwrap();
                assert_eq!(login.status(), 200);
                logged_in = Some(login.json::<serde_json::Value>().await.unwrap());
                break;
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    }
    let _ = child.kill().await;
    let body = logged_in.expect("binary login");
    assert_eq!(body["success"], true);
    assert_eq!(body["code"], 0);
    assert_eq!(body["message"], "OK");
    assert!(body["request_id"].is_string());
    assert!(body["timestamp"].is_string());
    assert_eq!(body["data"]["subject_id"], subject_id.to_string());
    assert_eq!(body["data"]["plane"], "member");
    assert!(body["data"]["token"]
        .as_str()
        .unwrap()
        .starts_with("aster_ms_"));
    println!("directory-login {}", body);
}

fn envelope(data: DirectoryDecision) -> serde_json::Value {
    serde_json::json!({
        "success": true,
        "code": 0,
        "message": "OK",
        "data": data,
        "request_id": Uuid::new_v4(),
        "timestamp": "2026-09-22T00:00:00Z"
    })
}

async fn admit(
    State(state): State<AdmitState>,
    headers: axum::http::HeaderMap,
    Json(body): Json<DirectoryAdmitRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    authorize(&state, &headers)?;
    if body.plane != Plane::Member.as_str() || body.identifier != "person@example.com" {
        return Ok(Json(envelope(DirectoryDecision {
            subject_id: None,
            org_id: None,
            active: false,
            console_login: false,
        })));
    }
    Ok(Json(envelope(DirectoryDecision {
        subject_id: Some(state.subject_id),
        org_id: None,
        active: true,
        console_login: false,
    })))
}

async fn status(
    State(state): State<AdmitState>,
    Path(_id): Path<Uuid>,
    headers: axum::http::HeaderMap,
    Json(_body): Json<DirectoryStatusRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    authorize(&state, &headers)?;
    Ok(Json(envelope(DirectoryDecision {
        subject_id: Some(state.subject_id),
        org_id: None,
        active: true,
        console_login: false,
    })))
}

fn authorize(state: &AdmitState, headers: &axum::http::HeaderMap) -> Result<(), StatusCode> {
    let presented = headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .unwrap_or("");
    if presented == state.token {
        Ok(())
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}
