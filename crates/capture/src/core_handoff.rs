pub const AGENT_CONTEXT: &str = "agentContext";
pub const LIVE_TRANSCRIBE: &str = "liveTranscribe";

static EXPERIMENT_OVERRIDES: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

/// Evidence-core experiments are enabled explicitly by the host. Product configuration is not
/// read from Capture's separate application store.
pub fn enable_experiment(name: &str) {
    EXPERIMENT_OVERRIDES
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .push(name.to_owned());
}

pub fn experiment(name: &str) -> bool {
    let overridden = EXPERIMENT_OVERRIDES
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .iter()
        .any(|candidate| candidate == name);
    if overridden || cfg!(test) {
        return overridden;
    }
    false
}
