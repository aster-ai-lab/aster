# AIX workbench qualification package

Build from `apps/studio`:

```sh
node scripts/build-aix-proof.mjs /tmp/a3s-workbench-proof.aix
```

The builder copies this ACL and Skill together with the existing executable
tree/mini-app fixtures. It packages five declared UI surfaces: a tree and custom
view in an ActivityBar container, a bottom panel, an editor and a mini app.
Fixed file ordering/timestamps make repeated builds byte-identical. It does not
parse ACL or grant permissions; API and native admission use `a3s-use-extension`.

For the existing Console plugin form, use publisher `proof`, name `workbench`,
version `1.0.0` and surfaces `skill`/`ui`, with the required storefront images.
The artifact is local qualification input; it has not been uploaded or accepted
against real PG/RustFS/Tauri. Native and API artifact tests read the path from
`A3S_CLOUD_TEST_AIX_WORKBENCH_PACKAGE` and must be invoked explicitly.

GUI acceptance must install through the existing plugin flow, open AIX Resources,
expand Tasks, select/invoke First task, refresh, use/save the custom view, open the
bottom panel/editor/mini app, change theme/language and uninstall. Confirm all
owned entries/documents/ports disappear and another owner remains independent.
Renderer-only tests cannot certify these observations in the deployed app.
