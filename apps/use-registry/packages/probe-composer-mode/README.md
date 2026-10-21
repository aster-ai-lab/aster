# Composer mode contract probe

This package exercises the `desktop.composer.modes` declaration and installed
catalogue projection. `build` binds a required local builder agent and editor
with workspace/document context; `inspect` is conversation scoped without an
editor. Neither declaration changes the host permission posture.

The editor is a contract fixture. This package is not the digital twin engine
and does not certify scene editing or composer execution. Native tests install
the actual archive, inspect its modes, remove it, and reject corrupted metadata.

Human-authored mode declarations use `a3s-use-extension.acl`. The host validates
version, local references, uniqueness, scope and bounded context before exposing
them. Runtime selection and resource grants remain independent host checks.
