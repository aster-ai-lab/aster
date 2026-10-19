#![cfg(test)]

use super::*;
use std::sync::{Arc, Mutex};
use std::thread;

const MEMBER: &str = "33333333-3333-4333-8333-333333333333";
const OTHER: &str = "44444444-4444-4444-8444-444444444444";
const PACKAGE: &str = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

struct Fixture {
    state: tempfile::TempDir,
    workspace: tempfile::TempDir,
    job: Arc<Mutex<Value>>,
    manifest: Arc<Mutex<Value>>,
    body: Arc<Mutex<Vec<u8>>>,
    source_body: Arc<Mutex<Vec<u8>>>,
}
impl Fixture {
    fn new() -> Self {
        let state = tempfile::tempdir().unwrap();
        let workspace = tempfile::tempdir().unwrap();
        let source = include_bytes!("../../tests/fixtures/paperloom/two-pages.pdf");
        fs::write(workspace.path().join("paper.pdf"), source).unwrap();
        let mut bytes = source.to_vec();
        bytes.extend_from_slice(b"\n% verified artifact fixture\n");
        fs::write(workspace.path().join("translated.pdf"), &bytes).unwrap();
        let fixture = Self {
            state,
            workspace,
            body: Arc::new(Mutex::new(bytes.clone())),
            source_body: Arc::new(Mutex::new(source.to_vec())),
            job: Arc::new(Mutex::new(json!({"job_id":"test-job","status":"succeeded",
                "request_payload":{"source":{"upload_id":"20261016-opaque-upload"}}}))),
            manifest: Arc::new(Mutex::new(json!({"job_id":"test-job", "items":[{
                "artifact_key":"translated-pdf","file_name":"translated.pdf", "checksum":digest(&bytes),
                "ready":true,"attempt":1 }, {
                "artifact_key":"source_pdf","artifact_kind":"file","file_name":"paper.pdf","content_type":"application/pdf",
                "checksum":digest(source),"ready":true,"attempt":1,"size_bytes":source.len()
            }]}))),
        };
        fixture.sync();
        fixture
    }
    fn sync(&self) {
        let jobs_dir = self
            .state
            .path()
            .join("paperloom-native")
            .join(MEMBER)
            .join("jobs");
        if jobs_dir.exists() {
            fs::remove_dir_all(&jobs_dir).unwrap();
        }
        let files = [
            ("translated-pdf", self.body.lock().unwrap().clone()),
            ("source_pdf", self.source_body.lock().unwrap().clone()),
        ];
        let borrowed: Vec<(&str, &[u8])> = files
            .iter()
            .map(|(key, bytes)| (*key, bytes.as_slice()))
            .collect();
        super::super::native::install_fixture(
            self.state.path(),
            MEMBER,
            &self.job.lock().unwrap(),
            &self.manifest.lock().unwrap(),
            &borrowed,
        )
        .unwrap();
    }
    fn request(&self) -> Registration {
        let root = self
            .workspace
            .path()
            .canonicalize()
            .unwrap()
            .to_str()
            .unwrap()
            .to_owned();
        let source_path = format!("{root}/paper.pdf");
        let source = WorkspaceSnapshots::default()
            .read(&root, &source_path)
            .unwrap();
        Registration {
            root: root.clone(),
            source_path,
            source_revision: source.revision,
            source_digest: hex_digest(&source.bytes),
            saved_path: format!("{root}/translated.pdf"),
            job_id: "test-job".into(),
            artifact_key: "translated-pdf".into(),
            attempt: 1,
            checksum: digest(&self.body.lock().unwrap()),
        }
    }
}
fn submit(fixture: &Fixture, request: Registration) -> Result<Value> {
    fixture.sync();
    register(fixture.state.path(), MEMBER, PACKAGE, request)
}
fn query(root: &str, path: &str) -> String {
    url::form_urlencoded::Serializer::new(String::new())
        .append_pair("root", root)
        .append_pair("path", path)
        .finish()
}

