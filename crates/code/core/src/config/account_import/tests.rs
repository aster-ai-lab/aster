use super::AccountImport;
use crate::config::{CodeConfig, LayerAcl};

#[test]
fn the_closed_set_round_trips_and_names_itself_on_refusal() {
    for import in AccountImport::ALL {
        assert_eq!(AccountImport::parse(import.as_str()), Ok(import));
    }
    let error = AccountImport::parse("Cursor").unwrap_err();
    assert!(error.contains("closed set: grok, cursor, codex"), "{error}");
    assert_eq!(
        AccountImport::parse_list("cursor, grok,cursor"),
        Ok(vec![AccountImport::Cursor, AccountImport::Grok])
    );
    assert!(AccountImport::parse_list("cursor,").is_err());
}

#[test]
fn an_acl_without_import_accounts_imports_nothing() {
    let config = CodeConfig::from_acl("default_model = \"mock/m\"\n").unwrap();
    assert!(config.import_accounts.is_empty());
}

#[test]
fn retired_cc_switch_import_is_rejected_at_configuration_boundaries() {
    assert!(AccountImport::parse("cc-switch").is_err());
    assert!(AccountImport::parse_list("cursor,cc-switch").is_err());
    assert!(serde_json::from_str::<AccountImport>("\"cc-switch\"").is_err());
    assert!(CodeConfig::from_acl("import_accounts = [\"cc-switch\"]\n").is_err());
}

#[test]
fn import_accounts_parses_the_closed_set_and_refuses_unknown_sources() {
    let config = CodeConfig::from_acl("import_accounts = [\"cursor\", \"codex\"]\n").unwrap();
    assert_eq!(
        config.import_accounts,
        vec![AccountImport::Cursor, AccountImport::Codex]
    );
    let error = CodeConfig::from_acl("import_accounts = [\"keychain\"]\n")
        .unwrap_err()
        .to_string();
    assert!(error.contains("closed set: grok, cursor, codex"), "{error}");
    assert!(CodeConfig::from_acl("import_accounts = \"cursor\"\n").is_err());
}

#[test]
fn layers_append_import_accounts_without_duplicates() {
    let config = CodeConfig::from_acl_layers(&[
        "import_accounts = [\"grok\"]\n",
        "import_accounts = [\"cursor\", \"grok\"]\n",
    ])
    .unwrap();
    assert_eq!(
        config.import_accounts,
        vec![AccountImport::Grok, AccountImport::Cursor]
    );
}

#[test]
fn an_untrusted_layer_cannot_opt_the_host_into_an_account_import() {
    let user = "import_accounts = [\"grok\"]\n";
    let workspace = "import_accounts = [\"cursor\"]\n";
    let mut config = CodeConfig::from_acl_layers(&[user, workspace]).unwrap();
    let withheld = crate::config::withhold_untrusted_host_authoritative_blocks(
        &mut config,
        &[
            LayerAcl {
                trusted: true,
                acl: user.into(),
            },
            LayerAcl {
                trusted: false,
                acl: workspace.into(),
            },
        ],
    );
    assert_eq!(withheld, vec!["import_accounts"]);
    assert_eq!(config.import_accounts, vec![AccountImport::Grok]);
}
