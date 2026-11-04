use super::*;

const CONFIG: &str = "# keep the host configuration\n\
default_model = \"custom/one\"\n\
providers \"custom\" {\n\
  api_key = env(\"ASTER_TEST_API_KEY\")\n\
  models \"one\" {}\n\
}\n\
search { timeout = 20 }\n";

fn press(state: &mut ModelConfigState, code: KeyCode) {
    state.handle_key(&KeyEvent::new(code, KeyModifiers::NONE));
}

#[test]
fn first_run_offers_three_unchecked_accounts_before_api_key_setup() {
    let mut state = ModelConfigState::from_draft(ConfigDraft::from_source(
        std::path::PathBuf::from("config.acl"),
        CONFIG.to_string(),
    ));
    state.focus_setup();
    assert_eq!(state.focus, Focus::Detail);
    assert_eq!(
        state.current_field(),
        Some(DetailField::ImportAccount(AccountImport::Codex))
    );
    assert!(state.draft.import_accounts.is_empty());
    assert!(!state.reveal_key);

    for (columns, rows) in [(120, 40), (80, 24)] {
        let area = ratatui::layout::Rect::new(0, 0, columns, rows);
        let mut buffer = ratatui::buffer::Buffer::empty(area);
        super::super::render::render_model_config(&mut buffer, area, &mut state, false);
        let screen = buffer
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        for text in [
            "Signed-in accounts",
            "[ ] Codex",
            "[ ] Cursor",
            "[ ] Grok",
            "API key",
            "Timeout",
        ] {
            assert!(
                screen.contains(text),
                "missing {text} at {columns}×{rows}: {screen}"
            );
        }
    }
}

#[test]
fn checkbox_selection_is_only_persisted_when_saved() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("用户 空间").join("config.acl");
    std::fs::create_dir(path.parent().unwrap()).unwrap();
    std::fs::write(&path, CONFIG).unwrap();
    let mut state = ModelConfigState::from_draft(ConfigDraft::load(path.clone()));
    state.focus_setup();
    press(&mut state, KeyCode::Char(' '));
    press(&mut state, KeyCode::Down);
    press(&mut state, KeyCode::Enter);
    press(&mut state, KeyCode::Down);
    press(&mut state, KeyCode::Char(' '));
    assert!(!state.is_editing());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), CONFIG);
    press(&mut state, KeyCode::Char('s'));

    let saved = std::fs::read_to_string(&path).unwrap();
    assert!(saved.starts_with(CONFIG));
    let config = aster_core::CodeConfig::from_acl(&saved).unwrap();
    assert_eq!(
        config.import_accounts,
        [
            AccountImport::Codex,
            AccountImport::Cursor,
            AccountImport::Grok
        ]
    );
    assert_eq!(config.default_model.as_deref(), Some("custom/one"));
    assert!(state.status.contains("Restart aster"));

    for import in AccountImport::ALL {
        state.focus_field(DetailField::ImportAccount(import));
        press(&mut state, KeyCode::Char(' '));
    }
    press(&mut state, KeyCode::Char('s'));
    let saved = std::fs::read_to_string(&path).unwrap();
    assert!(
        aster_core::CodeConfig::from_acl(&saved)
            .unwrap()
            .import_accounts
            .is_empty()
    );
    assert!(ConfigDraft::load(path).import_accounts.is_empty());
}

#[test]
fn existing_account_choices_are_loaded_and_duplicate_declarations_are_replaced() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("config.acl");
    let source =
        format!("{CONFIG}import_accounts = [\"grok\"]\nimport_accounts = [\"cursor\", \"grok\"]\n");
    std::fs::write(&path, &source).unwrap();
    let mut state = ModelConfigState::from_draft(ConfigDraft::load(path.clone()));
    assert_eq!(
        state.draft.import_accounts,
        [AccountImport::Grok, AccountImport::Cursor]
    );
    state.focus_setup();
    state.focus_field(DetailField::ImportAccount(AccountImport::Cursor));
    press(&mut state, KeyCode::Enter);
    press(&mut state, KeyCode::Char('s'));

    let saved = std::fs::read_to_string(path).unwrap();
    assert!(saved.starts_with(CONFIG));
    let document = aster_acl::parse_acl(&saved).unwrap();
    assert_eq!(
        document
            .blocks
            .iter()
            .filter(|block| block.name == "import_accounts")
            .count(),
        1
    );
    assert_eq!(
        aster_core::CodeConfig::from_acl(&saved)
            .unwrap()
            .import_accounts,
        [AccountImport::Grok]
    );
}

#[test]
fn invalid_account_imports_cannot_overwrite_the_existing_config() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("config.acl");
    for value in ["\"cursor\"", "[7]", "[\"keychain\"]"] {
        let source = format!("{CONFIG}import_accounts = {value}\n");
        std::fs::write(&path, &source).unwrap();
        let mut draft = ConfigDraft::load(path.clone());
        assert!(draft.load_error.is_some());
        draft.import_accounts.push(AccountImport::Codex);
        assert!(draft.save().is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), source);
    }
}

#[test]
fn a_default_from_a_selected_account_can_be_saved_without_copying_its_models() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("config.acl");
    let source = "import_accounts = [\"codex\"]\ndefault_model = \"codex/fixture-model\"\n";
    std::fs::write(&path, source).unwrap();
    let mut state = ModelConfigState::from_draft(ConfigDraft::load(path.clone()));
    press(&mut state, KeyCode::Right);
    assert_eq!(
        state.current_field(),
        Some(DetailField::ImportAccount(AccountImport::Codex))
    );
    state.focus_field(DetailField::ImportAccount(AccountImport::Grok));
    press(&mut state, KeyCode::Char(' '));
    press(&mut state, KeyCode::Char('s'));
    let saved = std::fs::read_to_string(&path).unwrap();
    let config = aster_core::CodeConfig::from_acl(&saved).unwrap();
    assert_eq!(
        config.import_accounts,
        [AccountImport::Codex, AccountImport::Grok]
    );
    assert_eq!(config.default_model.as_deref(), Some("codex/fixture-model"));
    assert!(config.providers.is_empty());

    state.focus_field(DetailField::ImportAccount(AccountImport::Codex));
    press(&mut state, KeyCode::Char(' '));
    press(&mut state, KeyCode::Char('s'));
    assert!(state.status.contains("Default provider"));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), saved);
}

#[test]
fn space_on_the_provider_list_does_not_change_account_consent() {
    let mut state = ModelConfigState::from_draft(ConfigDraft::from_source(
        std::path::PathBuf::from("config.acl"),
        CONFIG.to_string(),
    ));
    press(&mut state, KeyCode::Char(' '));
    assert!(state.draft.import_accounts.is_empty());
}
