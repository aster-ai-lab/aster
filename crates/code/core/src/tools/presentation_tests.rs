#![cfg(test)]

use super::*;

#[test]
fn modes_parse_exactly_and_profiles_keep_the_versioned_identity() {
    for mode in [
        ToolPresentationModeV1::Adaptive,
        ToolPresentationModeV1::Direct,
        ToolPresentationModeV1::Code,
        ToolPresentationModeV1::Disabled,
    ] {
        assert_eq!(ToolPresentationModeV1::parse(mode.as_str()).unwrap(), mode);
        let profile = ToolPresentationProfileV1::for_mode(mode);
        profile.validate().unwrap();
        assert_eq!(profile.schema(), TOOL_PRESENTATION_PROFILE_V1_SCHEMA);
        assert_eq!(profile.mode(), mode);
    }
    for invalid in ["", "Code", " code", "code ", "auto"] {
        let error = ToolPresentationModeV1::parse(invalid).unwrap_err();
        assert!(matches!(error, ToolPresentationError::UnknownMode { .. }));
        assert!(error
            .to_string()
            .contains("[adaptive, direct, code, disabled]"));
    }
}
