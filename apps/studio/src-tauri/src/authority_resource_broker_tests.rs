#![cfg(test)]

use super::super::{
    StudioAuthority,
    tests::{request, split_response},
};
use super::*;
use std::{
    io::{Cursor, Write},
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

fn bundle(marker: &str, read_only: bool) -> Vec<u8> {
    let acl = if read_only {
        r#"extension "proof/document-editor" {
        schema_version = 3 version = "1.0.0" route = "document-editor"
        requires_use = ">=0.3.0, <0.4.0" actions = ["read"]
        repository { url = "https://example.org/proof/document-editor" }
        ui "editor" { title = "Document editor" entry = "ui/index.html" root = "ui" point = "studio.applets" presentation = "webview" contract_version = 1 mode = "append" optional = false }
    }"#
    } else {
        r#"extension "proof/document-editor" {
        schema_version = 3 version = "1.0.0" route = "document-editor"
        requires_use = ">=0.3.0, <0.4.0" actions = ["read", "mutate"]
        repository { url = "https://example.org/proof/document-editor" }
        ui "editor" { title = "Document editor" entry = "ui/index.html" root = "ui" point = "studio.applets" presentation = "webview" contract_version = 1 mode = "append" optional = false }
    }"#
    };
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (path, bytes) in [
        ("a3s-use-extension.acl", acl),
        ("README.md", marker),
        ("ui/index.html", "<html><body>Document editor</body></html>"),
    ] {
        zip.start_file(path, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(bytes.as_bytes()).unwrap();
    }
    zip.finish().unwrap().into_inner()
}

