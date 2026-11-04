use super::BraceGlob;

fn expansions(pattern: &str) -> Vec<String> {
    BraceGlob::new(pattern).unwrap().expansions().to_vec()
}

#[test]
fn brace_groups_expand_in_order_including_nested_and_multiple() {
    assert_eq!(expansions("*.{txt,md}"), ["*.txt", "*.md"]);
    assert_eq!(expansions("*.{md}"), ["*.md"]);
    assert_eq!(
        expansions("{a,b}/*.{x,y}"),
        ["a/*.x", "a/*.y", "b/*.x", "b/*.y"]
    );
    assert_eq!(expansions("*.{ts,{tsx,js}}"), ["*.ts", "*.tsx", "*.js"]);
    assert_eq!(expansions("*.{ts,}"), ["*.ts", "*."]);
}

#[test]
fn unbalanced_braces_stay_literal() {
    assert_eq!(expansions("a{b"), ["a{b"]);
    assert_eq!(expansions("a}b"), ["a}b"]);
}

#[test]
fn matches_any_alternative_and_rejects_the_rest() {
    let glob = BraceGlob::new("*.{txt,md}").unwrap();
    assert!(glob.matches("a.txt"));
    assert!(glob.matches("c.md"));
    assert!(!glob.matches("e.rs"));
}

#[test]
fn runaway_expansion_is_rejected() {
    let pattern = "{a,b}".repeat(9);
    assert!(BraceGlob::new(&pattern).is_err());
}
