const REPOS: &str = "/v1/api/org/repositories";
const OID: &str = "0123456789abcdef0123456789abcdef01234567";

fn only(path: &str, allowed: &[&str]) {
    for method in ["GET", "POST", "PUT", "DELETE", "PATCH"] {
        assert_eq!(
            crate::org_login::allowed_member_path(method, path),
            allowed.contains(&method),
            "{method} {path}"
        );
    }
}

#[test]
fn listing_and_creating_use_the_collection() {
    only(REPOS, &["GET", "POST"]);
    only(&format!("{REPOS}/"), &[]);
    only(&format!("{REPOS}x"), &[]);
}

#[test]
fn browsing_reads_only_well_formed_repositories() {
    let repo = format!("{REPOS}/beichen/web");
    only(&repo, &["GET"]);
    for tail in ["branches", "tags", "tree", "blob", "commits", "compare"] {
        only(&format!("{repo}/{tail}"), &["GET"]);
    }
    only(&format!("{repo}/commits/{OID}"), &["GET"]);
    only(&format!("{repo}/commits/{OID}/checks"), &["GET"]);
    only(&format!("{repo}/protections"), &["GET", "PUT"]);
    for path in [
        format!("{REPOS}/Beichen/web"),
        format!("{REPOS}/beichen/web-"),
        format!("{REPOS}/beichen/../web"),
        format!("{REPOS}/beichen"),
        format!("{repo}/commits/main"),
        format!("{repo}/commits/{OID}/checks/extra"),
        format!("{repo}/refs/heads/main"),
        format!("{repo}/objects"),
    ] {
        only(&path, &[]);
    }
}

#[test]
fn change_requests_move_through_numbered_lifecycle_actions() {
    let requests = format!("{REPOS}/beichen/web/change-requests");
    only(&requests, &["GET", "POST"]);
    only(&format!("{requests}/12"), &["GET"]);
    for action in ["comments", "reviews", "merge", "close", "reopen"] {
        only(&format!("{requests}/12/{action}"), &["POST"]);
    }
    for path in [
        format!("{requests}/twelve"),
        format!("{requests}/-1"),
        format!("{requests}/1234567890"),
        format!("{requests}/12/delete"),
    ] {
        only(&path, &[]);
    }
}

#[test]
fn a_team_namespace_is_read_by_team_id() {
    let team = "0197e5a2-89f1-7d33-a11c-5f53d9b3af10";
    only(
        &format!("/v1/api/org/repository-namespaces/{team}"),
        &["GET"],
    );
    only("/v1/api/org/repository-namespaces/", &[]);
    only("/v1/api/org/repository-namespaces/beichen", &[]);
    only(&format!("/v1/api/org/repository-namespaces/{team}/x"), &[]);
}

#[test]
fn pipeline_runs_are_read_only_for_members() {
    let run = "0197e5a2-89f1-7d33-a11c-5f53d9b3af10";
    only("/v1/api/org/pipelines/repos/beichen.web", &["GET"]);
    only(&format!("/v1/api/org/pipelines/runs/{run}"), &["GET"]);
    for path in [
        "/v1/api/org/pipelines/repos/..web".to_string(),
        "/v1/api/org/pipelines/repos/beichen.web/workflows".to_string(),
        "/v1/api/org/pipelines/repos/beichen.web/secrets".to_string(),
        "/v1/api/org/pipelines/runners".to_string(),
        "/v1/api/org/pipelines/runs/not-a-run".to_string(),
        format!("/v1/api/org/pipelines/runs/{run}/cancel"),
    ] {
        only(&path, &[]);
    }
}
