# Main window unavailable: read-only source diagnosis

Root reported `cgWindowNotFound` from both the existing bound accessibility
read and subsequent app acquisition while the known application process
remained running. This slice did not independently inspect the process,
window state, user preferences, configuration, sessions, arguments, or
environment. The actual cause remains **unknown**: source inspection cannot
distinguish minimized, hidden, another desktop, destroyed, or an accessibility
binding problem.

## Auditable Work paths

- `apps/work/src-tauri/src/lib.rs:371`: closing `main` hides it and prevents
  close only when `desktop_settings::hide_on_close()` permits it; otherwise
  it requests application exit. `desktop_settings.rs:458` requires both tray
  settings and an installed tray. No current setting was read.
- `desktop_settings.rs:623`: `show_main` resolves an existing `main`; if absent,
  it returns. If present, it calls `unminimize`, `show`, then `set_focus`.
  This is foreground recovery and does not recreate a missing main window.
- Tray Show/left-click use that same function at
  `desktop_settings.rs:598` and `:614`. Notification-response handling also
  calls it at `message_attention.rs:143`.
- Unix single-instance activation sends a show request when a later same-tenant
  claim is busy (`single_instance.rs:157`). Its listener invokes the callback
  (`:220`), wired to `show_main` by `lib.rs:384`. It therefore also focuses the
  window, and is not an authorized background recovery mechanism here.
- `authority.rs:257` boots desktop settings. Launch-to-tray can leave the first
  main-renderer load hidden (`desktop_settings.rs:470`, `lib.rs:821`). The
  initial signed-in window also starts hidden until the main renderer finishes
  (`lib.rs:797` and `:809`). These are conditional source paths, not evidence
  that either occurred in this run.
- The app menu includes AppKit's Hide item (`lib.rs:1075`). Screenshot
  `HideWindow` mode hides the caller and then explicitly attempts show/focus
  (`screenshot.rs:181`–`:201`). No screenshot or menu action was executed here.
- Work's `app.run` callback handles `Ready` at `lib.rs:389`; it has no custom
  `Reopen` recovery branch. Framework/AppKit default reopen behavior was not
  independently examined or claimed.
- The inspected renderer window IPC schema exposes close/minimize/size and
  reload operations, not a passive main-window restore. No inspected Work
  entry point restores accessibility binding without showing/focusing a window.

The inspected main-window, tray, single-instance, and screenshot paths provide
no evidence that completion of A07 caused a hide or close. Existing recovery
entry points cannot be invoked within this turn's prohibition on activation.
The debug launcher also contains a stop-previous-instance step
(`macos_app.rs:122`); it was not called or proposed as a fallback.

## Explicitly not claimed

- No root-cause attribution or current minimized/hidden/desktop/window state.
- No recovery, UI operation, foregrounding, synthetic input, global mouse,
  system script, process control, application exit/relaunch, or model request.
- No live user configuration/session/account/argument/environment inspection.
- No source repair, build, test rerun, changed task score, or claim that the
  still-running original tasks have settled. Root retains the user-mediated
  window-restoration and execution-settlement decisions.

Only bounded repository source reads/searches were performed; this evidence
record is the sole file written by this diagnostic slice.
