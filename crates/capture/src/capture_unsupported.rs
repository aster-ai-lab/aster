#[derive(Clone, Debug)]
pub enum LiveOverlayEvent {
    CaptureShortcut,
    PasteLatestShortcut,
    UndoStroke,
    RequestSnapshot,
    PhoneEnded,
}
