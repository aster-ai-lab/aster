#![cfg(test)]

//! The member-proxy whitelist admits exactly what the renderer calls. These
//! cases pin the three lanes the org pages and the drive pipeline panels use,
//! after they silently 404'd behind a missing arm.

const ORG: &str = "0197e5a2-89f1-7d33-a11c-5f53d9b3af10";
const USER: &str = "0197e5a2-89f1-7d33-a11c-5f53d9b3af21";
const FILE: &str = "0197e5a2-89f1-7d33-a11c-5f53d9b3af32";

fn org_root() -> String {
    format!("/v1/api/org/members/orgs/{ORG}")
}

fn drive_path(tail: &str) -> String {
    format!("/v1/api/org/drive/{ORG}/{tail}")
}

#[test]
fn member_department_move_is_a_scoped_put() {
    let path = format!("{}/members/{USER}/department", org_root());
    assert!(super::allowed_member_path("PUT", &path));
    for method in ["GET", "POST", "DELETE", "PATCH"] {
        assert!(!super::allowed_member_path(method, &path));
    }
    assert!(!super::allowed_member_path(
        "PUT",
        &format!("{}/members/not-a-uuid/department", org_root())
    ));
    assert!(!super::allowed_member_path(
        "PUT",
        &format!("{}/members/{USER}/department/extra", org_root())
    ));
    assert!(!super::allowed_member_path(
        "PUT",
        &format!("{}/members/{USER}/departments", org_root())
    ));
}

#[test]
fn drive_pipeline_lane_is_admitted_for_reads_parse_and_results() {
    assert!(super::allowed_member_path("GET", &drive_path("pipeline")));
    assert!(super::allowed_member_path(
        "POST",
        &drive_path(&format!("files/{FILE}/parse"))
    ));
    assert!(super::allowed_member_path(
        "GET",
        &drive_path(&format!("files/{FILE}/encyclopedia"))
    ));
}

#[test]
fn drive_pipeline_lane_rejects_wrong_shapes() {
    for (method, tail) in [
        ("POST", "pipeline"),
        ("GET", "pipeline/extra"),
        ("PUT", "pipeline"),
        ("GET", "files/not-a-uuid/parse"),
        ("POST", "files/not-a-uuid/parse"),
        ("POST", "files/not-a-uuid/encyclopedia"),
        ("GET", "files/not-a-uuid/encyclopedia"),
        ("DELETE", "pipeline"),
    ] {
        assert!(
            !super::allowed_member_path(method, &drive_path(tail)),
            "{method} /{tail} must not pass"
        );
    }
}