#[test]
fn verified_copy_is_idempotent_durable_and_immutable_after_workspace_edits() {
    let f = Fixture::new();
    let request = f.request();
    let root = request.root.clone();
    let path = request.saved_path.clone();
    let first = submit(&f, request).unwrap();
    let proof: ArtifactProof = serde_json::from_value(first).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let artifact_path = ledger(f.state.path(), MEMBER, &root, &path)
            .unwrap()
            .join("proof-artifacts")
            .join(&proof.session_id)
            .join(&proof.artifacts[0].id);
        assert_eq!(
            fs::metadata(artifact_path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    let again = register(f.state.path(), MEMBER, PACKAGE, f.request()).unwrap();
    assert_eq!(again["messageId"], proof.message_id);
    assert_eq!(
        proof.artifacts[0].origin.as_ref().unwrap().source_digest,
        format!(
            "sha256:{}",
            hex_digest(include_bytes!(
                "../../tests/fixtures/paperloom/two-pages.pdf"
            ))
        )
    );
    assert_eq!(
        proof.artifacts[0].origin.as_ref().unwrap().package_digest,
        PACKAGE
    );
    fs::write(&path, b"later user edit").unwrap();
    let response = route(
        f.state.path(),
        MEMBER,
        PACKAGE,
        "GET",
        &["provenance", &proof.message_id, "snapshot"],
        &query(&root, &path),
        b"",
    )
    .unwrap();
    assert_eq!(response.0, 200);
    let bytes: Value = serde_json::from_slice(&response.3).unwrap();
    assert_eq!(
        base64::engine::general_purpose::STANDARD
            .decode(bytes["data"]["data"].as_str().unwrap())
            .unwrap(),
        *f.body.lock().unwrap()
    );
    let history = route(
        f.state.path(),
        MEMBER,
        PACKAGE,
        "GET",
        &["provenance", &proof.message_id, "history"],
        &query(&root, &path),
        b"",
    )
    .unwrap();
    assert_eq!(history.0, 200);
    assert_eq!(
        serde_json::from_slice::<Value>(&history.3).unwrap()["data"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let denied = route(
        f.state.path(),
        OTHER,
        PACKAGE,
        "GET",
        &["provenance", &proof.message_id],
        &query(&root, &path),
        b"",
    )
    .unwrap();
    assert_eq!(denied.0, 404);
    let dir = ledger(f.state.path(), MEMBER, &root, &path).unwrap();
    let record = dir.join(format!("{}.json", proof.message_id));
    let mut tampered = serde_json::to_value(proof).unwrap();
    tampered["artifacts"][0]["origin"]["attempt"] = json!(2);
    fs::write(record, serde_json::to_vec(&tampered).unwrap()).unwrap();
    assert_eq!(
        route(
            f.state.path(),
            MEMBER,
            PACKAGE,
            "GET",
            &[
                "provenance",
                tampered["messageId"].as_str().unwrap(),
                "snapshot"
            ],
            &query(&root, &path),
            b""
        )
        .unwrap()
        .0,
        409
    );
}

#[test]
fn registration_rechecks_source_revision_and_actual_saved_bytes() {
    let f = Fixture::new();
    let request = f.request();
    fs::write(&request.source_path, b"%PDF-new revision").unwrap();
    assert_eq!(submit(&f, request).unwrap_err(), (409, "source_changed"));
    let f = Fixture::new();
    let request = f.request();
    fs::write(&request.saved_path, b"different bytes").unwrap();
    assert_eq!(
        submit(&f, request).unwrap_err(),
        (409, "saved_file_changed")
    );
}

#[test]
fn foreign_jobs_stale_attempts_and_unverified_manifests_refuse_registration() {
    let f = Fixture::new();
    f.job.lock().unwrap()["job_id"] = json!("foreign");
    assert_eq!(
        submit(&f, f.request()).unwrap_err(),
        (502, "processor_unavailable")
    );
    for (key, value) in [
        ("attempt", json!(2)),
        ("ready", json!(false)),
        (
            "checksum",
            json!("sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
        ),
        ("file_name", json!("other.pdf")),
    ] {
        let f = Fixture::new();
        f.manifest.lock().unwrap()["items"][0][key] = value;
        assert_eq!(
            submit(&f, f.request()).unwrap_err(),
            (409, "artifact_changed")
        );
    }
    let f = Fixture::new();
    let request = f.request();
    f.body
        .lock()
        .unwrap()
        .extend_from_slice(b"tampered response");
    assert_eq!(submit(&f, request).unwrap_err(), (409, "artifact_changed"));
}

#[test]
fn job_source_bytes_bind_opaque_upload_ids_and_render_reuse() {
    let f = Fixture::new();
    assert_ne!(
        f.job.lock().unwrap()["request_payload"]["source"]["upload_id"],
        f.request().source_digest
    );
    let proof = submit(&f, f.request()).unwrap();
    assert_eq!(
        proof["artifacts"][0]["origin"]["sourceDigest"],
        format!("sha256:{}", f.request().source_digest)
    );
    let f = Fixture::new();
    f.job.lock().unwrap()["request_payload"]["source"] =
        json!({"upload_id":"", "artifact_job_id":"previous-job"});
    assert!(submit(&f, f.request()).is_ok());
}

#[test]
fn source_manifest_and_downloaded_source_must_both_match_the_workspace_pdf() {
    for (key, value) in [
        ("checksum", Value::Null),
        (
            "checksum",
            json!("sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
        ),
        ("ready", json!(false)),
        ("attempt", json!(2)),
    ] {
        let f = Fixture::new();
        f.manifest.lock().unwrap()["items"][1][key] = value;
        assert_eq!(
            submit(&f, f.request()).unwrap_err(),
            (409, "artifact_source_mismatch")
        );
    }
    let f = Fixture::new();
    f.source_body
        .lock()
        .unwrap()
        .extend_from_slice(b"altered on server");
    assert_eq!(
        submit(&f, f.request()).unwrap_err(),
        (409, "artifact_source_mismatch")
    );
    let f = Fixture::new();
    let duplicate = f.manifest.lock().unwrap()["items"][1].clone();
    f.manifest.lock().unwrap()["items"]
        .as_array_mut()
        .unwrap()
        .push(duplicate);
    assert_eq!(
        submit(&f, f.request()).unwrap_err(),
        (409, "artifact_source_mismatch")
    );
}

#[test]
fn workspace_containment_symlinks_and_source_overwrite_refuse() {
    let f = Fixture::new();
    let mut request = f.request();
    request.saved_path = request.source_path.clone();
    assert_eq!(submit(&f, request).unwrap_err(), (409, "source_overwrite"));
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("out.pdf"), b"outside").unwrap();
    let mut request = f.request();
    request.saved_path = outside.path().join("out.pdf").to_str().unwrap().into();
    assert_eq!(
        submit(&f, request).unwrap_err(),
        (404, "workspace_source_failed")
    );
    #[cfg(unix)]
    {
        let mut request = f.request();
        let link = f.workspace.path().join("out.pdf");
        std::os::unix::fs::symlink(outside.path().join("out.pdf"), &link).unwrap();
        request.saved_path = link.to_str().unwrap().into();
        assert_eq!(
            submit(&f, request).unwrap_err(),
            (404, "workspace_source_failed")
        );
    }
}

#[test]
fn ready_partial_output_records_failed_job_honestly() {
    let f = Fixture::new();
    f.job.lock().unwrap()["status"] = json!("failed");
    let proof = submit(&f, f.request()).unwrap();
    assert_eq!(proof["artifacts"][0]["origin"]["jobStatus"], "failed");
    assert_eq!(proof["tools"][0]["name"], "paperloom.verify-saved-copy");
}

#[test]
fn source_lookup_restores_saved_receipts_without_contacting_the_processor() {
    let f = Fixture::new();
    let request = f.request();
    let root = request.root.clone();
    let path = request.source_path.clone();
    let revision = request.source_revision.clone();
    let source_digest = request.source_digest.clone();
    let proof = submit(&f, request).unwrap();
    let params = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("root", &root)
        .append_pair("sourcePath", &path)
        .append_pair("sourceRevision", &revision)
        .append_pair("sourceDigest", &source_digest)
        .finish();
    let found = lookup_source(f.state.path(), MEMBER, &params).unwrap();
    assert_eq!(found.as_array().unwrap().len(), 1);
    assert_eq!(found[0]["messageId"], proof["messageId"]);
    assert_eq!(
        lookup_source(f.state.path(), OTHER, &params).unwrap(),
        json!([])
    );
    fs::write(&path, b"%PDF-changed").unwrap();
    assert_eq!(
        lookup_source(f.state.path(), MEMBER, &params).unwrap_err(),
        (409, "source_changed")
    );
}

#[test]
fn simultaneous_registration_publishes_one_receipt_for_the_same_saved_revision() {
    let f = Fixture::new();
    let state = f.state.path().to_path_buf();
    let first = f.request();
    let second = f.request();
    let barrier = Arc::new(std::sync::Barrier::new(2));
    let left = barrier.clone();
    let state_left = state.clone();
    let a = thread::spawn(move || {
        left.wait();
        register(&state_left, MEMBER, PACKAGE, first).unwrap()
    });
    let b = thread::spawn(move || {
        barrier.wait();
        register(&state, MEMBER, PACKAGE, second).unwrap()
    });
    assert_eq!(
        a.join().unwrap()["messageId"],
        b.join().unwrap()["messageId"]
    );
}
