use super::*;

#[test]
fn recording_states_have_exact_names_and_transitions() {
    for (name, state) in [
        ("idle", RecordingState::Idle),
        ("selecting", RecordingState::Selecting),
        ("recording", RecordingState::Recording),
        ("finalizing", RecordingState::Finalizing),
        ("review", RecordingState::Review),
        ("attaching", RecordingState::Attaching),
        ("exporting", RecordingState::Exporting),
        ("cancelled", RecordingState::Cancelled),
        ("failed", RecordingState::Failed),
    ] {
        assert_eq!(RecordingState::parse(name), Ok(state));
        assert_eq!(state.as_str(), name);
    }
    assert!(RecordingState::Idle.can_transition_to(RecordingState::Selecting));
    assert!(RecordingState::Recording.can_transition_to(RecordingState::Finalizing));
    assert!(RecordingState::Finalizing.can_transition_to(RecordingState::Review));
    assert!(RecordingState::Review.can_transition_to(RecordingState::Attaching));
    assert!(RecordingState::Attaching.can_transition_to(RecordingState::Review));
    assert!(!RecordingState::Idle.can_transition_to(RecordingState::Review));
    assert!(!RecordingState::Cancelled.can_transition_to(RecordingState::Recording));
    assert!(RecordingState::Failed.is_terminal());
    assert!(RecordingState::parse("done").is_err());
}

#[test]
fn targets_and_destinations_are_closed_sets() {
    for name in ["area", "window", "screen", "device"] {
        assert_eq!(
            CaptureTargetKind::parse(name).map(|value| value.as_str()),
            Ok(name)
        );
    }
    for name in ["composer", "recorded", "page", "localhost", "clipboard"] {
        assert_eq!(
            CaptureDestination::parse(name).map(|value| value.as_str()),
            Ok(name)
        );
    }
    assert!(CaptureTargetKind::parse("display").is_err());
    assert!(CaptureDestination::parse("opencode").is_err());
}

#[test]
fn recording_errors_are_closed_strings() {
    for (name, error) in [
        ("recording-busy", RecordingError::Busy),
        ("recording-cancelled", RecordingError::Cancelled),
        ("recording-not-found", RecordingError::NotFound),
        ("permission-denied", RecordingError::PermissionDenied),
        ("invalid-target", RecordingError::InvalidTarget),
        ("invalid-time", RecordingError::InvalidTime),
        ("invalid-bundle", RecordingError::InvalidBundle),
        ("invalid-transition", RecordingError::InvalidTransition),
        ("unsupported-media", RecordingError::UnsupportedMedia),
        (
            "destination-unavailable",
            RecordingError::DestinationUnavailable,
        ),
        ("stale-request", RecordingError::StaleRequest),
        ("foreign-session", RecordingError::ForeignSession),
        ("payload-too-large", RecordingError::PayloadTooLarge),
        ("finalization-failed", RecordingError::FinalizationFailed),
        ("export-failed", RecordingError::ExportFailed),
        (
            "desktop-capability-unavailable",
            RecordingError::CapabilityUnavailable,
        ),
    ] {
        assert_eq!(RecordingError::parse(name), Ok(error));
        assert_eq!(error.as_str(), name);
    }
    assert!(RecordingError::parse("unknown-error").is_err());
}

#[test]
fn area_bounds_reject_nan_zero_and_excessive_geometry() {
    let valid = CaptureArea {
        x: -100.0,
        y: 20.0,
        width: 640.0,
        height: 480.0,
    };
    assert_eq!(valid.validate(), Ok(()));
    for invalid in [
        CaptureArea {
            width: f64::NAN,
            ..valid
        },
        CaptureArea {
            width: 0.0,
            ..valid
        },
        CaptureArea {
            height: 15.0,
            ..valid
        },
        CaptureArea {
            x: f64::INFINITY,
            ..valid
        },
    ] {
        assert_eq!(invalid.validate(), Err(RecordingError::InvalidTarget));
    }
}

fn metadata() -> AsterCaptureMetadata {
    AsterCaptureMetadata {
        schema_version: ASTER_CAPTURE_SCHEMA.into(),
        tenant: "aster".into(),
        session_id: "session-1".into(),
        draft_id: Some("draft-1".into()),
        destination: CaptureDestination::Composer,
    }
}

#[test]
fn metadata_round_trip_preserves_fields_and_refuses_overwrite() {
    let directory = std::env::temp_dir().join(format!(
        "aster-metadata-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("aster.json");
    let expected = metadata();
    expected.write_new(&path).unwrap();
    assert_eq!(AsterCaptureMetadata::read(&path).unwrap(), expected);
    assert!(expected.write_new(&path).is_err());

    let mut invalid = expected;
    invalid.tenant.clear();
    assert!(invalid.validate().is_err());
    invalid.schema_version = "aster-capture-0".into();
    assert!(invalid.validate().is_err());
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn draft_metadata_binds_once_and_refuses_reassignment() {
    let directory = std::env::temp_dir().join(format!(
        "aster-binding-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("aster.json");
    let mut bound = metadata();
    bound.session_id = ASTER_DRAFT_SESSION.into();
    bound.draft_id = None;
    bound.write_new(&path).unwrap();
    bound.bind_session(&path, "aster", "session-2").unwrap();
    assert_eq!(bound.session_id, "session-2");
    assert_eq!(bound.draft_id.as_deref(), Some(ASTER_DRAFT_SESSION));
    assert_eq!(AsterCaptureMetadata::read(&path).unwrap(), bound);
    assert!(
        bound
            .clone()
            .bind_session(&path, "aster", "session-3")
            .is_err()
    );
    assert!(
        bound
            .clone()
            .bind_session(&path, "other", "session-2")
            .is_err()
    );
    assert_eq!(AsterCaptureMetadata::read(&path).unwrap(), bound);
    bound.bind_session(&path, "aster", "session-2").unwrap();
    std::fs::remove_dir_all(directory).unwrap();
}
