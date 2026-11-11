use super::*;
use crate::app::{
    agent::{QueueEntryKind, QueuedPrompt},
    agent_view::{self, StashCause},
};

fn agent() -> AgentView {
    agent_view::test_agent_view(Some("recovery-test"), std::env::temp_dir())
}
fn draft(text: &str) -> StashedPrompt {
    let mut prompt = StashedPrompt::default();
    prompt.text = text.to_owned();
    prompt
}
fn snapshot(agent: &mut AgentView, directory: &std::path::Path) {
    storage::save_at(Snapshot::take(agent).unwrap(), directory).unwrap();
}
fn recover(agent: &mut AgentView, directory: &std::path::Path) {
    let session = agent.session.session_id.clone().unwrap();
    assert!(agent.begin_input_recovery(&session));
    assert!(agent.receive_saved_input(
        &session,
        storage::load_at(directory).map_err(|e| e.to_string())
    ));
}

#[test]
fn round_trip_preserves_queued_payloads_and_draft() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("input state 中文");
    std::fs::create_dir_all(&path).unwrap();
    let mut source = agent();
    source.session.enqueue_prompt("first".into());
    source.session.enqueue_prompt("second".into());
    source.session.pending_prompts[0].wire_blocks = Some(vec![acp::ContentBlock::Text(
        acp::TextContent::new("expanded payload"),
    )]);
    source.prompt.set_text("中文 draft\nsecond line");
    source.prompt_input_mode = PromptInputMode::Remember;
    snapshot(&mut source, &path);
    let mut target = agent();
    recover(&mut target, &path);
    assert_eq!(target.prompt.text(), "中文 draft\nsecond line");
    assert_eq!(target.prompt_input_mode, PromptInputMode::Remember);
    assert_eq!(target.session.pending_prompts.len(), 2);
    assert!(
        target
            .session
            .pending_prompts
            .iter()
            .all(|row| row.recovered)
    );
    assert_eq!(
        target.session.pending_prompts[0]
            .wire_blocks
            .as_ref()
            .unwrap()[0],
        acp::ContentBlock::Text(acp::TextContent::new("expanded payload"))
    );
}

#[test]
fn fresh_input_defers_saved_draft() {
    let directory = tempfile::tempdir().unwrap();
    let mut source = agent();
    source.prompt.set_text("old draft");
    snapshot(&mut source, directory.path());
    let mut target = agent();
    target.prompt.set_text("fresh draft");
    recover(&mut target, directory.path());
    assert_eq!(target.prompt.text(), "fresh draft");
    target.prompt.set_text("");
    assert!(target.restore_saved_draft());
    assert_eq!(target.prompt.text(), "old draft");
    assert!(!target.restore_saved_draft());
}

#[test]
fn deferred_drafts_survive_another_exit() {
    let directory = tempfile::tempdir().unwrap();
    let mut source = agent();
    source.prompt.set_text("old draft");
    snapshot(&mut source, directory.path());
    let mut current = agent();
    current.prompt.set_text("fresh draft");
    recover(&mut current, directory.path());
    snapshot(&mut current, directory.path());
    let mut resumed = agent();
    recover(&mut resumed, directory.path());
    assert_eq!(resumed.prompt.text(), "fresh draft");
    resumed.prompt.set_text("");
    assert!(resumed.restore_saved_draft());
    assert_eq!(resumed.prompt.text(), "old draft");
}

#[test]
fn restores_edit_and_stash_ownership() {
    let directory = tempfile::tempdir().unwrap();
    let mut source = agent();
    let id = source.session.enqueue_prompt("original row".into());
    source.prompt.set_text("edited row");
    source.prompt_mode = PromptMode::EditingQueued {
        id,
        original: "original row".into(),
        server_id: None,
        kind: QueueEntryKind::Prompt,
    };
    source.stashed_prompt = Some(draft("pre-edit draft"));
    source.prompt_stash = Some(PromptStashEntry {
        prompt: draft("parked draft"),
        input_mode: PromptInputMode::Bash,
        cause: StashCause::Chord,
        undo_armed: true,
    });
    snapshot(&mut source, directory.path());
    let mut target = agent();
    target.session.enqueue_prompt("fresh queue".into());
    recover(&mut target, directory.path());
    assert_eq!(target.prompt.text(), "edited row");
    assert!(
        matches!(target.prompt_mode, PromptMode::EditingQueued { id, .. } if id==target.session.pending_prompts[0].id)
    );
    assert_eq!(
        target.stashed_prompt.as_ref().unwrap().text,
        "pre-edit draft"
    );
    assert_eq!(
        target.prompt_stash.as_ref().unwrap().prompt.text,
        "parked draft"
    );
    assert_eq!(
        target.prompt_stash.as_ref().unwrap().input_mode,
        PromptInputMode::Bash
    );
}

