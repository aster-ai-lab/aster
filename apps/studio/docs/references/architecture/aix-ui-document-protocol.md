---
description: >-
  Native AIX document admission, private UI/tree/Composer/command channels, lifecycle ownership,
  state restoration and explicit capability limits
sources:
  - src/shared/types/aixViewBridge.ts
  - src/shared/types/aixTree.ts
  - src/shared/types/aixComposer.ts
  - src/shared/types/aixCommand.ts
  - src/renderer/services/plugins/aixCommandProvider.ts
  - src/renderer/components/plugins/AixViewActions.tsx
  - src/renderer/services/plugins/aixComposerProvider.ts
  - src/renderer/services/plugins/aixComposerInvocation.ts
  - src/renderer/services/plugins/aixWorkbench.ts
  - src/renderer/components/plugins/AixWorkbenchRuntime.tsx
  - src/renderer/components/plugins/PluginScopeProvider.tsx
  - src/shared/types/studioContribution.ts
  - src/shared/types/studioExtensionPoints.generated.ts
  - scripts/generateStudioExtensionPoints.mjs
  - src/renderer/services/studioTurn.ts
  - src/renderer/services/harnessAssemble.ts
  - src/renderer/components/plugins/PluginContributionContext.tsx
  - src/renderer/components/plugins/PluginSessionRuntime.tsx
  - src/renderer/components/plugins/PluginComposerActions.tsx
  - src/renderer/pages/agents/AgentChat.tsx
  - src-tauri/src/capabilities_workbench.rs
  - src-tauri/src/capabilities_aix.rs
  - src-tauri/src/capabilities_ui.rs
  - src-tauri/src/agent_plane/plugin_selection.rs
  - src/shared/types/aixWorkbench.ts
  - tests/fixtures/aix
---

# AIX UI document protocol

AIX Package uses an ACL manifest. The protocol below carries runtime messages;
these messages are not product configuration. Native admission verifies the
selected package, generation, publication revision and declared assets before
AsterDream mounts an isolated document. Installation alone grants no resource port.

AIX UI governs plugin documents and workbench contributions. Model-generated
OpenUI content uses the separate [Intelligent UI](./intelligent-ui.md) contract;
rendering a model interaction does not install a plugin or acquire an AIX port.

## Canonical extension point capability directory

`a3s-use-extension` owns the eight AsterDream point contracts. Run
`pnpm plugins:contract:generate` after changing the Rust contracts and
`pnpm plugins:contract:check` to refuse a stale TypeScript projection.
The native catalogue advertises `extensionPoints`; renderer preflight rejects
unknown or altered point contracts, composition modes and scopes. Legacy
catalogues retain their existing append-only behavior when this field is absent.

A UI declaration may specify `contract_version = 1` and `mode = "append"`.
Omitting them preserves schema 3 behavior. Versions are checked against the
resolved point, including Sidebar/Panel derived from an owned container.
Unknown versions, `replace`, `decorate`, and undeclared `target` attributes are
currently refused. The catalogue publishes no replacement targets. Contract
version, manifest schema, catalogue, admission and private-port versions remain
independent. The directory does not grant a resource capability.

ActivityBar/Sidebar/Panel are derived from `view_container`; Editor, Workspace,
miniapps and Composer use `ui`. External Harness declarations remain unavailable.
Composer supports Window/Workspace/Session/Run/Turn. Turn is the observed model
call ordinal; Subtask still requires native facts before external admission.
Transient document state belongs to its owner. Layout and recipe choices use
Preference, and business data belongs to its authority context.

Harness recipe selection uses `app.harness.assemble` through Preference. The
AsterDream bridge migrates `a3s.studio.harness-assemble.v1` once, gives an existing
unified value precedence, persists before notification, and relays other windows’
storage changes. This is a device-profile setting shared across sign-ins, without
account credentials or execution grants. Team selection remains member-bound.
The use case captures team and recipe before transport; retries preserve explicit
captured values. A failed preference load refuses new capture. Native admission
still verifies the recipe, member, workspace and exact package graph.

These implementation checks do not qualify cross-process/restart durability,
simultaneous multi-writer updates, PostgreSQL/RustFS or WebView isolation. Those
remain real certification obligations in META-00/META-05.

