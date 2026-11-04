//! Shared welcome and resumed-composer readiness for the shipped TUI tests.

pub fn is_input_frame(screen: &str) -> bool {
    (screen.contains("Aster") && screen.contains("Type a message..."))
        || (screen.contains("Add a follow-up") && screen.contains("Shift+Tab:mode"))
}

#[path = "../boundary_e2e/readiness_tests.rs"]
mod tests;
