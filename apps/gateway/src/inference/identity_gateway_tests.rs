use super::*;

#[test]
fn trusted_client_identity_survives_repeated_preparation_and_other_targets_strip_it() {
    let request = InferenceRequestIdentity::new(
        "synthetic-gateway".into(),
        Uuid::new_v4(),
        1,
        InferenceEndpoint::ChatCompletions,
    );
    let mut headers = HeaderMap::new();
    headers.insert("x-aster-gateway-client", "forged".parse().unwrap());
    let mut attempt = request.begin_attempt(Uuid::new_v4());
    let client = Uuid::new_v4();
    attempt.set_gateway_client(client);
    for _ in 0..2 {
        attempt.prepare_upstream_headers(&mut headers);
        assert_eq!(headers["x-aster-gateway-client"], client.to_string());
    }
    request
        .begin_attempt(Uuid::new_v4())
        .prepare_upstream_headers(&mut headers);
    assert!(!headers.contains_key("x-aster-gateway-client"));
}
