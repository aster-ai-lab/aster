#![cfg(test)]

use std::{collections::BTreeMap, time::Duration};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::oneshot,
    task::JoinHandle,
};

use super::*;
use crate::{PhoneNumber, SecretReference};

const PROFILE: &str = include_str!("../../../examples/smsbao.acl");
struct FakeSecrets {
    credential: &'static str,
}
#[async_trait]
impl SmsCredentialResolver for FakeSecrets {
    async fn resolve(&self, reference: &SecretReference) -> Result<SecretValue, SmsError> {
        SecretValue::new(
            if reference.as_str() == "smsbao-username@1" {
                "fixture-user"
            } else {
                self.credential
            }
            .into(),
        )
    }
}

async fn provider(
    endpoint: &str,
    format: &str,
    auth: &str,
    credential: &'static str,
) -> SmsbaoProvider {
    let profile = PROFILE
        .replace(
            "response_format = \"txt\"",
            &format!("response_format = \"{format}\""),
        )
        .replace(
            "auth_method = \"password\"",
            &format!("auth_method = \"{auth}\""),
        );
    let config = SmsbaoConfig::from_acl(&profile).unwrap();
    let mut provider = SmsbaoProvider::from_config(config, &FakeSecrets { credential })
        .await
        .unwrap();
    provider.client = build_client(provider.config.timeout, false).unwrap();
    provider.send_url = format!("{endpoint}/sms");
    provider.balance_url = format!("{endpoint}/query");
    provider
}
fn plan() -> ProviderSendPlan {
    ProviderSendPlan {
        recipient: PhoneNumber::parse("+8613800138000").unwrap(),
        content: SmsContent::text("验证码：654321 & + ?".into()),
        binding: PreparedBinding {
            profile_id: "smsbao-cn".into(),
            profile_revision: 1,
            attempt_id: "attempt-1".into(),
        },
    }
}
fn context() -> SubmitContext {
    SubmitContext {
        deadline: Instant::now() + Duration::from_secs(2),
    }
}

/// Exercise actual reqwest serialization and transport, not only response helpers.
async fn fixture(
    status: u16,
    body: &[u8],
    delay: Duration,
    truncate: bool,
) -> (String, oneshot::Receiver<String>, JoinHandle<usize>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let (tx, rx) = oneshot::channel();
    let body = body.to_vec();
    let redirect = format!("{endpoint}/redirected");
    let task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut request = Vec::new();
        while !request.ends_with(b"\r\n\r\n") {
            let mut byte = [0; 1];
            if stream.read(&mut byte).await.unwrap() == 0 {
                break;
            }
            request.extend_from_slice(&byte);
            assert!(request.len() < 32_768);
        }
        tx.send(String::from_utf8(request).unwrap()).ok();
        tokio::time::sleep(delay).await;
        let length = body.len() + usize::from(truncate) * 10;
        let headers = format!("HTTP/1.1 {status} Fixture\r\nContent-Length: {length}\r\nLocation: {redirect}\r\nConnection: close\r\n\r\n");
        stream.write_all(headers.as_bytes()).await.ok();
        stream.write_all(&body).await.ok();
        stream.shutdown().await.ok();
        // Redirect/retry must not create another request even for a GET side effect.
        match tokio::time::timeout(Duration::from_millis(100), listener.accept()).await {
            Ok(Ok(_)) => 2,
            _ => 1,
        }
    });
    (endpoint, rx, task)
}
fn fields(request: &str) -> BTreeMap<String, String> {
    let target = request.split_whitespace().nth(1).unwrap();
    let (_, query) = target.split_once('?').unwrap();
    url::form_urlencoded::parse(query.as_bytes())
        .into_owned()
        .collect()
}

async fn submission_contract(provider: &dyn SmsProvider) -> SubmissionOutcome {
    assert_eq!(provider.descriptor().capabilities, &[SmsCapability::Text]);
    let prepared = provider.prepare(plan()).await.unwrap();
    provider.submit(prepared, context()).await
}