#[test]
fn ignores_late_result_for_another_session() {
    let mut target = agent();
    let original = target.session.session_id.clone().unwrap();
    target.begin_input_recovery(&original);
    target.bind_session_id(acp::SessionId::new("other"));
    assert!(!target.receive_saved_input(&original, Ok(None)));
    assert!(target.session.pending_prompts.is_empty());
    assert!(!target.input_recovery.loading);
}

#[test]
fn recovered_rows_are_not_combined() {
    let mut target = agent();
    target
        .session
        .pending_prompts
        .push_back(QueuedPrompt::plain(1, "first", QueueEntryKind::Prompt));
    let mut held = QueuedPrompt::plain(2, "restored", QueueEntryKind::Prompt);
    held.recovered = true;
    target.session.pending_prompts.push_back(held);
    assert_eq!(
        target.session.dequeue_combined_prompt(None).unwrap().text,
        "first"
    );
    assert!(target.session.dequeue_combined_prompt(None).is_none());
    assert_eq!(target.session.pending_prompts.len(), 1);
}

#[test]
fn rejects_invalid_data_without_replacing_it() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("unsent-input.json");
    std::fs::write(&path, b"not json").unwrap();
    assert!(storage::load_at(directory.path()).is_err());
    let mut current = agent();
    current.prompt.set_text("fresh input");
    let mut captured = Snapshot::take(&mut current).unwrap();
    captured.merge_disk = true;
    assert!(storage::save_at(captured, directory.path()).is_err());
    assert_eq!(std::fs::read(path).unwrap(), b"not json");
}

#[test]
fn preserves_input_when_exit_precedes_loading() {
    let directory = tempfile::tempdir().unwrap();
    let mut original = agent();
    original.prompt.set_text("original input");
    snapshot(&mut original, directory.path());
    let mut interrupted = agent();
    let sid = interrupted.session.session_id.clone().unwrap();
    interrupted.begin_input_recovery(&sid);
    interrupted.prompt.set_text("new input");
    snapshot(&mut interrupted, directory.path());
    let mut resumed = agent();
    recover(&mut resumed, directory.path());
    assert_eq!(resumed.prompt.text(), "original input");
    resumed.prompt.set_text("");
    assert!(resumed.restore_saved_draft());
    assert_eq!(resumed.prompt.text(), "new input");
}

#[test]
fn completion_is_applied_once() {
    let mut target = agent();
    let sid = target.session.session_id.clone().unwrap();
    assert!(target.begin_input_recovery(&sid));
    assert!(target.receive_saved_input(&sid, Ok(None)));
    assert!(!target.receive_saved_input(&sid, Ok(None)));
    assert!(!target.begin_input_recovery(&sid));
}

#[test]
fn clears_consumed_snapshot() {
    let directory = tempfile::tempdir().unwrap();
    let mut original = agent();
    original.prompt.set_text("draft");
    snapshot(&mut original, directory.path());
    let mut resumed = agent();
    recover(&mut resumed, directory.path());
    resumed.prompt.set_text("");
    snapshot(&mut resumed, directory.path());
    assert!(storage::load_at(directory.path()).unwrap().is_none());
}

#[test]
fn images_survive_without_staging_files() {
    use base64::Engine as _;
    let bytes = base64::engine::general_purpose::STANDARD.decode("iVBORw0KGgoAAAANSUhEUgAAAAgAAAAICAIAAABLbSncAAAAEUlEQVR4nGMQiVqAFTEMLQkAPU5DgULiSUcAAAAASUVORK5CYII=").unwrap();
    let directory = tempfile::tempdir().unwrap();
    let mut source = agent();
    let image = crate::prompt_images::from_clipboard_data(&crate::clipboard::ImageData {
        data: bytes.clone(),
        mime_type: "image/png".into(),
    });
    source.prompt.insert_image(image).unwrap();
    source.session.enqueue_prompt("queued image".into());
    source.session.pending_prompts[0].images = source.prompt.images.clone();
    let text = source.prompt.text().to_owned();
    snapshot(&mut source, directory.path());
    let mut resumed = agent();
    recover(&mut resumed, directory.path());
    assert_eq!(resumed.prompt.text(), text);
    assert_eq!(resumed.prompt.images.len(), 1);
    assert!(resumed.prompt.images[0].encoded_bytes.is_none());
    assert_eq!(
        crate::prompt_images::load_for_send(&resumed.prompt.images[0])
            .unwrap()
            .0,
        bytes
    );
    assert_eq!(
        crate::prompt_images::load_for_send(&resumed.session.pending_prompts[0].images[0])
            .unwrap()
            .0,
        bytes
    );
    assert!(!resumed.prompt.textarea.elements().is_empty());
}

