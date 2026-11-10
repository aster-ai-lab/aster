use super::{ThemeKind, canonical_name, display_name_for_canonical};

#[test]
fn legacy_names_resolve_to_aster() {
    for (legacy, canonical, display, kind) in [
        (
            "groknight",
            "aster-night",
            "Aster Night",
            ThemeKind::AsterNight,
        ),
        ("grokday", "aster-day", "Aster Day", ThemeKind::AsterDay),
    ] {
        assert_eq!(ThemeKind::from_name(legacy), Some(kind));
        assert_eq!(canonical_name(legacy), Some(canonical));
        assert_eq!(kind.display_name(), canonical);
        assert_eq!(display_name_for_canonical(legacy), display);
        assert_eq!(display_name_for_canonical(canonical), display);
    }
}

#[test]
fn public_names_use_current_brand() {
    for kind in ThemeKind::ALL {
        assert!(!kind.display_name().to_lowercase().contains("grok"));
    }
}
