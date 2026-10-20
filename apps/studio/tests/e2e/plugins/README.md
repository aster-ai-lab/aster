<a id="studio-plugin-composition-campaign"></a>

# AsterDream plugin composition campaign

This fixture exercises the production `WorkbenchPluginHost`, scope provider,
ActivityBar buttons, Sidebar registry provider, actual `/app/plugin` route,
view slots, and composer action consumer. Two workspace scopes and independent
TanStack memory routers coexist under one window. The 32-step ACL scenario
verifies exact page URLs, isolated edits, generation replacement, closing only
one workspace, removing both open pages and all contributions, and fencing old
event listeners.

It is a renderer integration campaign. It does not launch the complete AsterDream
shell, install native packages, execute ACP/Harness runs, or run VSX extensions.
`ComposerSurface.lazy.test.tsx` additionally exercises the production shared
Composer's plugin action placement and stale/rich-draft guards.

From `apps/studio`, start the loopback fixture:

```sh
./node_modules/.bin/vite --config tests/e2e/plugins/vite.config.ts
```

Prepare a standalone `agent-browser` 0.26.x executable compatible with the
installed a3s-test's protocol revision 15. The retained campaign used 0.26.0;
global 0.37.1 fails executable admission. Pass the compatible executable's
absolute path explicitly:

```sh
a3s-test run tests/e2e/plugins/composition.acl \
  --browser-driver standalone \
  --browser-executable /absolute/path/to/compatible/agent-browser \
  --json
```

The fixture binds only `127.0.0.1:4323`. The scenario owns and closes its browser
session. Stop the Vite process separately when finished. A missing CSS target is
asserted with `visible_count = 0`, because this adapter reports a missing CSS
element as a command failure for a visibility query. Cleanup evidence uses exact
`rendered_text` assertions, including a stable observation after removal.

The interactive exploration was promoted to `composition.acl` after observing
the successful workflow. Reports and source fingerprints are retained in the
OS repository's `docs/evidence/studio-plugins/20261008-activity-pages/`. The earlier
foundation certificate remains under `20261007-ui-scope-foundation/`. The latest
browser result precedes the final immediate-close refinement; that refinement
has contract-test evidence and awaits a browser rerun because the current
execution permissions refuse local port listening.
