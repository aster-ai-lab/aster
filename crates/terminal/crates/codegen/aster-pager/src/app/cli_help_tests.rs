use super::*;
use clap::CommandFactory;

#[test]
fn help_uses_aster_names() {
    let mut root = PagerArgs::command();
    let help = root.render_long_help().to_string();
    assert!(!help.to_ascii_lowercase().contains("grok"));
    let dashboard = root.find_subcommand_mut("dashboard").unwrap();
    let help = dashboard.render_long_help().to_string();
    assert!(!help.to_ascii_lowercase().contains("grok"));
    assert!(!help.contains("config.toml"));
    assert!(help.contains("Dashboard"));
}

#[test]
fn sandbox_keeps_its_environment_binding() {
    let command = PagerArgs::command();
    let argument = command
        .get_arguments()
        .find(|argument| argument.get_id() == "sandbox")
        .unwrap();
    assert_eq!(
        argument.get_env(),
        Some(std::ffi::OsStr::new("GROK_SANDBOX"))
    );
}
