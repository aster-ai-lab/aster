#![cfg(test)]

use super::*;

fn entries(code_backend: bool) -> Vec<ShortcutsHelpEntry> {
    build_entries_for_backend(
        &[When::AgentScreen, When::PromptFocused],
        &ActionRegistry::defaults(),
        false,
        code_backend,
    )
}

#[test]
fn code_paste_help_matches_text_capability() {
    let entries = entries(true);
    let (item, help) = entries
        .iter()
        .find_map(|entry| match entry {
            ShortcutsHelpEntry::Hint {
                item, long_help, ..
            } if item.label == "paste" => Some((item, long_help.unwrap())),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        item.description.as_deref(),
        Some("Paste text from the clipboard")
    );
    assert!(help.contains("accepts text only"));
    assert!(!help.contains("clipboard images"));
}

#[test]
fn code_history_help_points_to_working_inline_recall() {
    let entries = entries(true);
    let help = entries
        .iter()
        .find_map(|entry| match entry {
            ShortcutsHelpEntry::Hint {
                item, long_help, ..
            } if item.label == "history" => *long_help,
            _ => None,
        })
        .unwrap();
    assert!(help.contains("Press Up on an empty prompt"));
    assert!(!help.contains("/history"));
}

#[test]
fn code_sessions_help_does_not_offer_hidden_commands_or_dashboard() {
    let entries = entries(true);
    let help = entries
        .iter()
        .find_map(|entry| match entry {
            ShortcutsHelpEntry::Hint {
                action_id: Some(ActionId::OpenSessions),
                long_help,
                ..
            } => *long_help,
            _ => None,
        })
        .unwrap();
    assert!(help.contains("Select one to reattach to its full history"));
    assert!(!help.contains("/resume"));
    assert!(!help.contains("Dashboard"));
}

#[test]
fn other_backends_keep_their_existing_shortcut_help() {
    let entries = entries(false);
    for label in ["paste", "history"] {
        let help = entries
            .iter()
            .find_map(|entry| match entry {
                ShortcutsHelpEntry::Hint {
                    item, long_help, ..
                } if item.label == label => *long_help,
                _ => None,
            })
            .unwrap();
        assert_eq!(
            help,
            if label == "paste" {
                PASTE_LONG_HELP
            } else {
                HISTORY_LONG_HELP
            }
        );
    }
    let registry = ActionRegistry::defaults();
    let sessions = registry.find(ActionId::OpenSessions).unwrap();
    assert!(entries.iter().any(|entry| matches!(entry,
        ShortcutsHelpEntry::Hint { action_id: Some(ActionId::OpenSessions), long_help, .. }
        if *long_help == sessions.long_help
    )));
}

#[test]
fn code_help_does_not_offer_the_unavailable_direct_shell_entry() {
    assert!(!entries(true).iter().any(|entry| matches!(
        entry,
        ShortcutsHelpEntry::Hint {
            action_id: Some(ActionId::BashMode),
            ..
        }
    )));
    assert!(entries(false).iter().any(|entry| matches!(
        entry,
        ShortcutsHelpEntry::Hint {
            action_id: Some(ActionId::BashMode),
            ..
        }
    )));
}

#[test]
fn code_send_now_help_describes_cancellation_and_resource_release() {
    let entries = entries(true);
    let help = entries
        .iter()
        .find_map(|entry| match entry {
            ShortcutsHelpEntry::Hint {
                action_id: Some(ActionId::InterjectPrompt),
                long_help,
                ..
            } => *long_help,
            _ => None,
        })
        .unwrap();
    assert!(help.contains("Cancels the current turn"));
    assert!(help.contains("release its resources"));
    assert!(!help.contains("without cancelling"));
}
