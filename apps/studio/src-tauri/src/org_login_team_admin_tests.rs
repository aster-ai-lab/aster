#![cfg(test)]

const ID: &str = "0197e5a2-89f1-7d33-a11c-5f53d9b3af10";
const ORG: &str = "/v1/api/org/members/orgs/org-1";
const MEMBERS: &str = "/v1/api/org/members";

fn only(path: &str, allowed: &[&str]) {
    for method in ["GET", "POST", "PUT", "DELETE", "PATCH"] {
        assert_eq!(
            super::allowed_member_path(method, path),
            allowed.contains(&method),
            "{method} {path}"
        );
    }
}

#[test]
fn member_status_is_a_scoped_put() {
    only(&format!("{ORG}/members/{ID}/status"), &["PUT"]);
    for path in [
        format!("{ORG}/members/not-a-uuid/status"),
        format!("{ORG}/members/{ID}/status/extra"),
        format!("{ORG}/members/{ID}/statuses"),
        format!("/v1/api/org/members/orgs/bad%2forg/members/{ID}/status"),
    ] {
        only(&path, &[]);
    }
}

#[test]
fn plan_and_verification_follow_their_owner_contracts() {
    only(&format!("{ORG}/subscription"), &["GET", "PUT"]);
    only(&format!("{ORG}/entitlements"), &["GET"]);
    only(&format!("{ORG}/audit"), &["GET"]);
    only(&format!("{ORG}/verification"), &["GET", "POST"]);
    only(&format!("{MEMBERS}/verification"), &["GET", "POST"]);
    for tail in [
        "subscription/extra",
        "entitlements/extra",
        "verification/decision",
        "audit/export",
    ] {
        only(&format!("{ORG}/{tail}"), &[]);
    }
    only(&format!("{MEMBERS}/verifications/reviews"), &[]);
}

#[test]
fn team_runs_read_logs_and_stop_only_by_uuid() {
    only(&format!("{ORG}/runs"), &["GET"]);
    only(&format!("{ORG}/runs/{ID}/logs"), &["GET"]);
    only(&format!("{ORG}/runs/{ID}/stop"), &["POST"]);
    for tail in [
        format!("runs/{ID}"),
        format!("runs/{ID}/logs/extra"),
        format!("runs/{ID}/restart"),
        "runs/not-a-uuid/stop".into(),
        "runs/../stop".into(),
        format!("runs/{ID}%2fextra/stop"),
    ] {
        only(&format!("{ORG}/{tail}"), &[]);
    }
}

#[test]
fn the_capability_policy_is_read_and_replaced_whole() {
    only(&format!("{ORG}/capability-policy"), &["GET", "PUT"]);
    for tail in [
        "capability-policy/extra",
        "capability-policies",
        "capability-policy/allowed",
    ] {
        only(&format!("{ORG}/{tail}"), &[]);
    }
}

#[test]
fn invites_and_access_tokens_revoke_only_by_uuid() {
    for root in ["access-tokens", "invites"] {
        only(&format!("{MEMBERS}/{root}"), &["GET", "POST"]);
        only(&format!("{MEMBERS}/{root}/{ID}"), &["DELETE"]);
        for tail in ["not-a-uuid", "../orgs", "", &format!("{ID}/extra")] {
            only(&format!("{MEMBERS}/{root}/{tail}"), &[]);
        }
    }
}
