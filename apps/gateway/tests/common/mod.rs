//! Shared auth for Cloud-managed Gateway integration tests.
//! Enabled management requires a non-empty `auth_token_env` (no soft-open).

use reqwest::header::AUTHORIZATION;

pub const TEST_ADMIN_TOKEN: &str = "managed-test-admin-token";
pub const TEST_ADMIN_TOKEN_ENV: &str = "ASTER_GATEWAY_TEST_ADMIN_TOKEN";

pub fn ensure_test_admin_token() {
    std::env::set_var(TEST_ADMIN_TOKEN_ENV, TEST_ADMIN_TOKEN);
}

pub fn bearer_header() -> (reqwest::header::HeaderName, String) {
    (AUTHORIZATION, format!("Bearer {TEST_ADMIN_TOKEN}"))
}
