# Task workspace over an open browser

Studio revision: `cb89d095b8d6106139b14cee498b97b7dee6f308`. The change is in the renderer; the native host is unchanged.

On macOS 27.2 (arm64), opened a task from the recent-task catalog with a local form already displayed in Aster's native browser. Before the fix, the form's button and input remained exposed beside the Analysis tab. After the fix, Analysis exposed its event details and neither webpage control was present. Switched to Encyclopedia with the same result. Closed the workspace: the input retained the test text, the counter was still 1, and clicking the button advanced it to 2.

From `apps/studio`:

- Vitest: 5 renderer suites, 105 tests passed. Suites: AsterWorkspaceBrowser, workspaceBrowser.race, WebviewNavigation, WorkspaceWikiCatalog and AppShell. The local runner configuration supplied the checkout's dependency aliases.
- `ASTER_TENANT_SLUG=aster node_modules/.bin/rsbuild build`: production renderer built successfully (output redirected to a temporary directory).

## Explicitly not claimed

No Windows or Linux desktop interaction run, full repository test run, or release package build for this fix.
