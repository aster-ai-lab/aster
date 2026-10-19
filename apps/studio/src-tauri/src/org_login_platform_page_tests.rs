#![cfg(test)]

#[test]
fn supplier_related_pages_admit_only_closed_addressed_reads() {
    for kind in ["models", "dependencies"] {
        let path = format!(
            "/v1/api/org/members/platform/suppliers/7f8c1c62-6a32-4e7d-9a1b-3a2f0c4d5e6f/{kind}/page"
        );
        assert!(super::allowed_member_path("GET", &path));
        for method in ["POST", "PUT", "PATCH", "DELETE"] {
            assert!(!super::allowed_member_path(method, &path));
        }
        assert!(!super::allowed_member_path("GET", &format!("{path}/extra")));
        for id in [
            "bad-id",
            "00000000-0000-0000-0000-000000000000",
            "%2e%2e",
            "../suppliers",
        ] {
            assert!(!super::allowed_member_path(
                "GET",
                &format!("/v1/api/org/members/platform/suppliers/{id}/{kind}/page")
            ));
        }
    }
    assert!(!super::allowed_member_path(
        "GET",
        "/v1/api/org/members/platform/suppliers/7f8c1c62-6a32-4e7d-9a1b-3a2f0c4d5e6f/secrets/page"
    ));
}

#[test]
fn owner_catalog_pages_cross_only_the_member_read_proxy() {
    for resource in ["suppliers", "models"] {
        let path = format!("/v1/api/org/members/platform/{resource}/page");
        assert!(super::allowed_member_path("GET", &path));
        for method in ["POST", "PUT", "DELETE", "PATCH"] {
            assert!(!super::allowed_member_path(method, &path));
        }
        assert!(!super::allowed_member_path("GET", &format!("{path}/extra")));
        assert!(!super::allowed_member_path(
            "GET",
            &format!("/v1/api/admin/model-service/{resource}/page")
        ));
    }
    assert!(!super::allowed_member_path(
        "GET",
        "/v1/api/org/members/platform/secrets/page"
    ));
}

#[test]
fn supplier_commands_admit_only_exact_member_resource_paths() {
    let root = "/v1/api/org/members/platform/suppliers";
    let record = format!("{root}/7f8c1c62-6a32-4e7d-9a1b-3a2f0c4d5e6f");
    assert!(super::allowed_member_path("POST", root));
    assert!(super::allowed_member_path("GET", &format!("{root}/access")));
    for method in ["GET", "PUT", "DELETE"] {
        assert!(super::allowed_member_path(method, &record));
    }
    assert!(!super::allowed_member_path("POST", &record));
    for tail in [
        "access",
        "page",
        "../models",
        "%2e%2e",
        "not-an-id",
        "7f8c1c62-6a32-4e7d-9a1b-3a2f0c4d5e6f/probes",
        "7f8c1c62-6a32-4e7d-9a1b-3a2f0c4d5e6f/extra",
    ] {
        for method in ["POST", "PUT", "DELETE"] {
            assert!(!super::allowed_member_path(
                method,
                &format!("{root}/{tail}")
            ));
        }
    }
    for method in ["POST", "PUT", "DELETE"] {
        assert!(!super::allowed_member_path(
            method,
            "/v1/api/admin/model-service/suppliers"
        ));
        assert!(!super::allowed_member_path(
            method,
            "/v1/api/org/members/platform/models"
        ));
    }
}

#[test]
fn supplier_intent_resolution_admits_only_an_addressed_post() {
    let intent = "7f8c1c62-6a32-4e7d-9a1b-3a2f0c4d5e6f";
    let path = format!("/v1/api/org/members/platform/suppliers/intents/{intent}/resolve");
    assert!(super::allowed_member_path("POST", &path));
    for method in ["GET", "PUT", "DELETE", "PATCH"] {
        assert!(!super::allowed_member_path(method, &path));
    }
    for invalid in [
        "abc",
        "00000000-0000-0000-0000-000000000000",
        "../",
        "a/b",
        "a%2fb",
    ] {
        assert!(!super::allowed_member_path(
            "POST",
            &format!("/v1/api/org/members/platform/suppliers/intents/{invalid}/resolve")
        ));
    }
    for tail in [
        format!("suppliers/intents/{intent}/resolve/extra"),
        format!("suppliers/intents/{intent}/cancel"),
        "suppliers/abc".into(),
    ] {
        assert!(!super::allowed_member_path(
            "POST",
            &format!("/v1/api/org/members/platform/{tail}")
        ));
    }
}
