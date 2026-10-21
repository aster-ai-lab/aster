<a id="desktop-digital-twin"></a>

# AsterDream Digital Twin

Standard schema-3 `.aix` source for AsterDream's digital twin editor. The package
declares an ActivityBar container and a workspace composer mode. AsterDream owns
installation, scoped project resources and agent execution; the package owns
scene editing and rendering. It grants no physical-device command channel.

The editor uses `a3s-digital-twin` compiled to WebAssembly. ACL is the saved
scene format; structured JSON is used only for typed transport messages. Scene
validation, revisions and inverse operations come from the same Rust domain.

Build from the repository root using the installed AsterDream dependencies:

```sh
bun apps/use-registry/packages/digital-twin/tools/build.mjs
```

Verify the editor slice with:

```sh
bash tools/dev/verify_desktop_digital_twin_editor.sh
```

The current editor supports primitive objects, exact-ID selection, transforms,
properties, undo/redo and scoped project save/reopen. The repeatable check uses
the real Rust WASM domain, Chromium/WebKit under AsterDream's unchanged sandbox,
and a real native archive. Browser project storage is an explicit test fixture;
the API/native resource campaign is a separate memory/PostgreSQL proof.

The catalogue supplies this package as “数字孪生引擎（预览）”. Durable draft/intent
recovery across navigation/restart, native GUI acceptance, the executable
builder/tool chain, GLB templates, simulation and telemetry remain in the epic
task ledger. Its preview listing does not certify the complete engine.

`src/`, `tests/`, `tools/` and `.build/` are source/build inputs. Only the ACL
manifest, agent/skill instructions, notices and `ui/` assets belong in the AIX
archive. Installation and composer execution acceptance are recorded in the
epic evidence; package source alone does not establish those results.