struct NativeProof {
    authority: StudioAuthority,
    root: PathBuf,
}
impl Drop for NativeProof {
    fn drop(&mut self) {
        self.authority.shutdown();
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
impl NativeProof {
    fn new(origin: &str, bearer: &str, member: &str, organization: Uuid) -> Self {
        let root = std::env::temp_dir().join(format!("a3s-document-native-{}", Uuid::now_v7()));
        let authority =
            StudioAuthority::start_with_store(origin, root.join("capabilities")).unwrap();
        authority.shared.member_session.note_sign_in(bearer, member);
        authority
            .shared
            .member_session
            .save(json!({"token":bearer}).to_string().as_bytes())
            .unwrap();
        super::super::store_org_center(
            &authority.shared,
            json!({"id":organization}).to_string().as_bytes(),
        );
        let proof = Self { authority, root };
        proof.install("first generation");
        proof
    }
    fn install(&self, marker: &str) {
        self.install_with_access(marker, false);
    }
    fn install_with_access(&self, marker: &str, read_only: bool) {
        let bytes = bundle(marker, read_only);
        let entry = json!({"slug":"document-editor","kind":"plugin","installed":true,"install":{"type":"plugin","packageType":"aix","available":true,"packageId":"use/proof/document-editor","digest":crate::capabilities::digest_of(&bytes),"size":bytes.len()}});
        self.authority
            .shared
            .capabilities
            .reconcile(&[entry], |_| Ok(bytes.clone()))
            .unwrap();
    }
    fn call(&self, method: &str, path: &str, body: Value) -> (u16, Value) {
        let raw = request(
            self.authority.address().port(),
            method,
            path,
            self.authority.token(),
            None,
            &body.to_string(),
        );
        let (status, _, body) = split_response(&raw);
        (status, serde_json::from_str(&body).unwrap())
    }
    fn open(&self) -> String {
        let (_, catalog) = self.call("GET", "/api/aix/views", Value::Null);
        let id = || format!("{:064x}", Uuid::now_v7().as_u128());
        let expires = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
            + 30_000;
        let input = json!({"protocolVersion":3,"requestId":id(),"activationId":id(),"instanceId":id(),"expiresAt":expires,"slug":"document-editor","surface":"editor","revision":catalog["packages"][0]["revision"]});
        let (status, opened) = self.call("POST", "/api/aix/views/sessions", input);
        assert_eq!(status, 200, "{opened}");
        opened["token"].as_str().unwrap().into()
    }
    fn grant(&self, view: &str, workspace: Uuid, project: Uuid, access: &str) -> String {
        let (status, lease) = self.call("POST", &format!("/api/aix/views/sessions/{view}/resources/leases"), json!({"protocolVersion":1,"workspaceId":workspace,"projectId":project,"resources":["scene"],"access":access}));
        assert_eq!(status, 200, "{lease}");
        assert_eq!(lease["identity"]["packageId"], "use/proof/document-editor");
        assert!(!lease.to_string().contains("a3s_ms_"));
        lease["lease"].as_str().unwrap().into()
    }
    fn resource(&self, view: &str, lease: &str, operation: Value) -> (u16, Value) {
        self.call(
            "POST",
            &format!("/api/aix/views/sessions/{view}/resources"),
            json!({"protocolVersion":1,"lease":lease,"request":operation}),
        )
    }
}

#[test]
fn resource_transport_refuses_arbitrary_paths_and_unknown_fields() {
    let organization = Uuid::now_v7();
    let workspace = Uuid::now_v7();
    let project = Uuid::now_v7();
    let base = format!("{}/{project}", project_path(organization, workspace));
    assert!(allows_upstream(
        "GET",
        &format!("{base}/resources/scene?revision=4")
    ));
    assert!(allows_upstream("PUT", &format!("{base}/resources/scene")));
    assert!(allows_upstream(
        "GET",
        &format!("{}?page=2&limit=20", project_path(organization, workspace))
    ));
    for query in [
        "page=0",
        "limit=101",
        "page=1&page=2",
        "offset=1",
        "workspace=other",
        "page=100002&limit=1",
    ] {
        assert!(!allows_upstream(
            "GET",
            &format!("{}?{query}", project_path(organization, workspace))
        ));
    }
    for (method, path) in [
        ("DELETE", base.clone()),
        ("GET", format!("{base}/resources/../secret")),
        (
            "GET",
            format!("{base}/resources/scene?revision=4&member=other"),
        ),
        ("GET", "/v1/api/admin/identity".into()),
        (
            "GET",
            format!("{base}/resources/scene?revision={}", MAX_REVISION + 1),
        ),
    ] {
        assert!(!allows_upstream(method, &path));
    }
    for access in [ResourceAccess::Read, ResourceAccess::ReadWrite] {
        assert!(ResourceAccess::parse(access.as_str()).is_ok());
    }
    assert!(ResourceAccess::parse("owner").is_err());
    let valid = json!({"protocolVersion":1,"lease":"opaque","request":{"operation":"read","key":"scene","revision":null}});
    assert!(serde_json::from_value::<ResourceRequest>(valid.clone()).is_ok());
    let mut forged = valid;
    forged["request"]["projectId"] = json!(project);
    assert!(serde_json::from_value::<ResourceRequest>(forged).is_err());
}

#[test]
fn native_authoring_drafts_are_durable_scoped_and_revocable() {
    let member = Uuid::now_v7().to_string();
    let organization = Uuid::now_v7();
    let proof = NativeProof::new(
        "http://127.0.0.1:9",
        "a3s_ms_draft_owner_token",
        &member,
        organization,
    );
    let view = proof.open();
    let path = |view: &str| format!("/api/aix/views/sessions/{view}/resources/drafts/session");
    assert_eq!(
        proof.call("GET", &path(&view), Value::Null).1,
        json!({"revision":0,"content":null})
    );
    let input = json!({"expectedRevision":0,"operationId":Uuid::now_v7(),"content":"{\"scene\":\"safe draft\",\"saveIntent\":\"same operation\"}"});
    assert_eq!(proof.call("PUT", &path(&view), input.clone()).0, 200);
    assert_eq!(
        proof.call("PUT", &path(&view), input.clone()).1["revision"],
        1
    );
    let mut mismatch = input.clone();
    mismatch["content"] = json!("changed bytes");
    assert_eq!(proof.call("PUT", &path(&view), mismatch).0, 409);
    let mut forged = input.clone();
    forged["memberId"] = json!(Uuid::now_v7());
    assert_eq!(proof.call("PUT", &path(&view), forged).0, 400);
    assert_eq!(
        proof
            .call("GET", &format!("{}?member=other", path(&view)), Value::Null)
            .0,
        400
    );
    proof.install_with_access("read-only generation", true);
    assert_eq!(proof.call("GET", &path(&view), Value::Null).0, 404);
    let readonly = proof.open();
    assert_eq!(
        proof.call("GET", &path(&readonly), Value::Null).1["content"],
        input["content"]
    );
    assert_eq!(proof.call("PUT", &path(&readonly), input.clone()).0, 404);
    proof
        .authority
        .shared
        .capabilities
        .reconcile(&[], |_| unreachable!())
        .unwrap();
    assert_eq!(proof.call("GET", &path(&readonly), Value::Null).0, 404);
    proof.install("reinstalled generation");
    let fresh = proof.open();
    assert_eq!(
        proof.call("GET", &path(&fresh), Value::Null).1["content"],
        input["content"]
    );
    proof
        .authority
        .shared
        .member_session
        .note_sign_in("a3s_ms_different_member_token", &Uuid::now_v7().to_string());
    proof
        .authority
        .shared
        .member_session
        .save(br#"{"token":"a3s_ms_different_member_token"}"#)
        .unwrap();
    assert_eq!(proof.call("GET", &path(&fresh), Value::Null).0, 404);
}

// The API campaign launches this exact native route proof against both real adapters.
#[test]
#[ignore = "requires A3S_CLOUD_TEST_DOCUMENT_HTTP_* supplied by the owning API campaign"]
fn native_document_round_trip() {
    let origin = std::env::var("A3S_CLOUD_TEST_DOCUMENT_HTTP_ORIGIN").unwrap();
    let bearer = std::env::var("A3S_CLOUD_TEST_DOCUMENT_HTTP_BEARER").unwrap();
    let member = std::env::var("A3S_CLOUD_TEST_DOCUMENT_HTTP_MEMBER").unwrap();
    let organization = std::env::var("A3S_CLOUD_TEST_DOCUMENT_HTTP_ORGANIZATION")
        .unwrap()
        .parse()
        .unwrap();
    let workspace = std::env::var("A3S_CLOUD_TEST_DOCUMENT_HTTP_WORKSPACE")
        .unwrap()
        .parse()
        .unwrap();
    let project = std::env::var("A3S_CLOUD_TEST_DOCUMENT_HTTP_PROJECT")
        .unwrap()
        .parse()
        .unwrap();
    let proof = NativeProof::new(&origin, &bearer, &member, organization);
    let view = proof.open();
    let picker_workspace = Uuid::now_v7();
    let picker_project = Uuid::now_v7();
    let project_input =
        json!({"workspaceId":picker_workspace,"id":picker_project,"title":"Editor project"});
    let picker_path = format!("/api/aix/views/sessions/{view}/projects");
    assert_eq!(
        proof
            .call(
                "GET",
                &format!("/api/aix/views/sessions/{view}/resources/access"),
                Value::Null
            )
            .1["canWrite"],
        true
    );
    let page_path = format!("{picker_path}?workspace={picker_workspace}&page=1&limit=20");
    assert_eq!(
        proof.call("GET", &page_path, Value::Null).1["data"]["total"],
        0
    );
    assert_eq!(
        proof.call("POST", &picker_path, project_input.clone()).0,
        201
    );
    assert_eq!(
        proof.call("POST", &picker_path, project_input.clone()).0,
        201
    );
    let (status, projects) = proof.call("GET", &page_path, Value::Null);
    assert_eq!(status, 200);
    assert_eq!(projects["data"]["total"], 1);
    assert_eq!(
        projects["data"]["items"][0]["id"],
        picker_project.to_string()
    );
    assert_eq!(
        proof
            .call("GET", &format!("{page_path}&page=2"), Value::Null)
            .0,
        400
    );
    let read = json!({"operation":"read","key":"scene","revision":null});
    let lease = proof.grant(&view, workspace, project, "read-write");
    let (status, snapshot) = proof.resource(&view, &lease, read.clone());
    assert_eq!(status, 200, "{snapshot}");
    let revision = snapshot["data"]["revision"].as_u64().unwrap();
    let save = json!({"operation":"save","key":"scene","operationId":Uuid::now_v7(),"expectedRevision":revision,"content":"scene \"native-workcell\" { value = 3 }","assets":[]});
    let (status, receipt) = proof.resource(&view, &lease, save.clone());
    assert_eq!(status, 200, "{receipt}");
    assert_eq!(receipt["data"]["document"]["revision"], revision + 1);
    let queried = proof.resource(
        &view,
        &lease,
        json!({"operation":"read-receipt","operationId":save["operationId"]}),
    );
    assert_eq!(queried.0, 200);
    assert_eq!(queried.1["data"], receipt["data"]);
    let unoffered = proof.open();
    let (status, other_lease) = proof.call("POST", &format!("/api/aix/views/sessions/{unoffered}/resources/leases"), json!({"protocolVersion":1,"workspaceId":workspace,"projectId":project,"resources":["private"],"access":"read"}));
    assert_eq!(status, 200);
    assert_eq!(
        proof
            .resource(
                &unoffered,
                other_lease["lease"].as_str().unwrap(),
                json!({"operation":"read-receipt","operationId":save["operationId"]})
            )
            .0,
        404
    );
    let bytes = b"native immutable asset";
    let digest = crate::capabilities::digest_of(bytes);
    assert_eq!(proof.resource(&view, &lease, json!({"operation":"store-asset","digest":digest,"kind":"glb","bytesBase64":STANDARD.encode(bytes)})).0, 200);
    let (status, asset) = proof.resource(
        &view,
        &lease,
        json!({"operation":"read-asset","digest":digest}),
    );
    assert_eq!(status, 200);
    assert_eq!(
        STANDARD
            .decode(asset["bytesBase64"].as_str().unwrap())
            .unwrap(),
        bytes
    );
    assert_eq!(
        proof.resource(&view, &lease, save.clone()).1["data"],
        receipt["data"]
    );
    let mut stale = save.clone();
    stale["operationId"] = json!(Uuid::now_v7());
    assert_eq!(proof.resource(&view, &lease, stale).0, 409);
    let mut late_save = save.clone();
    late_save["operationId"] = json!(Uuid::now_v7());
    late_save["expectedRevision"] = json!(revision + 1);
    late_save["content"] = json!("scene \"late-result\" { value = 99 }");
    let read_only = proof.grant(&view, workspace, project, "read");
    assert_eq!(proof.resource(&view, &read_only, save.clone()).0, 404);
    assert_eq!(
        proof
            .resource(
                &view,
                &lease,
                json!({"operation":"read","key":"private","revision":null})
            )
            .0,
        404
    );
    let other_view = proof.open();
    assert_eq!(proof.resource(&other_view, &lease, read.clone()).0, 404);
    assert_eq!(
        proof
            .call(
                "DELETE",
                &format!("/api/aix/views/sessions/{view}/resources/leases/{lease}"),
                Value::Null
            )
            .0,
        200
    );
    assert_eq!(proof.resource(&view, &lease, read.clone()).0, 404);
    assert_eq!(proof.resource(&view, &lease, late_save.clone()).0, 404);
    let lease = proof.grant(&view, workspace, project, "read-write");
    super::super::store_org_center(
        &proof.authority.shared,
        json!({"id":Uuid::now_v7()}).to_string().as_bytes(),
    );
    assert_eq!(proof.resource(&view, &lease, read.clone()).0, 404);
    super::super::store_org_center(
        &proof.authority.shared,
        json!({"id":organization}).to_string().as_bytes(),
    );
    assert_eq!(proof.resource(&view, &lease, read.clone()).0, 404);
    let view = proof.open();
    let lease = proof.grant(&view, workspace, project, "read-write");
    assert_eq!(
        proof
            .call(
                "DELETE",
                &format!("/api/aix/views/sessions/{view}"),
                Value::Null
            )
            .0,
        200
    );
    assert_eq!(proof.resource(&view, &lease, read.clone()).0, 404);
    let view = proof.open();
    let lease = proof.grant(&view, workspace, project, "read-write");
    proof.install("replacement generation");
    assert_eq!(proof.resource(&view, &lease, read.clone()).0, 404);
    assert_eq!(proof.resource(&view, &lease, late_save.clone()).0, 404);
    let view = proof.open();
    let lease = proof.grant(&view, workspace, project, "read-write");
    proof
        .authority
        .shared
        .capabilities
        .reconcile(&[], |_| unreachable!())
        .unwrap();
    assert_eq!(proof.resource(&view, &lease, read.clone()).0, 404);
    assert_eq!(proof.resource(&view, &lease, late_save.clone()).0, 404);
    proof.install("replacement generation");
    let view = proof.open();
    let lease = proof.grant(&view, workspace, project, "read-write");
    let restarted = NativeProof::new(&origin, &bearer, &member, organization);
    assert_eq!(restarted.resource(&view, &lease, read.clone()).0, 404);
    assert_eq!(restarted.resource(&view, &lease, late_save.clone()).0, 404);
    proof
        .authority
        .shared
        .resource_broker
        .leases
        .lock()
        .unwrap()
        .get_mut(&lease)
        .unwrap()
        .expires = Instant::now();
    assert_eq!(proof.resource(&view, &lease, read.clone()).0, 404);
    assert_eq!(proof.resource(&view, &lease, late_save.clone()).0, 404);
    proof.install_with_access("read-only generation", true);
    let view = proof.open();
    let picker_path = format!("/api/aix/views/sessions/{view}/projects");
    assert_eq!(
        proof
            .call(
                "GET",
                &format!("/api/aix/views/sessions/{view}/resources/access"),
                Value::Null
            )
            .1["canWrite"],
        false
    );
    assert_eq!(
        proof
            .call(
                "GET",
                &format!("{picker_path}?workspace={picker_workspace}"),
                Value::Null
            )
            .1["data"]["total"],
        1
    );
    let mut denied_project = project_input;
    denied_project["id"] = json!(Uuid::now_v7());
    assert_eq!(proof.call("POST", &picker_path, denied_project).0, 404);
    assert_eq!(proof.call("POST", &format!("/api/aix/views/sessions/{view}/resources/leases"), json!({"protocolVersion":1,"workspaceId":workspace,"projectId":project,"resources":["scene"],"access":"read-write"})).0, 404);
    let lease = proof.grant(&view, workspace, project, "read");
    assert_eq!(proof.resource(&view, &lease, read.clone()).0, 200);
    assert_eq!(proof.resource(&view, &lease, save).0, 404);
    proof.authority.shared.member_session.clear().unwrap();
    assert_eq!(
        proof
            .call(
                "GET",
                &format!("/api/aix/views/sessions/{view}/resources/access"),
                Value::Null
            )
            .0,
        404
    );
    assert_eq!(proof.call("GET", &page_path, Value::Null).0, 404);
    assert_eq!(proof.resource(&view, &lease, read).0, 404);
    assert_eq!(proof.resource(&view, &lease, late_save).0, 404);
    let response = reqwest::blocking::Client::new()
        .get(format!(
            "{origin}{}/{project}/resources/scene",
            project_path(organization, workspace)
        ))
        .bearer_auth(&bearer)
        .send()
        .unwrap();
    assert_eq!(response.status(), 200);
    let snapshot: Value = serde_json::from_slice(&response.bytes().unwrap()).unwrap();
    assert_eq!(snapshot["data"]["revision"], revision + 1);
}