The `view-contract` fixture declares explicit version 1/append views. Run
`bash scripts/test-aix-view-commands.sh <new-evidence-directory> view-contract`
to use one actual archive across API admission, native installation/catalogue and
renderer document execution. The native projection preserves each explicit
contract. Renderer execution reads HTML/scripts from that archive. The default
fixture retains legacy declarations and bytes. These are memory-backed API and
JSDOM checks; they cannot qualify real storage or platform isolation.

## Host compatibility and earlier installations

AsterDream requires `requires_use` to match the Use contract compiled into the
canonical manifest library. This is distinct from the AsterDream product version,
manifest schema 3, catalogue protocol 1 and asset-session protocol 2. Parsing a
valid requirement does not establish compatibility with the current host.

The same bounded, verified manifest check applies to fresh installation,
restart adoption, callable/UI projections, installed selection publication,
view discovery/admission, asset reads and the native Meta Harness launcher.
Earlier hosts' content-valid generations cannot bypass that check. Neither raw
installation manifests nor archive fallback can promote an incompatible retained
generation. All native surfaces retain the same canonical manifest; disagreeing
or partially missing manifests refuse publication consumption. Unsupported
runtime surfaces remain refused.

The published Skill/UI surface IDs and kinds must exactly match that canonical
declaration. Workbench discovery, view admission and cached asset reads inspect
every surface, including non-UI surfaces. A compatible UI cannot hide an
incompatible Skill, an omitted declaration or an undeclared native surface.

Manifest parsing and asset responses consume bytes returned from Use's bounded
`read_verified_file` payload. The complete collected content digest, entry and MCP
projection are verified before returning those same owned bytes. AsterDream does not
verify a directory and then reopen a file for consumption. A missing manifest is
optional only in a valid legacy generation; corruption is never absence. This
read contract does not acquire a lease or grant execution/resource access.

Independent compatible views remain available when discovery refuses an
incompatible package. Complete Harness admission still refuses the entire
selected graph. An incompatible archive is refused before native generation
publication. Retained historical bytes are not rewritten to change compatibility.

## Retired format cleanup observations

Retired-format device cleanup is independent of AIX application success. It
withdraws the owned foreign selection before package retirement and does not
require executable content to be healthy. Only canonical `vsx/` package identities
are eligible; a foreign selection cannot withdraw an AIX package. Failure for one
owned package does not skip the other retirements. Existing generation leases
retain bytes until their owners settle. Unreadable or unowned metadata remains
refused and cleanup can retry after repair.

The authenticated native `/api/health` response includes
`capabilities.retiredPluginCleanup`: `unchecked`, `withdrawn`, `pending` or
`unavailable`. This reports the last cleanup attempt, not an execution grant.
`withdrawn` means foreign admission was withdrawn or absent; it does not assert
that leased bytes were reclaimed. `pending` records incomplete cleanup and emits
an error through the host logger. `unavailable` reports an unreadable local
observation. Historical receipt GET/DELETE requests do not run device cleanup.

## Catalogue discovery and package refusal

Native discovery inspects exact selected package generations under the Use
writer fence and retains verified bytes during inspection. Individual content
or installation failures quarantine only that package. The complete selection
metadata must still be valid and current. An invalid or withdrawn selection
refuses the whole catalogue. This inspection is separate from the complete
`SelectionLease` used for graph execution; that lease still refuses any failed
package. The installed selection and renderer catalogue both have a 128-package
limit.
Two installed directories claiming the same package identity are ambiguous;
both claims are refused during discovery and asset admission. A third package
with an independent identity remains eligible.

The native response carries at most 128 diagnostics. Each diagnostic has a
nullable installed `slug` and one closed `reason`: `package-unavailable` or
`package-content-refused`. The renderer validates these fields and logs refusals
centrally. A diagnostic grants no authority. Catalogue revision hashes the
selection cursor, verified projections and bounded diagnostics, so a newly
refused package changes the observed revision even without an install mutation.
Package publication revisions remain the independent asset admission identity.

## Conditional contribution visibility

An AIX `ui` or `view_container` declaration may contain one structured ACL
`when` block:

```acl
when {
  all = ["workspace", "conversation"]
  none = ["run"]
  theme = "dark"
  locale = "en-US"
}
```