#[tokio::test]
async fn trait_contract_sends_encoded_single_recipient_and_password_md5_once() {
    let (endpoint, request, task) = fixture(
        200,
        br#"{"code":0,"data":{"taskId":"task-1"}}"#,
        Duration::ZERO,
        false,
    )
    .await;
    let provider = provider(&endpoint, "json", "password", "fixture-password").await;
    let outcome = submission_contract(&provider).await;
    assert_eq!(
        outcome,
        SubmissionOutcome::Accepted {
            provider_message_id: Some("task-1".into())
        }
    );
    let fields = fields(&request.await.unwrap());
    assert_eq!(fields.len(), 5);
    assert_eq!(fields["u"], "fixture-user");
    assert_eq!(
        fields["p"],
        format!("{:x}", Md5::digest(b"fixture-password"))
    );
    assert_eq!(fields["m"], "13800138000");
    assert_eq!(fields["c"], "【Aster】验证码：654321 & + ?");
    assert_eq!(fields["f"], "json");
    let debug = format!("{provider:?}");
    assert!(!debug.contains("fixture-password"));
    assert!(!debug.contains(&fields["p"]));
    assert_eq!(task.await.unwrap(), 1);
}

#[tokio::test]
async fn api_key_is_not_hashed_and_txt_codes_use_txt_semantics() {
    let (endpoint, request, task) = fixture(200, b"30", Duration::ZERO, false).await;
    let provider = provider(&endpoint, "txt", "api_key", "fixture-api-key").await;
    assert_eq!(
        submission_contract(&provider).await,
        SubmissionOutcome::DefinitelyNotAccepted {
            reason: SmsReason::AuthenticationFailed,
            provider_code: Some(30)
        }
    );
    let fields = fields(&request.await.unwrap());
    assert_eq!(fields["p"], "fixture-api-key");
    assert_eq!(fields["f"], "txt");
    assert_eq!(task.await.unwrap(), 1);
}

#[tokio::test]
async fn prepared_data_is_bound_to_instance_even_with_equal_profile_ids() {
    let first = provider("http://127.0.0.1:1", "json", "api_key", "fixture-key").await;
    let second = provider("http://127.0.0.1:1", "json", "api_key", "different-key").await;
    let prepared = first.prepare(plan()).await.unwrap();
    assert_eq!(
        second.submit(prepared, context()).await,
        refused(SmsReason::InvalidPreparedBinding)
    );
}

#[tokio::test]
async fn prepare_refuses_wrong_scope_phone_content_and_unsupported_template() {
    let provider = provider("http://127.0.0.1:1", "json", "api_key", "fixture-key").await;
    let mut invalid = plan();
    invalid.binding.profile_revision = 2;
    assert!(provider.prepare(invalid).await.is_err());
    let mut invalid = plan();
    invalid.recipient = PhoneNumber::parse("+14155552671").unwrap();
    assert!(provider.prepare(invalid).await.is_err());
    let mut invalid = plan();
    invalid.content = SmsContent::text(" ".into());
    assert!(provider.prepare(invalid).await.is_err());
    let mut invalid = plan();
    invalid.content = SmsContent::text("长".repeat(501));
    assert!(provider.prepare(invalid).await.is_err());
    // The signature consumes five Unicode characters of the 500-character bound.
    let mut boundary = plan();
    boundary.content = SmsContent::text("长".repeat(495));
    assert!(provider.prepare(boundary).await.is_ok());
    let mut invalid = plan();
    invalid.content = SmsContent::text("长".repeat(496));
    assert!(provider.prepare(invalid).await.is_err());
    let mut invalid = plan();
    invalid.content = SmsContent::Template {
        id: "native-template".into(),
        variables: BTreeMap::new(),
    };
    assert!(matches!(
        provider.prepare(invalid).await,
        Err(SmsError::Refused {
            reason: SmsReason::UnsupportedCapability
        })
    ));
    let prepared = provider.prepare(plan()).await.unwrap();
    assert_eq!(
        provider
            .submit(
                prepared,
                SubmitContext {
                    deadline: Instant::now()
                }
            )
            .await,
        refused(SmsReason::Expired)
    );
}

