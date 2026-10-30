use crate::model::MissingTranscript;

/// The plain sentence for a capture without a transcript, from the helper's `missing`.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub fn missing(why: &str, locale: &str, error: Option<&str>) -> MissingTranscript {
    let model = if cfg!(target_os = "windows") {
        format!("The Parakeet speech model ({locale})")
    } else {
        format!("Apple's speech model ({locale})")
    };
    match why {
        "downloading" => MissingTranscript {
            reason: format!(
                "{model} was still downloading. The audio is saved, and Capture adds the \
                 transcript to this bundle when the download finishes."
            ),
            retry: true,
        },
        "permission" => MissingTranscript {
            reason: "Speech Recognition is off for Capture. The audio is saved, and Capture adds \
                     the transcript to this bundle once it's on (Capture Settings › Permissions)."
                .to_owned(),
            retry: true,
        },
        "dictation" => MissingTranscript {
            reason: "Dictation's on-device model isn't on this Mac. Turn on Dictation in System \
                     Settings › Keyboard, and Capture adds the transcript to this bundle once it's \
                     there."
                .to_owned(),
            retry: true,
        },
        "unsupported" => MissingTranscript {
            reason: format!(
                "this Mac has no Apple speech model for {locale}. Hex can transcribe instead \
                 (Capture Settings › Recording)."
            ),
            retry: false,
        },
        _ => MissingTranscript {
            reason: format!(
                "{model} couldn't be installed{}. The audio is saved, and Capture adds the \
                 transcript to this bundle if a later try works.",
                error.map(|e| format!(" ({e})")).unwrap_or_default()
            ),
            retry: true,
        },
    }
}