Every specified condition must match. `all` requires present host observations;
`none` requires absent observations. The closed context keys are `workspace`,
`document`, `conversation`, `run`, `turn` and `harness`. Lists must be unique,
disjoint and contain at most six keys. Theme is `light` or `dark`; locale is a
bounded ASCII language tag matched exactly, ignoring case. Empty conditions,
unknown fields, nested expressions and guest-supplied context keys are refused.
This protocol does not evaluate JavaScript or a general expression language.

Visibility evaluates at the nearest consuming scope, including inherited
Composer contributions. It does not replace their original lifecycle owners
or grant resources. Window containers observe Window facts; they do not infer
an active workspace or Run from another UI subtree. Native catalogue projection
retains conditions without interpreting renderer appearance. The canonical ACL
parser and renderer boundary both validate the structured condition.

Container conditions withdraw their ActivityBar/sidebar/panel placements.
Per-view conditions withdraw owned documents; standalone editor, workspace and
mini-app entries share their declared conditions. Hiding an executing document
unmounts it and closes its private port and native asset admission. Restoring a
mini-app page creates a fresh owner rather than exposing its former private
draft. Unaffected siblings keep their existing documents. Pending Composer
actions and delegated sidebar navigation recheck visibility before applying
their results. Conditions are UI selection, never native authorization.

The independent `context-visibility` package and
`scripts/test-aix-context-visibility.sh` qualify exact archive publication/member
download, native projection/assets/uninstall and actual packaged-script UI
withdrawal. The same API campaign is wired to live PG/RustFS; it additionally
requires `A3S_CLOUD_TEST_AIX_CONTEXT_PACKAGE`. Local memory/JSDOM results do not
certify live storage or platform WebView isolation.

## Ownership and admission

Each mounted document has its own native asset session, admission request and
random document identifier. The identifier contains 32 random bytes, encoded
as 64 lowercase hexadecimal characters. It differs from the native request ID
and asset bearer ticket. AsterDream adds it to the admitted entry URL as
`#aix-document=<identifier>`; it is not part of an HTTP request or Referer.

The document creates a MessageChannel and transfers one endpoint to its parent.
AsterDream accepts only a message from that exact iframe's WindowProxy, with the
opaque origin `null`, the strict shape below and exactly one transferred port:

```typescript
{
  protocolVersion: 1,
  type: 'aix.document.ready',
  documentId: '<64 lowercase hexadecimal characters>'
}
```

AsterDream checks the identifier against the current admitted session. It accepts
one channel per session. Wrong identifiers, versions, shapes and port counts are
refused; owned refused ports are closed. A replay cannot replace a healthy
channel or reset its message budget. Unrelated frame messages cannot bind this
view. After 16 matching-type admission attempts, a seventeenth attempt withdraws
only the sending view and requests its native retirement.

The identifier binds a communication instance. It does not attest document bytes
or grant file, workspace, Tool, MCP or Harness access. Package/asset verification
is native. An already admitted package can deliberately disclose its identifier
or delegate a port; this protocol does not claim to prevent that delegation.

## Package-side startup

Install the handler before announcing readiness. Keep the package endpoint;
AsterDream replies on the transferred endpoint and never sends state to a
WindowProxy. A minimal bootstrap is:

```javascript
const documentId = new URLSearchParams(location.hash.slice(1)).get('aix-document')
if (!/^[a-f0-9]{64}$/.test(documentId || '')) throw new Error('No AIX document admission')
const channel = new MessageChannel()
channel.port1.onmessage = ({ data }) => {
  // Process the admitted UI or tree protocol on this private endpoint.
}
channel.port1.start()
window.parent.postMessage(
  { protocolVersion: 1, type: 'aix.document.ready', documentId },
  '*',
  [channel.port2]
)
window.addEventListener('pagehide', () => channel.port1.close(), { once: true })
```

