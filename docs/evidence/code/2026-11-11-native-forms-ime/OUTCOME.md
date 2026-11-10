# Native forms and Chinese input

These checks used the freshly paired source build identified in
[candidate.json](candidate.json), `deepseek/deepseek-flash`, and a private copy
of the supplied `../../config.acl`. The source ACL was unchanged. This was a
dirty diagnostic candidate containing the parallel ACP cancellation patch,
not a release qualification.

## Actual checks

The native Terminal window was observed and captured with agent-desktop.
Form gestures used recorded native System Events input after the previously
retained agent-desktop window-identity failures. Chinese composition,
candidate selection, and submission used agent-desktop physical keys.

Commands were run at the repository root:

```sh
cargo build --locked -p aster -p aster-acp --bins
python3 .runtime/tui-full-e2e/launch_native.py
agent-desktop --session run-1794328309095-53068-0 snapshot --app 终端 --window-id w-55283 -i --compact
agent-desktop --session run-1794328309095-53068-0 --headed press n --app 终端 --window-id w-55283
```

The remaining physical letters, Space, and Return used the same command
route. Private operator scripts and raw traces remain in `.runtime`; they
are development aids, not a new permanent OS-interaction test gate.

- Empty form submission retained the dialog and showed both required-field
  errors. Amount `6` then showed the declared maximum `5`, retaining the
  draft. The original invocation finally returned exactly
  `{"name":"Ada","amount":4.0,"ready":true}` once, and the Run completed.
  See [form-result.json](form-result.json) and the retained screenshots.
- Ctrl+C cancelled a second form. Its one original invocation returned the
  declared explicit cancellation error, and the model did not retry.
  See [form-cancel-result.json](form-cancel-result.json).
- Pinyin preedit admitted no task. Return committed literal `nihao` without
  sending it. In a separate scenario, Space selected `你好`; an explicit
  subsequent Return submitted exactly one task with UTF-8 bytes
  `e4bda0e5a5bd`. Its Run completed once. See
  [ime-result.json](ime-result.json).

The form document is the unchanged fixture from
`crates/code/acp/tests/aster_tui_live_e2e/forms.rs`. The bridge rendered it
through the existing Aster elicitation UI. This does not prove integration
with an external MCP server.

## Retained checker mistakes

The initial form-cancel checker expected a JSON action object, but the tool's
declared cancellation contract returns an explicit error. The original
failure is recorded with the corrected receipt check; product behavior and
the no-retry requirement were unchanged.

The initial IME checker incorrectly expected Return to choose a Chinese
candidate. That failed expectation remains in
[ime-enter-raw-result.json](ime-enter-raw-result.json). The separate Space
selection scenario follows Apple's
[candidate-window documentation](https://support.apple.com/en-al/guide/chinese-input-method/cim12992/mac).
The original candidate plan retains its inherited Return wording and records
the explicit Space operation separately; it was not rewritten after the run.
No product code, prompt, validation constraint, or provider branch was changed
to accommodate either checker mistake.

## Explicitly not claimed

This proves the observed basic form and local Pinyin scenarios. It does not
qualify every JSON Render component, schema boundary, IME, input source,
terminal, clipboard format, operating system, or the whole 508-case catalogue.
No application fix was needed for these scenarios. The owned CLI and ACP
processes were verified exited before subsequent source edits.
