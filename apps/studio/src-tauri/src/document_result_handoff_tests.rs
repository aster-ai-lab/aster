#![cfg(test)]

use super::{
    ResultFile, ResultStatus, check_window, finish_result, stage_result, take_result, validate,
};
use base64::Engine;

#[test]
fn only_pdf_requests_and_main_handles_the_save() {
    assert!(check_window("applet-pdf", "applet-pdf", "applet-pdf").is_ok());
    assert!(check_window("main", "main", "main").is_ok());
    for label in ["main", "applet-review", "applet-leave", "other"] {
        assert!(check_window(label, label, "applet-pdf").is_err());
    }
    assert!(check_window("applet-pdf", "applet-pdf", "main").is_err());
    assert!(check_window("main", "studio-embed", "main").is_err());
    assert!(check_window("other", "main", "main").is_err());
}

#[test]
fn result_never_supplies_a_destination_path_or_empty_bytes() {
    let mut file = ResultFile {
        name: "满江红.pdf".into(),
        media_type: "application/pdf".into(),
        bytes_base64: base64::engine::general_purpose::STANDARD.encode(b"%PDF-1.7"),
    };
    assert!(validate(&file).is_ok());
    for name in [
        "../notes.pdf",
        "/tmp/notes.pdf",
        "a\\notes.pdf",
        ".hidden",
        "..",
    ] {
        file.name = name.into();
        assert!(validate(&file).is_err());
    }
    file.name = "满江红.pdf".into();
    file.bytes_base64.clear();
    assert!(validate(&file).is_err());
    file.bytes_base64 = "not base64".into();
    assert!(validate(&file).is_err());
}

#[test]
fn window_reload_recovers_pending_bytes_and_next_save_keeps_previous_receipt() {
    let file = ResultFile {
        name: "满江红.pdf".into(),
        media_type: "application/pdf".into(),
        bytes_base64: base64::engine::general_purpose::STANDARD.encode(b"%PDF-1.7"),
    };
    let mut transfers = Vec::new();
    let first = stage_result(&mut transfers, file.clone()).unwrap();
    assert!(finish_result(&mut transfers, &first, None).is_err());
    let original = take_result(&mut transfers).unwrap();
    let recovered = take_result(&mut transfers).unwrap();
    assert_eq!(recovered.id, original.id);
    assert_eq!(recovered.file.bytes_base64, file.bytes_base64);
    assert!(stage_result(&mut transfers, file.clone()).is_err());
    assert!(finish_result(&mut transfers, "wrong-request", None).is_err());
    finish_result(&mut transfers, &first, Some("项目资料 / 导出".into())).unwrap();
    finish_result(&mut transfers, &first, Some("项目资料 / 导出".into())).unwrap();
    assert!(finish_result(&mut transfers, &first, None).is_err());
    let second = stage_result(&mut transfers, file).unwrap();
    assert_ne!(first, second);
    assert!(
        matches!(&transfers[0].status, ResultStatus::Saved { location } if location == "项目资料 / 导出")
    );
    assert!(transfers[0].request.file.bytes_base64.is_empty());
    assert_eq!(take_result(&mut transfers).unwrap().id, second);
    finish_result(&mut transfers, &second, None).unwrap();
    assert!(matches!(transfers[1].status, ResultStatus::Cancelled));
    assert!(take_result(&mut transfers).is_none());
}

#[test]
fn result_save_ipc_survives_a_workspace_child_without_granting_it_receiver_access() {
    use tauri::{LogicalPosition, LogicalSize, Manager, WebviewUrl};
    let app = tauri::test::mock_builder()
        .invoke_handler(tauri::generate_handler![
            super::stage_document_result_save,
            super::take_document_result_save,
            super::finish_document_result_save,
            super::document_result_save_status
        ])
        .build(crate::studio_context())
        .unwrap();
    let page: url::Url = crate::main_renderer_url(false).parse().unwrap();
    for label in ["main", "applet-pdf"] {
        tauri::WebviewWindowBuilder::new(&app, label, WebviewUrl::External(page.clone()))
            .build()
            .unwrap();
    }
    let child = app
        .get_window("main")
        .unwrap()
        .add_child(
            tauri::webview::WebviewBuilder::new("studio-embed", WebviewUrl::External(page)),
            LogicalPosition::new(20.0, 40.0),
            LogicalSize::new(400.0, 600.0),
        )
        .unwrap();
    assert!(app.get_webview_window("main").is_none());
    let main = app.get_webview("main").unwrap();
    let pdf = app.get_webview("applet-pdf").unwrap();
    let invoke = |view: &tauri::Webview<tauri::test::MockRuntime>, command: &str, body| {
        tauri::test::get_ipc_response(
            &std::sync::Arc::new(view.clone()),
            tauri::webview::InvokeRequest {
                cmd: command.into(),
                callback: tauri::ipc::CallbackFn(1),
                error: tauri::ipc::CallbackFn(2),
                url: view.url().unwrap(),
                body: tauri::ipc::InvokeBody::Json(body),
                headers: Default::default(),
                invoke_key: tauri::test::INVOKE_KEY.into(),
            },
        )
    };
    let id = invoke(
        &pdf,
        "stage_document_result_save",
        serde_json::json!({"file":{
            "name":"result.pdf","mediaType":"application/pdf",
            "bytesBase64":base64::engine::general_purpose::STANDARD.encode(b"%PDF-1.7")
        }}),
    )
    .unwrap()
    .deserialize::<String>()
    .unwrap();
    assert!(invoke(&child, "take_document_result_save", serde_json::json!({})).is_err());
    let request = invoke(&main, "take_document_result_save", serde_json::json!({}))
        .unwrap()
        .deserialize::<serde_json::Value>()
        .unwrap();
    assert_eq!(request["id"], id);
    invoke(
        &main,
        "finish_document_result_save",
        serde_json::json!({
            "id":id,"location":"saved/result.pdf"
        }),
    )
    .unwrap();
    let status = invoke(
        &pdf,
        "document_result_save_status",
        serde_json::json!({"id":id}),
    )
    .unwrap()
    .deserialize::<serde_json::Value>()
    .unwrap();
    assert_eq!(
        status,
        serde_json::json!({"state":"saved","location":"saved/result.pdf"})
    );
}

#[test]
fn shipped_authority_keeps_result_commands_on_their_window_roles() {
    use tauri::ipc::Origin;
    let mut context = crate::studio_context::<tauri::test::MockRuntime>();
    let authority = context.runtime_authority_mut();
    let page = Origin::Remote {
        url: "http://127.0.0.1:4318/windows/main/index.html"
            .parse()
            .unwrap(),
    };
    for (command, allowed, denied) in [
        ("stage_document_result_save", "applet-pdf", "main"),
        ("document_result_save_status", "applet-pdf", "main"),
        ("take_document_result_save", "main", "applet-pdf"),
        ("finish_document_result_save", "main", "applet-pdf"),
    ] {
        assert!(
            authority
                .resolve_access(command, allowed, allowed, &page)
                .is_some()
        );
        assert!(
            authority
                .resolve_access(command, denied, denied, &page)
                .is_none()
        );
        assert!(
            authority
                .resolve_access(command, "other", "other", &page)
                .is_none()
        );
    }
}