The window message contains no host state, environment or credentials. The
parent reference targets the owning browsing context. All later application
messages use the private port. This follows the HTML Standard's
[child-ready and channel messaging model](https://html.spec.whatwg.org/multipage/web-messaging.html#channel-messaging).
There is no window-message fallback for packages using the previous host-push
bootstrap; those packages must adopt this handshake to receive a channel.

## Channel contracts

The native declaration selects the channel. The package cannot switch protocol
or expand authority by changing its readiness message.

| Declaration | Host greeting | Supported exchanges |
| --- | --- | --- |
| Webview/editor/workspace/mini-app UI | `aix.view.connect`, version 1 | Bounded state restoration, ordered `aix.view.store`/`aix.view.stored`, readonly `aix.view.environment` |
| Tree presentation | `aix.tree.connect`, version 1 | Provider readiness, serialized refresh/expand/collapse/select/invoke requests, bounded forests and ordered revisions |
| `studio.composer.actions` UI | `aix.composer.connect`, version 1 | Provider readiness and one explicit draft-processing request/result per document |

UI state is a serialized JSON value with a 64 KiB UTF-8 limit, depth limit 16
and value-count limit 4,096. The port permits 4,096 valid ordered requests.
Extra command fields, malformed data, replayed IDs and floods close the channel.
Closure does not permit rebinding that same session.

Appearance observations contain only light/dark theme, bounded locale,
left-to-right/right-to-left direction and the 21 allowlisted design variables.
They are readonly observations. A tree renders with AsterDream's native
`@a3s-lab/ui` TreeView and receives the separate tree contract. Node command
identifiers are sent back to that provider; they are not global native commands.

## Lifetime and restoration

The first iframe load event sends no state or port. A second load withdraws the
document and its port, then requests native session cancellation. Reopening
the declared entry requires a fresh asset session and document identifier.
Within-document appearance updates and accepted state writes preserve the
current frame. Navigation to another document does not inherit its channel.

A tree that loads without completing its document/provider handshake fails
within the existing five-second provider deadline. Static UI can render without
requesting a channel. A failed native admission immediately requests cancellation.
Owner closure clears temporary state and withdraws every port before waiting
for correlated native cancellation receipts. Failed retirement remains visible
to the owning lifecycle report; receipt loss is not treated as successful cleanup.

State restoration keys include package ID, generation, publication revision and
surface within the activation owner. A fresh document in that same owner can
restore its last accepted state. Another owner, package, generation or surface
cannot inherit it. Standalone editor/mini-app instances have separate child
view owners; closing and reopening an instance starts with a new owner.
These drafts are temporary in-memory state, not PG/RustFS or restart persistence.

## Composer actions

An AIX UI may declare `point = "studio.composer.actions"` with an HTML entry and
declared scripts. It cannot also select a container or tree presentation. The
contribution is an action in an existing Composer, rather than an editor page,
ActivityBar entry or mini app. It may select `scope = "window"`, `"workspace"`
or `"session"`. Omitting scope retains window/workspace placement. Scope is
currently valid only on Composer actions; unsupported owners and scoped views
are refused rather than silently widened.

The actual agent conversation center owns a workspace UI scope and a child
session UI scope, using its selected session ID and workspace metadata.
Session actions require the conversation service with a nonempty identity. Changing session or workspace,
closing the center, withdrawing the package or closing the parent withdraws
the old UI owner. This does not terminate a background native Code run.
Native Run, Turn, Subtask and Harness admission remain separate pending work.

Composer opts into ancestor composition. One contribution key selects the nearest
published activation; distinct keys sort by order and stable key. Ancestor items
keep their original activation context and cannot bypass the consumer scope's
plugin ceiling. Other slots remain local unless their consumer opts in.
Subscriptions observe every included owner; a closed consumer publishes no
inherited actions even while its window remains open.

An invocation belongs to both its activation and the consuming UI scope. A
session/workspace change cancels an inherited window action as well as local
session actions, even when package generation and contribution key are unchanged.
Cancellation removes the old executing document immediately. Neither the
scope declaration nor inheritance grants native workspace, Tool or MCP access.

The host creates a fresh hidden isolated document only after a user invokes the
action. Following the normal document handshake, it sends
`{ protocolVersion: 1, type: 'aix.composer.connect', surface }`. The provider
announces `{ protocolVersion: 1, type: 'aix.composer.ready' }` on that port. The
host then sends:

```typescript
{
  protocolVersion: 1,
  type: 'aix.composer.request',
  id: 1,
  revision: 9,
  text: '<at most 64 KiB of UTF-8 draft text>'
}
```

The draft identity, session/message identity, service snapshots and runtime
credentials are never sent. The provider replies with
`{ protocolVersion: 1, type: 'aix.composer.result', id: 1, revision: 9, text }`
or `{ protocolVersion: 1, type: 'aix.composer.refused', id: 1, revision: 9 }`.
Shapes are strict, revisions are nonnegative safe integers and replacement text
has the same byte limit. Wrong versions, request IDs, revisions, readiness
replays, early results and extra fields withdraw the channel. Each document
can process one request; it cannot rebind after closure.

The provider deadline is five seconds from invocation, including native
admission and document readiness. Unmounting the Composer, disabling editing,
withdrawing its package or closing its owner withdraws the document and port.
The existing Composer applies the edit only while draft identity, revision,
text and editability still match the captured observation; changing text and
then restoring it does not restore the old revision.

Native cancellation has a separate five-second deadline. A timeout aborts the
HTTP request and reports an unknown cancellation outcome even when a transport
ignores the abort. No edit is applied if its cleanup is unconfirmed. The ticket
remains retained, and explicit closure retry uses the same admission identity
and correlated receipt. Owner closure still has its own overall cleanup deadline
and reports failure or timeout. A late reply cannot restore a withdrawn port.

The [normalization package](../../../tests/fixtures/aix/composer-normalize/README.md)
executes its actual script through private channels in renderer tests. The same
retained archive is separately admitted by API publication/member download and
native installation/asset retirement. These checks do not establish real
WebView CSP/IPC or hosted storage acceptance.

## View commands and menu placements

An ACL `ui` block can declare up to 32 local commands and 64 menu placements.
Commands have a local `id`, a nonempty `title`, an optional icon and an optional
`enablement` condition. A menu references a command in that same UI block and
declares `view.title` or `view.context`, an integer order and an optional `when`.
Duplicate command IDs, duplicate location/command pairs, foreign references,
unknown fields and view commands on Composer surfaces refuse package admission.
The common condition contract applies to both enablement and menu visibility.

Commands are registered in the existing renderer dispatcher, with identities
bound to their package/view namespace and fresh document ID. The same operation
can have several placements. `enablement` applies to every invocation; a menu's
`when` only controls its placement. Hidden placements cannot invoke a captured
action after their condition changes. Document closure withdraws registrations;
saved operations cannot target a reopened page or a sibling instance.

The host's view header renders title actions and opens context actions. Host
tree views are also inside this context boundary. Events from inside an isolated
iframe do not bubble to that host boundary; guest DOM context-menu contributions
are not part of this contract. Appearance or menu visibility changes retain the
executing frame and its private UI state.

One admitted document shares its private port between the state or tree protocol
and command protocol. The host sends:

```typescript
{ protocolVersion: 1, type: 'aix.command.connect', surface, commands: ['refresh'] }
```

After the provider replies `aix.command.ready`, the host can send
`{ protocolVersion: 1, type: 'aix.command.execute', id: 1, command: 'refresh' }`.
The provider replies `aix.command.completed` or `aix.command.refused` with that
same request ID and command. No native command, arguments, resource paths,
credentials or service snapshots are carried. A normal refusal allows retry.
Malformed, premature, uncorrelated or replayed messages withdraw the document.
Only one operation can be pending. Readiness and pending operations each have a
five-second deadline; cancellation refuses late completion.

The 4096-message command budget includes readiness and operation receipts.
The host checks available receipt capacity before sending another operation.
After 4095 valid completed/refused operations, further commands are disabled;
the view can retain its state until its normal owner/asset lifetime ends.
The command receiver does not replace or own the document's primary port receiver.
Failure in either subprotocol withdraws the shared executing document.

The independent [command proof package](../../../tests/fixtures/aix/view-commands/README.md)
contains two webviews, a native tree and a mini app. Run
`bash scripts/test-aix-view-commands.sh <new evidence directory>` from AsterDream to
build one archive, admit it through API publication/member pin and native install,
then execute the actual provider script with the native-produced catalogue.
Ordinary renderer runs explicitly skip these two archive-dependent cases when
the catalogue input is absent; the script requires both cases to pass.

## Verification and limits

The packaged [mini-app](../../../tests/fixtures/aix/miniapp-environment/README.md)
and [tree provider](../../../tests/fixtures/aix/tree-provider/README.md) execute
through actual private MessageChannels in renderer contract tests. A built AIX
archive is separately checked through API publication/member download and native
installation, asset validation and retirement. Those checks do not establish
real WebView sandbox/CSP, native IPC isolation, hosted HTTP, PG/RustFS or GUI
acceptance. Privileged resource ports and AIX Harness execution still require
their own admission and authority contracts.