#[test]
fn retains_styled_chips() {
    use ratatui::{
        style::{Color, Style},
        text::{Line, Span},
    };
    let directory = tempfile::tempdir().unwrap();
    let mut source = agent();
    source.session.enqueue_prompt("pasted content".into());
    let display = Line::from(Span::styled(
        "paste",
        Style::default().fg(Color::Rgb(1, 2, 3)),
    ));
    source.session.pending_prompts[0]
        .chip_elements
        .push(crate::app::agent::ChipElement {
            range: 0..6,
            kind: aster_ratatui_textarea::ElementKind(3),
            display: Some(display.clone()),
        });
    snapshot(&mut source, directory.path());
    let mut target = agent();
    recover(&mut target, directory.path());
    assert_eq!(
        target.session.pending_prompts[0].chip_elements[0]
            .display
            .as_ref(),
        Some(&display)
    );
}

#[test]
fn explicit_send_releases_only_selected_payload() {
    use crate::app::{
        actions::{Action, Effect},
        app_view::{ActiveView, InputOutcome},
    };
    for kind in [QueueEntryKind::Prompt, QueueEntryKind::BashCommand] {
        let mut app = crate::app::app_view::tests::test_app_with_agent();
        let ActiveView::Agent(id) = app.active_view else {
            panic!("expected agent");
        };
        let target = app.agents.get_mut(&id).unwrap();
        let mut first = QueuedPrompt::plain(1, "first", kind);
        first.recovered = true;
        let mut second = QueuedPrompt::plain(2, "second", QueueEntryKind::Prompt);
        second.recovered = true;
        target.session.pending_prompts.extend([first, second]);
        target.sync_queue_pane();
        let InputOutcome::Action(action @ Action::DrainQueue) = target.force_interject_queue_row(1)
        else {
            panic!("expected drain");
        };
        let effects = crate::app::dispatch::dispatch(action, &mut app);
        assert!(effects.iter().any(|effect| matches!(
            effect,
            Effect::SendPrompt { .. } | Effect::SendBashCommand { .. }
        )));
        let target = app.agents.get(&id).unwrap();
        assert_eq!(target.session.pending_prompts.len(), 1);
        assert!(target.session.pending_prompts[0].recovered);
    }
}

#[test]
fn readonly_rows_cannot_be_released() {
    use crate::{
        app::{actions::Action, app_view::InputOutcome},
        views::queue_mutation::QueueMutation,
    };
    let mut target = agent();
    let mut row = QueuedPrompt::plain(1, "read only", QueueEntryKind::Prompt);
    row.recovered = true;
    target.session.pending_prompts.push_back(row);
    target.queue.set_mutation(QueueMutation::ReadOnly);
    target.sync_queue_pane();
    assert!(!matches!(
        target.force_interject_queue_row(1),
        InputOutcome::Action(Action::DrainQueue)
    ));
    assert!(target.session.pending_prompts[0].recovered);
}

#[test]
fn diagnostics_hide_input_content() {
    let mut source = agent();
    source
        .session
        .enqueue_prompt("synthetic-queued-secret".into());
    source.prompt.set_text("synthetic-draft-secret");
    let snapshot = Snapshot::take(&mut source).unwrap();
    let debug = format!("{:?} {:?}", snapshot.saved, snapshot.saved.contexts);
    assert!(!debug.contains("synthetic-"));
}

#[test]
fn orphaned_edit_returns_as_draft() {
    let directory = tempfile::tempdir().unwrap();
    let mut source = agent();
    source.prompt.set_text("edited content");
    source.prompt_mode = PromptMode::EditingQueued {
        id: 1,
        original: "old content".into(),
        server_id: Some("no-longer-queued".into()),
        kind: QueueEntryKind::Prompt,
    };
    source.stashed_prompt = Some(draft("original composer"));
    snapshot(&mut source, directory.path());
    let mut target = agent();
    recover(&mut target, directory.path());
    assert_eq!(target.prompt.text(), "edited content");
    assert_eq!(target.prompt_mode, PromptMode::Normal);
    target.prompt.set_text("");
    assert!(target.restore_saved_draft());
    assert_eq!(target.prompt.text(), "original composer");
}