#[tokio::test]
async fn redirect_and_server_errors_do_not_replay_side_effect_get() {
    for status in [302, 429, 500, 503] {
        let (endpoint, request, task) =
            fixture(status, b"private-provider-response", Duration::ZERO, false).await;
        let provider = provider(&endpoint, "json", "api_key", "fixture-key").await;
        let outcome = submission_contract(&provider).await;
        assert_eq!(
            outcome,
            SubmissionOutcome::Indeterminate {
                reason: SmsReason::TransportFailure
            }
        );
        assert!(!format!("{outcome:?}").contains("private-provider-response"));
        request.await.unwrap();
        assert_eq!(task.await.unwrap(), 1);
    }
}

#[tokio::test]
async fn response_loss_and_deadline_after_submission_stay_unknown() {
    for (delay, truncate) in [(Duration::ZERO, true), (Duration::from_millis(100), false)] {
        let (endpoint, request, task) = fixture(200, br#"{"code":0}"#, delay, truncate).await;
        let provider = provider(&endpoint, "json", "api_key", "fixture-key").await;
        let prepared = provider.prepare(plan()).await.unwrap();
        let outcome = provider
            .submit(
                prepared,
                SubmitContext {
                    deadline: Instant::now() + Duration::from_millis(30),
                },
            )
            .await;
        assert_eq!(
            outcome,
            SubmissionOutcome::Indeterminate {
                reason: SmsReason::TransportFailure
            }
        );
        request.await.unwrap();
        assert_eq!(task.await.unwrap(), 1);
    }
}

#[tokio::test]
async fn oversized_response_is_bounded_and_never_treated_as_safe_rejection() {
    let (endpoint, _, task) = fixture(
        200,
        &vec![b'x'; MAX_RESPONSE_BYTES + 1],
        Duration::ZERO,
        false,
    )
    .await;
    let provider = provider(&endpoint, "json", "api_key", "fixture-key").await;
    assert_eq!(
        submission_contract(&provider).await,
        SubmissionOutcome::Indeterminate {
            reason: SmsReason::UnrecognizedProviderResponse
        }
    );
    assert_eq!(task.await.unwrap(), 1);
}

#[tokio::test]
async fn balance_query_is_read_only_and_uses_the_configured_format() {
    let (endpoint, request, task) = fixture(
        200,
        br#"{"code":0,"data":{"balance":42}}"#,
        Duration::ZERO,
        false,
    )
    .await;
    let provider = provider(&endpoint, "json", "api_key", "fixture-key").await;
    assert_eq!(provider.query_balance().await.unwrap().remaining, 42);
    let request = request.await.unwrap();
    assert!(request.starts_with("GET /query?"));
    let fields = fields(&request);
    assert_eq!(fields.len(), 3);
    assert!(!fields.contains_key("m"));
    assert_eq!(task.await.unwrap(), 1);
}

#[tokio::test]
async fn prehashed_credentials_are_validated_without_double_hashing() {
    let profile = PROFILE.replace(
        "auth_method = \"password\"",
        "auth_method = \"password_md5\"",
    );
    let config = SmsbaoConfig::from_acl(&profile).unwrap();
    assert!(SmsbaoProvider::from_config(
        config.clone(),
        &FakeSecrets {
            credential: "not-a-digest"
        }
    )
    .await
    .is_err());
    let provider = SmsbaoProvider::from_config(
        config,
        &FakeSecrets {
            credential: "ABCDEF0123456789ABCDEF0123456789",
        },
    )
    .await
    .unwrap();
    assert_eq!(
        provider.credential.expose_secret(),
        "abcdef0123456789abcdef0123456789"
    );
}
