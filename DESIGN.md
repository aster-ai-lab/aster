# Aster Design

This file, at the repository root, is the only product design spec. Aster, the web console, Work, Office, and the
official site share it. Product chrome uses these roles. Do not add a second `DESIGN.md` for an app or a package.

The values were taken from the Aster work page, the plugin page, the settings page, and the shell around them.
New chrome uses the same color, type, rows, search, segmented control, cards, and dialogs.

The visual values and the interaction contract in section 14 are the spec. Match them. Do not invent a second
gray, a second row height, or a second selected state for the same role. New and substantially changed resource
screens must satisfy [Collection development standard](#14-collection-development-standard); an existing screen
is reference material, not evidence that it already satisfies that standard.

Token architecture stays in [packages/ui/docs/design-token-system.md](./packages/ui/docs/design-token-system.md) and
[packages/ui/docs/variable-catalog.md](./packages/ui/docs/variable-catalog.md). Those documents own the Shadcn
variable graph. This document owns the product look. `packages/ui` writes
`src/styles/tokens/spec-roles.css` from the Color, Type, and Shape tables here. After a table change, run
`pnpm --filter @aster-ai-lab/ui theme:build`. Selection, rail icons, rows, and buttons use the neutral palette here.
The tenant brand is the build-time stamp `--aster-tenant-brand`. It does not paint this chrome. A tenant's name,
wordmark, and logo notes live beside that tenant's `INFO.md` in `BRAND.md`.

This file is the only `DESIGN.md` in the tree.

## Reference pages

| Surface | Source |
| --- | --- |
| Work home | [WorkbenchHome.tsx](./apps/studio/src/renderer/pages/agents/WorkbenchHome.tsx), [WorkbenchFeatureStrip.tsx](./apps/studio/src/renderer/pages/agents/WorkbenchFeatureStrip.tsx) |
| Plugin store | [PluginStorePage.tsx](./apps/studio/src/renderer/pages/plugins/PluginStorePage.tsx) |
| Settings | [SettingsPage.tsx](./apps/studio/src/renderer/pages/settings/SettingsPage.tsx), [settingsStyles.ts](./apps/studio/src/renderer/pages/settings/settingsStyles.ts), [settingsMenu.ts](./apps/studio/src/renderer/components/settingsMenu.ts) |
| Segmented control | [segmented-control.tsx](./packages/ui/src/components/primitives/segmented-control.tsx) |
| Window frame | [AppShell.tsx](./apps/studio/src/renderer/components/layout/AppShell.tsx), [settingsOverlay.css](./apps/studio/src/renderer/components/layout/settingsOverlay.css), [Sidebar.css](./apps/studio/src/renderer/components/Sidebar/Sidebar.css) |
| Page entry | [PageEntry.tsx](./apps/studio/src/renderer/components/layout/PageEntry.tsx), [tailwind.css](./apps/studio/src/renderer/assets/styles/tailwind.css) |

User-visible strings stay in `zh-cn.json` and `en-us.json`.

## 1. Direction

The pages are white work surfaces. Structure comes from type size, a gray field, and a light gray selected row.
Color is reserved for meaning inside a feature, not for navigation.

- Page, card, and dialog backgrounds are white.
- Selected navigation is a gray plate and darker ink. It is not a filled brand color.
- One list row, one search field, and one segmented control are reused across the three pages.
- Shadows sit on floating cards, dialogs, and the settings overlay. Resting page sections stay flat.
- Icons in rows are 16px Lucide outlines at stroke 1.7. The icon rail is the exception: its selected glyph is a
  black fill on a gray plate.
- Light mode is the authored look of the work page, plugin cards, and settings. Dark values exist for the rail,
  the segmented control, and the plugin search fill. Leave a surface in its light values until a dark design for
  that surface exists.

## 2. Color

| Role | Value | Where |
| --- | --- | --- |
| Ink | `#171717` | Selected row text, page titles via `text-foreground`, dialog confirm fill, session dot |
| Title ink | `#1a1a1a` | Plugin card titles, collection headings, hero title |
| Feature ink | `#1c1c1c` / `#222` | Feature-card titles and prompt text |
| Body | `#3f3f3f` | Unselected rows and headers |
| Muted | `#8a8a8a` | Subtitles, section labels, timestamps, empty copy. Settings placeholder uses `--settings-nav-muted`, which is this value |
| Caption | `#8b8b8b` | Plugin card descriptions |
| Segment rest | `#6f6f6f` | Unselected segment label |
| Placeholder | `#c5c5c5` | Work composer placeholder |
| Page | `#ffffff` | Work, plugins, settings nav, settings content |
| Canvas | `#e6e6e6` | Icon rail and the gutter around the content card. Dark `#242424` |
| Field | `#f5f5f5` | Search, 新建任务, text fields, segmented track. Dark search and track `#2c2c2c` |
| Selected row | `#f2f3f5` | Selected nav row, selected session, selected category |
| Hover row | `#f7f7f7` | Row hover |
| Field hover | `#ececec` | 新建任务 hover |
| Card border | `#ececec` | Plugin cards, composer, quiet outlines |
| Hairline | `#efefef` | Work sidebar edge, feature-card border |
| Button border | `#e5e5e5` / `#e6e6e6` | Plugin install button, hero button |
| Quiet button hover | `#fafafa` | White buttons |
| Inner tile | `#f4f5f7` | Feature-card prompts. Hover `#eef0f3` |
| Rail plate | `#d4d4d4` | Selected and hovered rail button. Dark `#3a3a3a` |
| Rail rest | `#666666` | Unselected rail glyph. Hover `#3d3d3d`. Dark rest `#d0d0d0`, dark glyph `#f5f5f5` |
| Send ready | `#3a3a3a` | Composer send button when it can send |
| Send idle | `#d0d0d0` | Composer send button when it cannot send |
| Danger text | `#b42318` | The confirm step of a row delete |
| Dialog scrim | `rgba(0, 0, 0, 0.20)` | Work dialogs |
| Settings scrim | `rgba(23, 23, 23, 0.14)` | Settings overlay |
| Focus | `black` at 10% | 2px ring, inset on rows |

Feature-card marks stay on those cards: writing `#69b9c9`, plan blue `#4c6fff`, slides `#e07848`, PDF
`#5b35d4`. They are not navigation colors and they are not the tenant brand.

Office editors retain their familiar format colors: Word blue `#185abd`, Excel green `#107c41`, and
PowerPoint orange `#c43e1c`. Use these within each editor for ribbon selection, editing selections, and
feature controls. Keep the application rail, file tabs, dialogs, typography, sizing, and neutral surfaces
on the shared roles above. File tabs own switching and closing; do not add a second back-to-files header
or a duplicate save-status row above the editor. Save status belongs in the editor footer and dirty tab dot.
Library row and context menus are non-modal. A press on blank space, a filter, another row, or the sidebar
closes the open menu and reaches the pressed control on that same interaction. Escape and choosing an
action also close the menu. Report saved only after the host accepts the write; do not imply automatic saving
when the host requires an explicit save.

## 3. Type

The UI font is `--font-family`. The default face is Ubuntu. On Windows the default is Microsoft YaHei UI, then
Microsoft YaHei. A saved UI font replaces that default through `--app-user-font-family`. Code, paths, and key
bindings use `--code-font-family`. The default code face is Cascadia Code. Weight is regular unless the role
below says otherwise. Medium marks a selected row, a card title, or a segment. Bold is the page title.

| Role | Size | Weight | Use |
| --- | --- | --- | --- |
| Greeting | 28px, tight tracking | semibold | Work home heading |
| Page title | 24px, tight leading | bold | Plugin title, settings section title |
| Hero | 22px, snug leading | semibold | Plugin home hero |
| Section | 18px | semibold | Plugin collection heading |
| Subtitle | 15px | regular | Page subtitle, settings description, composer text |
| Control | 14px | medium | 新建任务, plugin card title, feature title, dialog title |
| Row | 13px | regular, medium when selected | Nav rows, categories, sessions, buttons, segments |
| Meta | 12px | regular | Section labels, timestamps, card descriptions (`#8b8b8b`, line-height 20px) |
| Fine | 11px | regular | Feature-card tags |

Section labels are 12px `#8a8a8a`. In settings they use 12px / 16px line-height. In the work task list, day
headings are 12px with no extra line box. They are not buttons.

## 4. Icons

- Row and header icons are 16px (`size-4`) with Lucide `strokeWidth` 1.7. Color follows the row text.
- Small row actions (rename, pin, archive, delete, plus, close) and message actions are a 24px hit target, 14px
  glyph, stroke 1.7, radius 6px. Rest text is `#3f3f3f`. Hover, active, and open use `#f2f3f5` with `#171717`.
  Focus is the inset 2px `black/10` ring. Disabled opacity is 40%. Message actions load from
  `message-action-menu.css` through an unlayered `@import` in the main stylesheet.
- Composer toolbar icons sit in a 32px hit target.
- Workbench home and task inputs place the existing screenshot split control immediately after Plus.
  Its scissors and chevron form one 48px-wide, 30px-high rounded group with a shared hover/open background;
  the two actions retain separate focus and names. Toolbar minimum-width rules apply to standalone
  buttons, not to the internal halves of this compound control. The loading placeholder uses the same order.
- Rail menu glyphs are 22px at Lucide stroke 1.6. Selected rail glyphs use `fill: currentColor` and `stroke: none`.
  Unselected glyphs stay outlines. Do not set a CSS `stroke-width: 2px` on the rail.
- A selected rail button is a 12px-radius gray plate. The rail avatar keeps a 4px radius.

## 5. Shape, elevation, and focus

| Element | Radius | Shadow |
| --- | --- | --- |
| Row, field, quiet button, 新建任务 | 8px | none |
| Segment pill | 6px | `0 1px 2px rgba(0, 0, 0, 0.06)` when selected |
| Segment track | 8px | none |
| Plugin card, work composer | 14px | `0 8px 22px rgba(20, 40, 80, 0.04)` |
| Feature card, plugin hero | 16px | feature card `0 4px 16px rgba(0, 0, 0, 0.04)`; hero none |
| Dialog | 12px | `0 12px 32px rgba(0, 0, 0, 0.12)` |
| Content card | 10px | none; `0.5px` `border-border` |
| Rail menu plate | 12px | none |
| Rail avatar | 4px | none |
| Settings panel | square to the content card | `0 16px 48px rgba(23, 23, 23, 0.16)` |

Quiet white buttons may add `0 1px 2px rgba(20, 40, 80, 0.04)` when they sit on a tinted hero.

The 2px `black/10` ring is an inner focus tint. On rows and category buttons the tint is inset. Keyboard focus
must also remain clearly visible: use the existing Ink role for an outline when the tint alone is insufficient.
Do not remove the native focus indicator until its replacement passes section 14.12. Do not add a brand-colored
frame.

`rounded-full` is for the work session dot (20px, `#171717`) and for rail tooltips. Menu buttons are not pills.

## 6. Shared controls

### Motion and emotional feedback

Motion explains an action, preserves a place, or confirms a real result. The feeling Aster should leave is
calm competence: the work is moving, the person can intervene, and the result is theirs to inspect. Every
animation has a named trigger and an end state. A decorative effect is not a substitute for useful feedback.

The references are Arc's public [motion guidance](https://uiarc.dev/r/skills/arc/motion.md),
[action feedback](https://uiarc.dev/components/action-button),
[confirmation morph](https://uiarc.dev/components/confirm-morph),
[agent status](https://uiarc.dev/components/agent-status), and
[activity terrain](https://uiarc.dev/components/blocks/activity-terrain). Adopt their continuity, restrained
springs and feedback in the existing Aster controls. Keep this document's graphite palette, anatomy, focus,
dialog placement and authority rules. Arc is a behavior reference, not a second design system or a reason to
bulk-install components. Public examples with simulated events are not a production data source. Reusing
licensed source requires the corresponding rights; reading a public preview does not provide that source.

#### Emotional purpose

| Moment | What the person needs | Aster's response |
| --- | --- | --- |
| Starting | Know that the action registered | The pressed control answers immediately; accepted work has a real pending state. Keep the draft until submission is confirmed |
| Waiting | Know what is happening and retain control | One current activity, elapsed time where useful, and the existing Stop or pause action. Unknown duration stays indeterminate |
| Deciding | Understand the consequence | Stable approval text names the target, scope and action. Details expand in place; animation never moves the confirm target or grants permission |
| Finishing | See a useful accomplishment | Confirm the verified result once, then foreground its file, preview, sources or checks. Keep execution detail available behind one disclosure |
| Recovering | Continue without starting over | Keep input, context and the failed item; explain the next safe action in place. Retry and undo follow the real operation contract |
| Returning | Recover the working context | Preserve task, query, selection and reading position. Summarize actual changes since the last visit without replaying every entrance |

Do not infer a mood from private content. Do not invent time saved, completion percentages, usage streaks or
performance scores. Spending more tokens is not an achievement. A budget refusal, failure, or unverified
result never receives a celebratory animation. Copy should be specific and useful, rather than praise the
person for operating a control.

#### Timing and physical behavior

These are motion roles for shared components, not a new CSS variable or package API. Reuse the installed
Motion/CSS mechanism. The explicit picker, dialog, settings overlay and row timings below take precedence;
do not replace their geometry or timing while adding a microinteraction.

| Role | Default | Use and constraint |
| --- | --- | --- |
| Direct response | 120ms | Hover color and press acknowledgement; never delay the action |
| State exchange | 160ms | One short opacity exchange for an icon, action label or status; same baseline and stable slot |
| Ordinary transition | 240ms | A local view or content change without a component-specific timing |
| Responsive spring | 260ms, bounce at most 0.12 | Small press/feedback glyphs. A press may scale an inner visual to 0.97 while its hit target stays fixed |
| Settling spring | 400ms, bounce 0 | Non-modal inline expansion or content-size change when the changing size conveys information |
| Shared highlight | 420ms, bounce at most 0.16 | A small selection plate following a choice. It must stay inside the track and never suggest another selected value |

Values, progress, switch positions, drawer edges and panel dimensions settle with **no overshoot**. A small
feedback glyph may have a restrained spring; routine UI never visibly bounces. Exits use the component's
established exit, rather than a global rule that would contradict the 160ms/200ms dialog contract.

One interaction has one coordinated movement. A second input retargets the current position, not the start
pose. Interrupted opening must reverse without a jump, duplicate overlay, replayed check or stranded focus.
Business state and accessibility state change when the owner answers, not when an animation ends. Leave an
exiting surface mounted only as required for its exit; it must not keep interactive descendants active.

#### Component recipes

| Pattern | Required behavior in Aster |
| --- | --- |
| Button and copy | Preserve the target and label baseline through idle, pending, success and failure. Exchange the inner glyph or label, not the whole button. Copy success uses the existing icon feedback in place; avoid a second success toast for the same foreground action. Failure remains readable |
| Popup anchor | A menu, picker, popover or tooltip trigger answers by color. Do not press-scale its positioning element; the popup must not re-anchor against a temporarily smaller rectangle |
| Segments and tabs | Move one neutral highlight between the existing options. Update `aria-checked`/selection at once. Scope shared motion to the control instance; initial paint and container resize do not replay selection motion |
| Expand and preview | Keep the trigger, title and origin visible. Expand supporting content on demand, then return focus to the trigger. Only non-modal inline regions use a height spring; dialogs and PageSidePanel retain the in-place dialog contract |
| Inline edit and confirm | Exchange read, edit, pending and result faces in the same place. Grow width only when the new content needs it, without stretching glyphs or text. Keep the error and draft on failure; do not animate a destructive action into completion before the owner confirms it |
| Skeleton to content | Reserve the real layout. Resolve the region with a short exchange; do not blur the whole page, grow from zero, or restart skeletons on same-query refresh. Use progress/status for long work, not an indefinitely performing skeleton |
| Empty and no-result states | Keep the unified illustration, explain the actual state and offer one primary next step. Use the existing 200ms / 4px row entrance once on mount; label changes do not replay it. Reduced motion shows the same content immediately; the illustration stays still after entry |
| Lists and selection | Use the existing row-motion contract, stable identities and a bounded first entrance. Changed rows move or tint once. Filtering, scrolling, pagination, count changes and an unrelated selection do not replay every row |
| Numbers and meters | Use tabular figures and reserve the value slot. Retarget the latest observed value without overshoot; screen readers receive the real value. Unknown, unit/currency changes and authoritative corrections switch directly rather than count through misleading values |
| Streaming text | Append actual chunks; settled text does not re-render for letter effects. Keep selection, caret and the reader's scroll position. A working label may shimmer; the answer itself does not shimmer, blur, bounce or retype after completion |
| Task and tool status | One active step carries the live signal. Pending, running, waiting for approval/input, stopped, failed and completed are distinct labels with stable controls. Freeze live motion while waiting; show a completion check only for the corresponding confirmed event |
| Sources and artifacts | Reveal an actual source or deliverable when it arrives. Keep the title, path, preview and open action stable. Fold finished execution details only without hiding unresolved failures, approvals or checks |
| Chart and terrain | Transition between the same records without inventing data. A 3D view needs a readable top view, visible legend, values, keyboard exploration, reset and explicit zoom. Drag follows the pointer; never auto-orbit while a person reads |

Treat object continuity as a relationship, not a reason to make content fly across a page. An added file,
memory or selected filter can be acknowledged at its destination with one quiet change. Preserve current
filters; do not clear a query or move focus just to expose the animation.

#### Aster identity and quiet companionship

Use the existing tenant mark and Aster assistant character. Do not add a second mascot, permanent floating
dock, or competing set of animated brand marks. The character may acknowledge work, waiting for input,
confirmed completion and failure through the same observed status used by the task. State text remains
available; facial expression and color are not the only signals. An unavailable status stays unknown.

Keep the character subordinate to the work: one brief acknowledgement when a state changes, quiet while
the person reads or edits, and no repeated celebration after a task settles. Continuous motion is reserved
for the single active work signal and the explicitly bounded home-background behavior in section 7. No
unrequested sound, full-screen celebration, persistent attention pulse or pressure to keep a streak.

#### Accessibility and rendering

- `prefers-reduced-motion: reduce` removes travel, press scale, shimmer, parallax, rolling digits, stagger
  and autoplay. Show the same final state, content, controls and errors immediately. A manually operated 3D
  view remains usable; remove camera interpolation and offer the same top view.
- Focus and reading order do not depend on geometry or motion. Use the established keyboard focus style,
  keyboard alternatives for dragging, and a polite announcement for meaningful state changes, not every
  elapsed-time tick, counter frame, token, hover or camera move.
- Prefer `transform`/`opacity`. Size animation is allowed only for a meaningful inline content change;
  measure at content or container changes, not on every frame. Do not animate `filter`, all CSS properties,
  or a popup's positioning transform. Keep the dialog's separate `scale` rule.
- Share clocks where a list needs time; stop animation work when the view is hidden, offscreen, occluded,
  paused or settled. Update one active row instead of re-rendering an entire transcript or collection per
  frame. Cancel listeners, timers and frame loops with the owning component.
- A status change never waits for an entrance, artificial typing delay, success dwell or exit to become
  readable or operable. A visual acknowledgement cannot postpone Stop, dismissal or an error recovery.

#### Fusion into existing surfaces

| Existing surface | Arc behavior to reference | Integration boundary |
| --- | --- | --- |
| Shared Button, CopyButton, SearchSelect and SegmentedControl | Action feedback, icon exchange, shared highlight | Improve the house control once; preserve its API, dimensions, keyboard behavior and owner callbacks |
| Current task, tool rows and approval cards | Agent status, Tool call, Agent plan, Agent approvals | Use existing run/tool/permission events and authority. Do not create a second task state, progress owner or approval system |
| Task result, Changes and file preview | Agent run, AI answer, Source cards, Code suggestion | Reuse the current result, diff and Office renderers; the completion moment leads to a real artifact |
| Existing model service and model configuration | Provider keys, Provider status, Model picker/card | One provider/model identity and configuration writer; show actual capability and pricing coverage rather than a new catalogue |
| Existing Usage and API gateway | Usage meter, Generation stats, Activity terrain | One local accounting source for totals, details and export. Budget, routing and keys link to their current editors rather than duplicating them |
| Work home, local memory and scheduled tasks | Starter prompts, Agent memory, Catch up, Agent routines | Add context-aware entry and return feedback to the existing navigation, preserving scope and saved state |

When changing one of these surfaces, record its actual trigger, state owner, interruption behavior and
reduced-motion result. A reference preview, this table, or a component test alone does not prove that a
product workflow has been integrated.


#### Evidence graphs and RSI evolution

Reference: [Entelligence World Model](https://entelligence.ai/), plus Arc's
[Timeline](https://uiarc.dev/components/timeline), [Stepper](https://uiarc.dev/components/stepper) and
[Accordion](https://uiarc.dev/components/accordion). Inspection of the public World Model drawing module
confirmed a Canvas 2D scene with projected positions, a focus lens, hierarchical branches, event paths,
arrival pulses and linked explanation cards. These observations describe the reference; they do not claim
that all of these behaviors already exist in Aster. Use the interaction relationships, not its source code.

The RSI documentation figure is an explorable account of a change. Its dark scene is a figure-specific
presentation, not another application theme. Keep the normal page and settings chrome on the roles above.
Default to the graph, one quiet case selector and optional playback. An explicit focus view may hide the
surrounding document. The evolution history and provenance stay behind separate, intentional disclosures.

| Relationship to preserve | Required behavior |
| --- | --- |
| Global graph to inner view | The selected point remains the spatial origin. Move the context graph aside as a lens expands from that point; preserve a visible connection into the detail. Closing reverses to that same point and restores focus |
| Overview to detail | The inner view represents a typed hierarchy: object, relationship groups, then concrete records. Place branches by their actual relationships and label them directly. Do not turn every explanation into an identical satellite or invent intermediate records to fill the circle |
| Event to feedback | One sourced replay event activates its path. A light point travels to its destination; arrival triggers one short node ring and updates the matching explanation. The path, marker and card belong to the same event identity |
| Current event to context | Emphasize the active chain and keep other relationships readable at lower contrast. Color distinguishes a relationship or result; distance, line weight and opacity convey hierarchy. Do not give every decorative connection equal brightness |
| Reading position to replay | A selected event stays selected while its explanation is read. User selection pauses narration. Dragging follows the pointer; camera movement stops while reading, hovering a target or opening history. Pause, previous, next and reset operate the same replay state |
| History to graph | Timeline markers, current-stage ring, expanded row and graph focus share one selected phase. Selecting a timeline stage focuses its nodes; drilling into a node closes the history panel without losing the selected case |
| Graph to evidence | A detail card names the observed change, the evidence and the result boundary. Source links belong to that record. Show counts, costs, durations and successful outcomes only when supported by the supplied records |

A semantic zoom may take 600–900ms to settle because the spatial relationship is the content. Camera and
lens dimensions settle without overshoot. Use one coordinated transition rather than separate popup,
background and text entrances. Opening a supporting timeline row uses the existing non-modal expansion
role, retargets if interrupted, and keeps its trigger fixed. Do not animate blur or make text follow an
unrelated decorative orbit.

The trace owner, not the drawing clock, advances event identity. Geometry can interpolate on a frame
clock, but the arrival feedback and card must advance together. A manually selected record does not
pretend a model or tool is running. Background mesh points are texture, not additional experiment records.

DGM and STOP reconstruction shows algorithm or evidence order, not invented wall-clock history. The Aster
engineering case links to a fixed commit and historical acceptance record; it is not proof of autonomous
RSI. Use ordinal stages when individual timestamps are unavailable. A visited marker means navigation
position, not a passed evaluation. Do not manufacture event counts, glowing success states or performance
curves to make a reference animation look busier.

Reduced motion shows the same hierarchy, active path, result and timeline immediately, with no camera
travel, autoplay, arrival ripple or moving light point. Pause and hidden/offscreen states stop frame work.
Keyboard users can reach each actual record independently of its projected position, move through history
with arrows and Home/End, dismiss a lens before dismissing focus view, and return to the original trigger.

Code 9.1.6 implements this as authored object/category/record relationships and narrated reading events.
The replay state owns the current record, travel, arrival and pause; it is not live execution telemetry.
Keep context labels in a separate unscaled layer. Reproject the smaller graph at the device pixel ratio,
retain 12–13px node/record type, and keep category and record labels horizontal.
A reduced context may omit inactive labels, but selected labels must remain readable. Lower background
mesh contrast before reducing the contrast or size of the evidence itself.

Case selection, return and playback share a toolbar in normal flow; the scenario occupies its own row.
They must not float over one another or over projected labels. Size the detail lens from the remaining
stage height and reserve space for its evidence card. Resolve projected label collisions without changing
the underlying relationship: move the caption and retain a quiet leader to its original point. Use neutral
graphite surfaces and fine cool rims; relationship colors belong to markers and the active path rather than
large tinted backgrounds. Keep the title, source and explanation at distinct reading levels.

An RSI overview must state the scenario, baseline and goal, identify the actual decision condition, and
show directed alternatives and their results. Label scenario, decision, option and outcome nodes directly.
Use arrowheads for direction, readable condition labels for branches, and a distinct return path for the
next iteration. A branch label opens its corresponding evidence. Keep algorithmic/configuration outcomes
separate from observed run outcomes; a reported endpoint is not proof that a displayed change caused it.

### List row

Height 36px (`h-9`), radius 8px, horizontal padding 10px (`px-2.5`), icon gap 10px (`gap-2.5`), label 13px.

- Selected: background `#f2f3f5`, medium weight, text `#171717`.
- Rest: text `#3f3f3f`.
- Hover: background `#f7f7f7`, text `#171717`.
- Nested work rows (tasks under a project): height 32px, left padding 32px.
- Stacks of rows use a 2px gap (`gap-0.5`).

Settings rows, plugin categories, work sessions, and the organization nav all use this row. A selected
settings row sets `data-active`. A selected category or session sets `aria-current="page"`.

A record list is the same 36px row with a hairline instead of a gap. Archive uses it: 16px horizontal
padding, a 1px `#efefef` rule under each row, a 16px leading icon, a 13px title, a 12px `#8a8a8a` meta,
and the quiet 30px actions at the end. Hover is `#f7f7f7` and `#171717`. Empty and no-match are different
13px `#8a8a8a` lines, with 24px vertical padding, in place of the rows.

Use the gap row for navigation. Use the hairline row for records. Do not mix the two in one list.

### Sidebar sub-menu

A navigation row whose destination has sections (`服务` → `智能体`, `小程序` …) owns a collapsible sub-menu.

- **Disclosure.** A separate 24px button inset 6px from the row's right edge, never nested inside the row
  button. The 14px chevron (stroke 1.7) is `#8a8a8a`, points right when closed and turns down (90°, 200ms)
  when open. Hover is `black/5` with `#171717`, so it still reads on a selected row. It carries
  `aria-expanded`, `aria-controls` and the name `{menu}子功能`; the row keeps its own name and
  `aria-current`.
- **Behavior.** Choosing a row opens its remembered section and expands its sub-menu. Only the disclosure
  collapses; choosing a row never does. A collapsed parent of the current section keeps the selected style.
  Several sub-menus may stay open. Open state is remembered across visits; before any choice, only the current
  menu's sub-menu is open. While a sidebar search is active, every matching sub-menu is open and its
  disclosure is disabled.
- **Motion.** The sub-menu's height animates over 200ms ease-out (grid rows `0fr` → `1fr` with a fade).
  Reduced motion switches at once. A closed sub-menu stays mounted but is `inert` and `aria-hidden`.
- **Sub-rows.** 32px tall, 13px label, 14px leading icon (stroke 1.7), 8px icon gap. They sit 18px in from
  the parent's left edge behind a 1px `#efefef` guide line. Every sub-row has its own icon, distinct from its
  parent's icon. A third level repeats the pattern one indent deeper, with its own disclosure.

### Scroll area

- **Edge to edge.** A scroll container spans its panel from edge to edge, and the panel's padding lives
  inside the scroller (`overflow-y-auto px-3 pb-4`), never around it. The thumb then runs along the panel's
  edge, against its divider, in that padding. It never sits over rows, chevrons, badges or row actions, and
  the panel never insets a scroller to make room for it.
- **Fixed regions.** A header or search field that does not scroll sits outside the scroller with the same
  horizontal inset, so its content lines up with the rows below it.
- **Ends.** Bottom padding is inside the scroller, so the last row scrolls clear of the edge and the track
  covers the whole scroll region.
- **Thumb.** The quiet bar from `scrollbar-quiet.css`: a 4px pill in an 8px track, `rgb(23 23 23 / 28%)`
  (46% on hover), with no arrows and no track fill. It shows only while the region is hovered, focused or
  scrolling. A host that paints its own overlay thumb sets `data-scrollbar="overlay"` and never sets
  `scrollbar-color`, which would bring back the thick system bar.
- **Embedded diffs.** Message-flow cards and workspace Changes use the same quiet horizontal bar as
  vertical scrollers, including hover/focus colors and transparent tracks. Apply it inside the diff's
  shadow root; global selectors cannot reach that surface. Only overflowing code reserves a bar.
  Workspace panes omit transcript-only folding/copy chrome and never inherit a folded card's body state.

### Segmented control

Track `#f5f5f5`, radius 8px, padding 4px, role `radiogroup`. Dark track `#2c2c2c`.

The selected segment is a white pill, height 22px, radius 6px, horizontal padding 12px, 13px medium,
text `#171717`, shadow `0 1px 2px rgba(0, 0, 0, 0.06)`. Dark selected pill `#3a3a3a` with text `#f5f5f5`.

Unselected labels are `#6f6f6f` and turn `#171717` on hover. On the plugin title row the control is size `sm`
and the track is 30px tall.

The selected plate follows the shared-highlight recipe in [Motion and emotional feedback](#motion-and-emotional-feedback).
The label, option widths and keyboard focus stay still while it moves.
The group has one tab stop. Arrow keys select the next enabled option and wrap; Home/End select the first/last
enabled option. Controlled values remain owner-authoritative. Reduced motion removes the plate's travel and
responds when the system preference changes while the control is mounted.

### Search field

Borderless, height 30px, radius 8px, fill `#f5f5f5`, no shadow. Focus does not add a border. The plugin field
is 220px wide and sits immediately left of the segmented control. Settings keeps that field collapsed:
a 36px search icon sits on the same row, to the right of 返回应用, and opens the field in that row.
Placeholder color is `#8a8a8a`.

The work page does not put this field in the sidebar. Search there is a 36px icon button that opens the search
dialog.

### Picker (dropdown)

Every dropdown is drawn by the design system. The operating-system menu (native `<select>`, `<datalist>`,
`document.createElement('select')`) never ships in a product surface: it ignores this palette, the radius
scale, dark mode and the motion rules, and it differs per platform.

- **Component.** `SearchSelect` from `@aster-ai-lab/ui` is the default for filters, toolbars, forms and settings.
  Use `Combobox` only when the trigger must read as a text field (free text plus suggestions). Vanilla shells
  and non-React packages draw the same anatomy themselves (Aster's first-run shell picker; `SelectControl`
  in `@aster-ai-lab/form`). The Radix `Select` composite is legacy: existing screens are tracked in a shrink-only
  list and new code does not add it.
- **Trigger.** Borderless field fill `#f5f5f5` (`--color-control-fill`), radius 8px, 13px `#171717`, value
  truncated with an ellipsis. Toolbar size `sm` is 30px tall (min 120px wide); form size is 36px and fills its
  column. A 12–14px chevron in `#8a8a8a` sits at the end and turns 180° while open. Hover deepens the fill;
  open and keyboard focus add the inset `black/10` 2px ring. Disabled is 40% opacity. Invalid adds the error
  border. An empty value shows the placeholder in `#8a8a8a`.
- **Panel.** The menu card: white elevated surface, 1px hairline, radius 12px, `shadow-elevated`, 6px padding,
  2–4px row gap, 6px from the trigger. It is at least as wide as the trigger, opens below and flips above when
  the viewport has no room, and scrolls inside at 160px (`sm`) / 192px (form) / 280px (shell). It portals or
  uses fixed positioning so a scrolling panel or dialog never clips it. It enters with a 150ms fade and 97%
  scale; reduced motion removes the animation.
- **Rows.** 32px (`sm`) or 36px tall, radius 8px, 13px. Hover and the keyboard-active row use the hover fill;
  the chosen row is medium weight on the selected fill (or carries a trailing check). A disabled row is
  `#b0b0b0` and cannot be chosen. A group label is 12px `#8a8a8a`. Long labels truncate; the full text stays in
  the accessible name.
- **Search.** Add the in-panel search field only for more than 8 options (`searchable={false}` otherwise). It
  receives focus on open and filters as the user types; no match shows one quiet sentence.
- **Clear.** Only an optional filter offers clear (`allowClear`); a required value never clears.
- **Behavior.** Click or Enter / Space / ArrowDown / ArrowUp opens with the chosen row active. Arrow keys move,
  Home / End jump, Enter chooses, Escape closes, Tab closes and moves on, a press outside closes. Choosing
  closes the menu and returns focus to the trigger. A choice applies at once — optimistic where it writes
  (section 14.10) — and never needs a separate confirm.
- **Accessibility.** The trigger has an accessible name (`aria-label` or a visible label), `aria-haspopup=
  "listbox"`, `aria-expanded` and `aria-controls`. The menu is `role="listbox"` with `role="option"` rows and
  `aria-selected`; the focused element (the search field or a select-only combobox trigger) points at the active
  row with `aria-activedescendant`. Option rows are not tab stops.
- **Labels.** Values are i18n labels, never raw enum ids, except identifiers the user typed or that are their
  own name (model ids, file formats such as PNG / JPEG).

Enforcement: `apps/studio/src/renderer/__tests__/noNativeSelect.test.ts` fails on any native dropdown in Aster,
`@aster-ai-lab/ui` and `@aster-ai-lab/intelligent-ui`, and holds the Radix `Select` list shrink-only;
`crates/flow/packages/ui/tests/form-select-control.test.tsx` does the same for `@aster-ai-lab/form`.

### Buttons

- **Quiet.** White, 1px border `#e5e5e5` or `#e6e6e6`, radius 8px, 13px `#171717`, hover `#fafafa`. Plugin
  install is 30px tall. The hero action is 32px tall with horizontal padding 14px.
- **Filled.** `#171717` background, white 13px label, height 36px, radius 8px. Disabled opacity 40%. This is the
  single confirm inside a dialog.
- **Field.** `#f5f5f5`, height 36px, radius 8px, 13px. 新建任务 uses 14px medium and hover `#ececec`.
- **Ghost.** No border. Hover `#f5f5f5` or, for a 24px row action, `#f2f3f5`.
- **Text.** 13px `#8a8a8a`, hover `#171717`. Used for 更多 and the plugin "more" affordance.
- **Send.** 32px, radius 8px, white arrow. Ready `#3a3a3a`, idle `#d0d0d0`.
- **Danger.** Text `#b42318` on an inline delete confirm. A PopConfirm danger confirm is the filled `#c23b3b` button. The row is not red before that confirm.

Icon-only buttons need an accessible name.

Pending and result faces follow [Motion and emotional feedback](#motion-and-emotional-feedback). Keep the
same hit target through the exchange; a popup anchor uses color feedback rather than press scale.

### Cards

Plugin card: radius 14px, 1px `#ececec`, white, shadow `0 8px 22px rgba(20, 40, 80, 0.04)`. Artwork is 132px
tall. Body padding is 16px horizontal, 12px top, 14px bottom. Title 14px medium `#1a1a1a`, one line. Description
12px / 20px `#8b8b8b`, two lines. The install button is the quiet button.

Feature card: radius 16px, 1px `#efefef`, padding 12px, shadow `0 4px 16px rgba(0, 0, 0, 0.04)`. Prompt tiles
are `#f4f5f7`, radius 12px (`rounded-xl`), hover `#eef0f3`.

### Page

A settings page is `SettingsContentColumn`: 24px padding, then one centered column at `max-w-3xl`. The title
is the 24px bold page title. The description is 15px `#8a8a8a` with 4px above it. The body is one or more
`SettingGroup` cards: radius 14px, 1px `#ececec`, white, 16px padding, the plugin-card shadow. The first card
has no top margin. The next card has 16px.

A split page keeps the 260px page sidebar of list rows and puts the subject in the other column. The
organization page and the work page use that split. Do not add a second sidebar width.

The page header holds one filled action. Other header actions are quiet. Refresh is the 30px ghost icon,
16px glyph, stroke 1.7. A page with no rows says so in 13px `#8a8a8a`. A catalog centers that line with 64px
vertical padding. A settings card uses 24px. A filter that matches nothing uses a different sentence from an
empty source.

### Table

A data table is the organization member table. It is full width, `table-fixed`, and left aligned. It has no
vertical rules and no zebra.

The header sticks to the top of the scroll container on white. It is 36px, 12px regular `#8a8a8a`, with a
bottom hairline `#efefef`. The first cell has 10px left padding. The last cell has 10px right padding.

A body row is 48px. Hover is `#f7f7f7`. The name cell leads with a 32px mark, radius 8px, fill `#f2f3f5`,
then a 13px medium `#1a1a1a` name. Other cells are 13px `#3f3f3f`. Time and status text are `#8a8a8a`. A
missing value is an em dash. A status chip is 20px tall, radius 4px, 11px, padding 6px horizontal. An admin
or selected chip is `#f2f3f5` medium `#171717`.

Row actions reuse existing buttons: an icon action is the 24px row action and a text action is the ghost `sm`
button. They do not invent a new button. Destructive actions require the reviewed
confirmation in section 14.9; use PopConfirm for a simple one-record action and ConfirmDialog when consequences
need more explanation. Neither deletes on the first click.

An empty source replaces the table. It does not leave a header over zero rows. A wide numeric table, such as
usage, may set a minimum width and scroll inside its card. Its numbers are right aligned, and a sorted
header sets `aria-sort`.

Management tables (section 14) add these rules:

- **Toolbar.** One row above the table: search and filters on the left; view switch, result count
  (`共 N 条`, 12px `#8a8a8a`), last-updated text, the 30px refresh ghost and the filled create action on the
  right, in that order. The count never takes a row of its own. A second row is only for removable active-filter
  chips, and only while filters are active.
- **Selection.** The checkbox column is 40px wide and leftmost. The checkbox is the default 16px box (radius 4px,
  12px glyph), never the 24px `lg` size. Extend its pointer target to 24px with an invisible pseudo-element
  instead of enlarging the box. A click anywhere in the checkbox cell toggles selection and never expands the
  row. A selected row uses the selected fill `#f2f3f5`; hover and an expanded row stay `#f7f7f7`.
- **Selection bar.** The batch bar appears only while rows are selected and floats; it never takes layout height
  or pushes the table down. Center it horizontally 16px above the bottom edge of the list viewport, in a
  non-scrolling layer over the scroll area, so it stays put while the table scrolls. It is one compact white row
  (radius 12px, 1px `#ececec` border, shadow `0 12px 32px rgba(0,0,0,0.12)`, ghost `sm` actions) that enters with
  a 150ms fade and 8px rise (none under reduced motion) and does not re-animate when the selection changes. While
  it shows, the scroll content gets enough end padding (80px) for the last row and the pager to scroll clear of it.
- **Row actions.** A 120px right-aligned column headed `操作`. The frequent safe action (`详情`) is a visible ghost
  `sm` text button. Other permitted actions sit behind one 24px `更多` row action (14px horizontal-dots glyph)
  that opens a menu, with destructive entries last in `#b42318`. Omit the menu when no other action is permitted.
  Every action has a record-qualified accessible name (`更多操作：供应商 Acme`). A dialog opened from the menu
  opens after the menu has returned focus to its trigger, so closing the dialog lands focus back on that row.
- **Detail dialog.** A record detail/editor dialog is anchored 10dvh from the top (max height 80dvh, inner
  scroll) instead of centered, so asynchronously loaded content grows downward without moving the title. While
  the record loads, it shows a skeleton with the same row count and line height as the loaded summary. Dismissal
  sets `open` to false and unmounts the content after the exit animation; never remove a dialog from the tree to
  close it.
- **Refresh.** A same-query refresh keeps rows, checkboxes and selection actions at full opacity and enabled; the
  spinning refresh glyph and `aria-busy` are the progress signal. Dim (50%) and gate rows and selection only
  while they belong to a previous query.
- **Performance.** Render rows as memoized components with stable callbacks; selection and expansion change only
  the affected rows. Keep a debounced search draft in its own component so keystrokes do not re-render the table.
  Derive column and renderer definitions once per resource, not on every render.
- **Row motion.** Rows enter with a 200ms fade and 4px rise, staggered 16ms per row for the first 12 rows.
  An optimistic write (section 14.10) sets one row state: `leaving` (200ms fade and 8px slide left, then
  hidden), `pending` (60% opacity, `aria-busy`), `updated` (`#effaf3` tint for 900ms) or `restored`
  (`#fef3f2` tint for 900ms). Tints fade over 500ms. Reduced motion removes the movement and keeps the
  states. Use `rowMotion` on `ResourceTable`/`DataTable`, or `rowMotionClassName`/`rowMotionStyle` on a
  custom row.
- **Pager.** Tables default to 20 rows per page (choices 10/20/50/100). The page-size picker is the
  design-system picker (`SearchSelect`/`Combobox`, `size="sm"`), not a native `<select>`.

### Transient popups

Menus, context menus, popovers and hover cards use the shared `@aster-ai-lab/ui` primitives.
An outside press closes the popup and performs the clicked control's action on that first press.
Outside clicks must not move focus back to the popup trigger. Escape closes a keyboard-opened
popup and returns focus to its trigger. Nested portalled content remains inside its parent popup.
Application focus loss dismisses transient popups. Confirmation and unsaved-change decisions use
the existing dialog components. Page code does not recreate popup dismissal handlers.

Boolean controls use the shared `Switch`, with its neutral checked colors in light and dark themes.
Do not draw a second switch with a checkbox, button or page-specific blue background.

### Dialogs

Use a dialog for a form, or for a choice that needs more than a title and one sentence. Removing a member
and approving a join use `ConfirmDialog`. Simple one-record archive, restore, uninstall and row deletion use
PopConfirm; compound, high-impact and multi-record consequences use `ConfirmDialog` as specified in section 14.9.

Scrim `rgba(0, 0, 0, 0.20)`, padding 24px. A clean, idle dialog closes on Escape or a scrim click. Dirty forms
protect unsaved input, and a pending submission follows section 14.9. The panel is white, radius 12px, shadow
`0 12px 32px rgba(0, 0, 0, 0.12)`. Use `role="dialog"` and the focus behavior in section 14.12.

A dialog enters and leaves where it rests. It does not slide in from the bottom. Enter is 160ms: opacity 0 to 1
and scale 0.98 to 1, easing `cubic-bezier(0.16, 1, 0.3, 1)`. Leave is 200ms: opacity 1 to 0 and scale 1 to 0.98,
easing `cubic-bezier(0.4, 0, 1, 1)`. The scrim fades on those same durations and does not move. Only `opacity`
and the `scale` property animate. Do not animate `transform`, `top`, `height`, `margin`, or `filter`. The
centering translation stays in place, and the frame stays on the compositor. `prefers-reduced-motion: reduce`
removes the animation. A utility `animate-none` on `DialogContent` still wins, for a surface that must not move.
It removes motion. It is not a second entrance.

`PageSidePanel` and any other desktop side panel with `role="dialog"` are this dialog. They use the
same in-place motion. They do not enter or leave by their own width (`x: 100%`, `x: -100%`, or a
`translate` of `100%`). A desktop side panel is not a bottom sheet. Use a bottom sheet only on a
phone-width surface that is attached to the bottom edge.

Anchored menus, popovers, selects, tooltips, hover cards, and the table selection bar keep a short
rise of at most 0.5rem. The offset composes with the positioning transform. It does not replace that
transform. Enter and exit do not set `filter`, including `blur(0)`. Do not use the `animate-in` or
`animate-out` keyframes on a dialog, or on a popup whose position is a `transform`. Those keyframes
assign the whole `transform` and a `filter`. Until a popper has a position, hide it. Do not paint
`translate(0, -200%)` on a visible layer. Do not add `slide-in-from-bottom` to a dialog.

A dismissal timer listens for the property that changes. A `translate` animation ends on `translate`,
not on `transform`.

A confirmation has a 15px medium title, a 13px muted description, then Cancel and one confirm, both 36px,
radius 8px, aligned to the end. The confirm is filled `#171717`. `destructive` paints that confirm as the
danger action. A plain confirmation hides the corner close. Return `false` from `onConfirm` to keep the
dialog open.

Form dialogs use one corner close: a 24px ghost button, a 14px X at stroke 1.7, 16px from the top and
the right. The accessible name is `closeLabel`. The control shows no word. A dialog that keeps the
default padding reserves 48px on the right so the first row clears the control. A dialog that sets
`p-0` reserves that 48px on its own first row. Footer Cancel and confirm stay text buttons.

A form dialog picks one size. `sm` is 384px, `default` is 512px, `lg` is 576px, `xl` is 720px. Fields inside
it are the field control: height 36px, radius 8px, fill `#f5f5f5`, 13px.

| Dialog | Width | Placement |
| --- | --- | --- |
| Create project | 320px | centered |
| Project / task Wiki catalog | content area | fills the content card; icon rail stays visible |
| Task search | 420px | top, 14vh from the top, max height 70vh |

The create title is 15px medium. Project and task More open the Workspace Wiki catalog, inside the shell
content area above its isolated workspace layer. Its list header has one working close icon at the right,
with no duplicate return-to-work action. A centered 24px medium title and a 36px search field lead into a three-column card grid;
use two columns below 900px and one below 560px of content width. Cards are at least 148px tall, have a
14px radius, a quiet neutral fill, 1px card border, 16px padding, 14px medium names and two lines of
12px secondary text. The catalog has no creation card or creation button; original creation actions stay on the work page. No invented summaries, stars or Wiki timestamps.
Unavailable Wiki state is explicit; existing object timestamps are labeled as project/task updates.
Wiki status comes from the saved binding and publication owner for the current bounded page, without
scanning source folders or starting generation. A saved generation says "Wiki available", not that
live sources are current; its own timestamp is labeled as a Wiki update. Busy and unavailable differ
from not generated.
The title has no explanatory subtitle, and the grid has no explanatory footer.

Both More actions stay visible for small and empty collections. Query the complete original collection
before returning a bounded page (20 by default); tasks keep the inbox scope, with project tasks reached
through their project. Show distinct loading, unavailable, empty and no-match states. Escape first clears
search, then closes. The shared dialog owns keyboard focus; background work stays mounted and inert,
its commands are suspended, and closing restores the trigger. Enter/leave fades in place and honors reduced
motion. The independent task search stays a 420px top dialog at document z-index 220.

Task history renders as soon as its saved session and messages are available. Model initialization or refusal never replaces history with a page-level skeleton. The composer area shows a compact loading or recovery state until the original runtime admits sending; unlinked historical sessions remain readable without a composer.

Workspace cards open that object's Wiki detail in this same layer. The former Knowledge page is the
workspace Wiki reader. Reuse the existing KnowledgePage, tree, search, editor/reader, graph and maintenance components directly; do not replace them with a second detail layout. Its root comes from the selected workspace, never the background task or global preference. Browsing an empty scoped vault must not create directories.
The detail uses one 48px header row containing a truncated workspace name and inline path, original page actions, resource tabs and close. There is no additional overlay header, back button or workspace-open button; narrow windows hide the secondary path rather than wrap. Resource order is Analysis / Encyclopedia / Outputs / Memory / Board. Analysis opens by default and presents measured execution metrics, event filters, timeline playback and event details from the selected task's real messages; project scope selects one of its tasks before analysis. Outputs opens the selected folder's actual file tree. File editors reuse the shared workspace tabs, preserve open documents across resource switches, and guard unsaved closes. Selecting a file tab also selects its tree entry. File tabs use the same extension-specific document icons as the file tree; their context menu supports closing through the existing dirty guard, copying the path and revealing the file in the sidebar. Dialog content explicitly restores pointer events inside page-level portals so save/discard confirmations stay operable. Embedded Markdown omits the redundant read-only preview/line-count bar already covered by its host toolbar. Code save/download actions share the existing bottom status bar instead of reserving a second header row. Empty editor chrome never reserves height.
Outputs, Encyclopedia and Memory share a mouse-draggable sidebar seam, a remembered 260px default, a 200px minimum and a maximum of 480px or 45% of the available content width. The seam is keyboard accessible and cleans up drag state on blur or exit. Encyclopedia tree rows match Outputs: 32px height, consistent indentation, document/folder icons and a quiet selected fill. Reader/tab action rows stay 40px tall without wrapping; long labels truncate. Memory opens with readable prose, explicit editing, compact time/type/tags and collapsed technical details. It has no redundant displayed/total counter.
Keep the catalog mounted so Escape restores its query, page, scroll and selected-card focus. Returning refreshes that bounded owner page while retaining its existing cards; query/page changes clear old results, late responses are ignored and failed refreshes expose retry. Reading a Wiki does not change the background selection. An unavailable Wiki shows its real empty state,
not sample pages or a pretend generation. Knowledge is removed from the ActivityBar, including stored
and custom rail orders; old unscoped Knowledge routes return to Work.

The Board is a derived view of the existing task owner, scoped by project ID or exact task ID. Project cards are real sessions from the bounded owner query (20 per page), searched by stored name/description and filtered by the exact execution status before paging. Column counts describe the loaded page, not an invented collection total. Task cards are the latest structured agent plan, retaining original step IDs and states; the existing conversation source supplies live updates and bounded older-history continuation. A missing plan is explicit and can open the original task. Invalid plans or unavailable reads never become successful empty boards.
All five scoped resources use the shared EmptyState inbox illustration at 64px, a 14px muted regular title and the same 16px illustration gap. Center it horizontally and vertically in the remaining main content area, with a flex-filling parent. No resource substitutes a large standalone Lucide icon or a top-aligned empty block. A populated board may retain quiet empty-column text; a completely empty board uses the common centered empty state.
The file-tree context menu uses the shared anchored Popover inside the owning dialog, with collision handling and an interactive portal boundary for its nested confirmations. Clicking outside or Escape dismisses the menu without closing the Wiki. Creating or renaming inline receives focus; Escape cancels that edit before the Wiki's catalog navigation runs. Do not portal an interactive file menu directly to body outside a modal's pointer/focus boundary.
There is no standalone Encyclopedia question pane. Ordinary tasks use the existing scoped `search_wiki` tool. The Encyclopedia sidebar graph button and graph tab are the graph entry points; do not repeat them at the right edge or show the legacy related-information rail for a workspace Encyclopedia.

`search_wiki` and `search_memory` are optional evidence tools: the agent decides whether a task needs retrieval. Tool availability or index preparation does not trigger a search. The message flow displays only actual calls, with localized query, scope, wait/run/result state, available coverage counts and numbered evidence cards. Incomplete coverage never becomes a complete no-match. Citations show the original task/workspace, page or independent memory layer/type; source details disclose path, time and available generation/revision. Source-workspace actions reuse the Encyclopedia/Memory surface and reject changed bindings. Search text stays inert evidence; no cross-workspace path is silently opened in the caller's editor.

Memory reading uses a readable Markdown column. Layer and type are independent labels; creation time remains visible, useful properties have named values and confidence/importance use human-readable scales. Storage paths, identifiers and raw metadata live in secondary technical disclosures. Editing provides explicit save and cancel, preserves revision conflict feedback and discards only the current draft on cancel.
In an admitted Studio session, `search_wiki` defaults to `scope=all` over the current profile's saved active task and project bindings; `scope=current` limits it to the caller's workspace publication. The model supplies a query and optional continuation cursor, never roots or another profile. The native owner resolves each binding before and after reads, and the original ACP WikiIndex/aster-vec engine admits cold published collections. One call considers at most 32 bindings within a 3-second budget, returns at most 12 attributed page passages and 48 KiB of hits, and exposes pending/continuation coverage. Per-index relevance ranks are interleaved; they are not presented as a global similarity score. The bounded cold set retains 32 original indexes per ACP process. Personal `wiki/local`, source packets, historical files, unpublished candidates and archived/deleted/rebound task owners are not search collections. An incomplete search is never displayed as a complete no-match. Bare ACP clients without the Studio capability retain current-workspace search.
Outputs are the workspace's source files; compiled Encyclopedia pages retain the original OKF format and owner. PDF/image/Office conversion supplies bounded frozen evidence with page, slide, paragraph or cell coordinates and never rewrites those source files. Native Office semantics and the existing local document worker are preferred; explicitly configured document processors may be used through their existing settings. Unsupported, failed and truncated inputs remain visible in coverage, and OCR/conversion observations require factual review before publication.
Memory layer and memory type each have a separate 24px icon button in the sidebar header. Each opens its own short, single-choice menu directly; the selected row is checked, the current value is announced and shown in the tooltip, and a non-default filter marks its trigger. All layers merges bounded pages from the original three stores and retains each row's layer for reads, edits and deletion; changing type never changes layer. Escape or choosing an option closes only that menu and restores trigger focus. The reader retains its 40px “Memory details” header when empty, loading or failed; separate labels identify the selected record's layer and type. Readable Markdown prose uses a 720px measure and 16px text, with created time visible and tags/technical storage fields collapsed. Stored HTML and remote images remain inert. Edit/Preview, Save and Delete use the existing small-action style: 24px targets, 14px Lucide outlines at stroke 1.7, radius 6px, quiet hover/focus states and tooltips. File-tab menus close when their owning resource view hides.

The Encyclopedia compile action admits the original workspace-bound maintenance controller once, disables while queued/running, selects Board, where that job opens its detail. Board combines original conversation execution, compile controllers/ledger and original memory-extraction FIFO observations; it does not create another queue or treat an unknown outcome as success. Project scope includes its own and its child sessions' exact compile bindings. All three task types share one status/time/action header and Process/Results dialog; process reuses original conversation messages and approvals, results use original outputs or generation-checked compile review. Historical compile jobs without a saved conversation show their durable job summary. Memory extraction observations are bounded runtime state, not a durable history across restarts.

Studio's bundled `stc-answer` (“知识理解”) answers with short prose, then an optional scriptless hand-drawn HTML/SVG fragment for relationships, then qualifications. Simple factual questions remain text. The skill keeps its upstream writing rules and standalone-page exporter; assets ship through the existing bundled-skill inventory and native dev resource generation. File-backed skills carry their canonical resource directory through capability projection, without granting resource reads. Task conversation HTML fences reuse `MessageHtmlArtifact` and its existing sandbox/CSP and document consent boundaries; incomplete fences remain source. Never inject generated HTML into the host DOM or add a second preview renderer.

All task kinds share the Task / Results dialog and the default delivery path: execution → validation → review → delivery. Workspaces do not require Git and never receive an automatic repository. Each stage uses its original owner's recorded results; execution completion alone never qualifies an unrecorded validation, review or delivery. Compile review and publication remain generation-checked; memory delivery uses its actual extraction outcome. The Aster tenant uses the product mark and name. The dialog content/scrim sit above the full Wiki surface at z-indices 91/90. Git projects may opt into the CI / PR evidence view for the exact checkout and commit. Unknown, stale, missing and truncated records remain explicit. Current GitHub Actions runs are bounded, newer runs supersede older attempts of the same workflow/event, and an old PR head never qualifies the current commit. Opening details is read-only and never commits, pushes, creates a PR or merges.

Task row hover cards request only that task's summary after the hover/focus delay and unmount the reader when closed. They show published Wiki pages, processed knowledge sources and workspace memory counts beside recorded conversation, usage, tool and artifact facts. Wiki counts use the exact task binding; shared workspace memory counts are labelled as shared. Missing layers/timing remain unknown, including the aggregate memory total when any layer is missing. A refresh action is available; open live cards refresh at 2.5 seconds and idle cards at 10 seconds. The original message owner retains at most 64 derived summaries and invalidates the affected summary on every message mutation. Wiki statistics read bounded metadata rather than project source bodies. Memory counts use existing owner metadata, never start an ACP session, and respect a shared read deadline. The card has a bounded viewport height and scrolls when needed.

The Board toolbar uses the shared 30px search, status picker, reset and refresh. Four columns (Pending, Running, Needs attention, Completed) have 13px medium headings and muted counts. Status icons and text share semantic foreground tokens: blue for running, amber for attention/input/unknown, green for completed, red for failed, and muted for pending/cancelled/skipped. Approval, input, failure, cancellation and unknown each have a distinct Lucide glyph; status is always readable as text. Cards use white, a 1px card border, 8px radius, 12px padding, a 13px title and 12px status/time metadata; no invented PR or agent facts. Columns have a 240px minimum and scroll horizontally at narrow widths. Loading, empty, no-match and failure are distinct. Search and filters remain available in every state; changes reset the project cursor. A card opens that task record's actual Outputs folder in the same Wiki, retaining the surrounding project context. Project tasks may use the shared project root or their own original task-ID folder; other folders fail closed. After opening multiple roots, a compact work-directory picker in the existing header switches between the project and visited task folders. Each folder retains its own mounted file tabs, buffers and dirty protection; the inline path follows the active Outputs folder. Its separate Open task action navigates to the original task; lifecycle/approval commands remain there. Neither board cards nor sidebar task rows can be dragged to change execution or project membership. Project association is chosen when creating the task; initial private-directory setup does not move existing files. Paused, failed, skipped, cancelled and unknown work never appears as completed.

### PopConfirm

PopConfirm is the anchored confirm on the control that starts the action. The panel is `role="dialog"`,
width 18.5rem, radius 8px, 1px `#e5e5e5`, white, and the dialog shadow. It prefers to open below the trigger
with a 10px gap, and flips above when the viewport cannot hold it. While idle, Escape, Cancel, and a click
outside close it without calling `onConfirm`. While pending, gate dismissal and repeated submission as specified
in section 14.9. A thrown confirm stays open.

The title is 13px medium `#171717`. The description is 12px `#8a8a8a`. Actions sit at the end: Cancel, then
confirm. Both are the 30px small button, 13px. `tone="default"` has no icon. The workbench archive uses it.
`tone="danger"` adds a 16px warning glyph in `#c23b3b` and fills the confirm `#c23b3b`, hover `#a83232`.
Uninstall, and archive delete, use danger. Pass `boundary` when the panel must stay inside a sidebar. The
workbench archive uses `#workbench-sidebar`.

Do not open a dialog for that same one-line confirm. Do not put a form in PopConfirm.

## 7. Work page

White page. The left column defaults to 260px, can shrink to 220px, and draws a 1px `#efefef` edge on the right.
The column padding is 12px horizontal, 8px top, 16px bottom.

Top row: 新建任务 fills the remaining width (field button, 14px medium, `MessageSquarePlus`). A 36px search
icon button sits to its right.

项目 is a 36px header row: folder icon, 13px `#3f3f3f` label, a 24px plus on the right. 任务 is the same header
with a 13px `#8a8a8a` 更多 action. Empty copy is 13px `#8a8a8a` ("还没有项目", "尚无任务").

Session rows use the list row. A current session is the selected row. Each row leads with a 20px black circle,
then the title. The relative time is 12px `#8a8a8a` and hides while the row is hovered so Wiki, rename, pin,
and archive can show. The row has no delete control. Day headings stay above groups of tasks.

Project and task rows reveal a Wiki shortcut on hover or keyboard focus, using the same 14px Lucide
PanelsTopLeft glyph, 1.7 stroke and 24px ghost button as the neighboring row actions. The project
new-session action uses MessageSquarePlus. A shortcut resolves the original project/task record by
ID before opening the existing Wiki details; it does not fabricate a path or navigate the background
conversation. Missing or mismatched records show a retryable error. Project scope shows the project
root and all its task cards; task scope shows that task's actual directory and execution/plan. If
both scopes share a directory, Outputs and folder-scoped Memory can match; Encyclopedia binding and
Board scope remain distinct.

A 420ms hover opens a 304px card 10px to the right of the row: the task title on top, then 本地电脑 with a
代码开发 badge when the task recorded a code session, 所属空间, the branch it runs on, and the absolute update
stamp (`2026/10/25 12:04:30`). The card reads facts the row already holds plus that session record; hovering
never fetches. Rows with an unusable stamp omit it rather than print a placeholder.

The main column is white, horizontal padding 32px, and centers its content. The greeting, composer, and feature
strip share a 920px column. The composer is centered inside that column at 760px. The three feature cards fill
the 920px column in a 3-column grid, gap 12px, with 40px above the grid.

Aster's home wordmark uses the same monoline geometry and flat terminals as the ToWork mark,
at 64px high with 36px below it. The shared tenant mark elsewhere keeps its existing identity.

The home uses a single neutral canvas color with a restrained blue-gray cloud, never separate warm
and cool upper/lower panels. A decorative Canvas 2D starfield gives the background depth: perspective
projection, larger/brighter near stars, subtle trails and pointer parallax. Initial stars populate
all depths inside the viewport. The tilted orbital plane stays behind the content. Pointer events
pass through this layer. Painting is capped at 30 fps, 180 stars and a 1600 by 1200 framebuffer; no
React render runs per frame. Hidden/inactive windows and a covering Wiki pause animation. Reduced
motion draws a still sky. The dark theme uses the same layout with adapted luminance.

The static composer is a 14px-radius card with a `#ececec` border and the card shadow. The textarea is 15px,
placeholder `#c5c5c5`, padding 20px horizontal and 16px top. The toolbar is a row of 32px ghost controls. When
the live composer is mounted it occupies the same 760px slot.

A shallow footer band attaches below both the live and loading composer. Its light background is
`#f0f1f3` (`#282a2e` in dark mode), distinct from the page and the white input card. Project/directory
selection sits on the left; the canonical draft or durable `/goal` objective sits at the right.
The input's command token, footer objective and sent-message goal tag share a gold surface and Target
glyph; live state remains available as text and a status dot. They never create a second goal owner.
The scope menu offers No project, which restores a private task directory instead of inheriting the
last project. Task conversations keep their directory locked. In a Git project the current-directory
mode can switch or create local branches directly; worktree mode keeps its independent-directory
workflow. The native checkout command rejects dirty files and never forces, stashes or discards them.

Send and stop are the same 32px ink disk, differing only in a solid white glyph: an arrow up to send, a 10px
rounded square to stop. One disk, one glyph weight — swapping the two must not read as a different button.
A send that cannot fire yet drops the disk and keeps a muted arrow; stop is always the filled disk. The task
conversation has no standing AI note below the card; its dock alone owns the bottom spacing. An empty
Aster task uses the tenant logo above its greeting, while other tenants retain their assistant avatar.

Home opens directly in code development. It has no work-mode switch or office mode; the wordmark leads
directly into the composer. New drafts use the existing current-directory/worktree workflow and local
Agent. Old office or unknown mode selections resolve to this default; explicit installed-app actions
continue through their existing handlers and cannot change the default host runtime.

Quick tasks are one row of chips directly above the composer, left-aligned with it, 12px above it. A chip is 32px,
fully rounded, white, `#ececec` border, 16px `#3f3f3f` icon, 13px label, at most 220px with an ellipsis and the full
title as a tooltip. The row scrolls sideways and never wraps. While chips are hidden past an edge, that edge fades
to white and a 24px round `#f0f0f0` arrow button follows the row: it scrolls forward 60% of the row, and at the end
it turns back to the first chip. The row has no search field.

The three cards are AI 写作, PPT, and 超级PDF. Their marks use the feature colors in section 2. The page does
not show a mascot and does not use 夸克 in those titles.

添加 opens a launcher that fills the work surface, the content card under the icon rail. It is a light frosted
sheet (`absolute inset-0` on the workbench). A 30px search field, 280px wide and white, sits centered near the
top. Applets are a centered grid. Each tile opens that applet. 固定 and 解除固定 are text buttons. The home
strip shows only the pinned applets, then 添加. Escape, or a click on the empty sheet, closes the launcher.

## 8. Plugin page

White page. The header block is padding 24px horizontal and 16px vertical, with 16px between the title group and
the body.

Title row: 24px bold title on the left. On the right, the 220×30 search field, then an 8px gap, then the
segmented control. The segments are 市场 and 已安装. A 15px muted subtitle sits under the title. The header does
not contain refresh, manage, or create.

The category list is a 176px column, padding 12px horizontal, 8px top, 16px bottom, using the list row. It stays
on the left. The content column is padding 24px horizontal, 16px top, 24px bottom.

The home market view opens with a hero: minimum height 168px, radius 16px, padding 32px horizontal and 24px
vertical, gradient `radial-gradient(80% 130% at 100% 0%, #c5dcff 0%, #e4f0ff 42%, #f6f9ff 100%)`. Hero title
22px semibold `#1a1a1a`, subtitle 14px `#8a8a8a`, then the quiet hero button. This gradient belongs to that hero.

The collection heading is 18px semibold `#1a1a1a`. Cards are a 2-column grid with a 16px gap, becoming 4 columns
from 1080px. Empty results are centered 14px muted text with 64px vertical padding.

## 9. Capabilities page

Work-sidebar 能力扩展 opens this surface in the work canvas. The sidebar stays, and 新建任务 returns to the
composer. It does not navigate to the plugin store. The settings plugins item still routes to `/app/capabilities`,
which renders the same page.

The arrangement follows the MiniMax marketplace. The controls are the ones in section 6. Connectors and skills
are capabilities, not products: there is no price, listing, or purchase. The catalog and each member's install state
come from the OS (`/api/org/capabilities`) through the shell routes `GET /api/capabilities` and
`POST /api/capabilities/{slug}/install|uninstall`; operators curate it at `/api/admin/capabilities`. The catalog
starts empty.

The OS owns install intent. The Tauri shell owns local materialization. On startup, on every catalog read, and after
each install or uninstall, the shell reconciles the member's installed set into
`<app data>/<tenant>/capabilities/`. Connectors go to `mcp.json` (`mcpServers`, either stdio `command`/`args` or a
`http`/`sse` URL). Skills unpack into `skills/<slug>/`, downloaded from `GET /api/org/capabilities/{slug}/bundle`
(stored in RustFS). The shell checks the bytes against `install.digest` before unpacking and pins the result with
a `.aster-digest` marker, so a replaced package is fetched again on the next reconcile. If the OS records an install but the device can't apply it, the shell answers
502, and the page re-reads the OS so the toggle reflects the recorded state.

A pinned toolbar holds the size-`sm` segmented control (连接器, 技能) on the left. On the right, refresh, 管理,
and 创建 share that 30px height. Refresh is a ghost icon button with a 16px glyph. 管理 is the quiet button.
创建 keeps the filled `#171717` color at the same height. The 36px filled height stays on dialog confirms.

Under that, a centered `max-w-[1000px]` column holds the 24px bold title, a 15px `#8a8a8a` subtitle, a full-width
30px search field, and horizontal chips. A chip uses the list-row selected and rest colors at 36px. 精选 lists
every section, featured first. Choosing a chip scrolls to that section. It does not hide the others.

A section heading is 18px semibold `#1a1a1a`. Entries are a flat two-column list, not plugin cards. A row has a
40px provider mark, a 14px medium `#1a1a1a` name, a 13px `#8a8a8a` description, and a 30px button with a 14px
glyph at stroke 1.7: 安装 is the quiet white button with a download glyph; once installed the button is filled
`#171717` with a white 卸载 label and a trash glyph, and opens an anchored `PopConfirm` (danger tone) instead of
uninstalling directly — confirming runs the uninstall and re-reads the catalog so the shell reconciles local
materialization. A row lifts on hover to `0 10px 28px rgba(20, 40, 80, 0.10)`. A mark without a pictogram shows
the name's first letter. Overflow uses the text button
查看全部. Loading, a failed load with 重试, and a signed-out prompt replace the sections in the same centered muted
text. 管理 and 创建 leave the surface: connectors open MCP settings, skills open
the skills catalog.

## 10. Settings page

Settings floats over the content card. The icon rail stays visible. The work or plugin page underneath stays
mounted. Opening settings does not clear the selected rail item.

The overlay is `data-ui="settings.overlay"` inside the content area, above the page and not over the rail.

- Scrim fades in over 220ms ease-out and fades out over 180ms ease-in.
- The panel rises 12px and scales from 0.985, over 280ms `cubic-bezier(0.22, 1, 0.36, 1)`. Close moves it down
  10px over 200ms `cubic-bezier(0.4, 0, 1, 1)`.
- 返回应用 starts that close. The settings tab goes away when the panel animation ends.
- `prefers-reduced-motion: reduce` skips the animation and closes the tab immediately.

The settings view is white, text `#171717`. The nav is 260px, the shared page-sidebar width
(`PAGE_SIDEBAR_DEFAULT_WIDTH`), with 12px horizontal padding and a 1px `#efefef` edge on the right. The
background is `--settings-nav-background` (`#ffffff`). Content on the other side of that edge is the same white.

返回应用 is the list row with a 16px back arrow, stroke 1.7. A 36px search icon sits on that same row,
to its right. Opening search replaces 返回应用 with a 36px search field and turns that icon into a 36px
cancel button. 偏好 is the first group label. List rows do not add their own top margin. The first group
label uses a 4px top margin. Each later group label uses 12px. Rows under a label keep the 2px stack gap.
Managed settings keep the existing tenant-specific destinations; the local Aster exception follows below.
Nav groups are
12px / 16px `#8a8a8a` labels, then the list row. The active item is the selected row. The visible groups are
偏好 (通用, 外观, 快捷键, 用量), 连接 (渠道, 数据), 开发者 (自主进化, 执行轨迹, 环境依赖), and 系统 (归档, then 关于). 渠道 opens `/settings/channels` and that page draws a channel sidebar for 飞书, Telegram, QQ, 微信, Discord, and Slack. Each message channel binds ToBot and does not ask which agent to use. 数据 opens `/settings/data` and that page draws a data sidebar in the previous order: 数据目录, 本地备份, WebDAV, 坚果云配置, S3 兼容存储, 导入外部应用数据, 导出菜单设置, Markdown 导出, Notion 设置, 语雀配置, Joplin 配置, Obsidian 配置, 思源笔记配置. Configured backup and note channels belong to the default ToBot: naming one reads a short index, and asking to save or back up writes after the reply. A channel with missing credentials is not described as connected.
编码, 声音, 账户, and a plugins, skills, MCP, or agents settings group are not in this menu.

Local Aster exposes the existing 模型服务 at `/settings/provider` and 模型配置 under 智能. 模型配置 keeps
默认模型, 路由计划 and 本地预算 in one page; switching a view preserves the existing provider/model identities
and saved defaults. 用量 stays under 偏好 and reads the local accounting source. API 网关 lives under 连接
and links to those same model, route, budget and usage editors. It does not own duplicate configuration or a
second usage dashboard. A local account or cloud billing flow is not added.

Settings destinations have distinct outline glyphs, shared with their settings-search entries: sliders for
General, sun for Appearance, keyboard for Shortcuts, bars for Usage, server for Model services, CPU for Model
configuration, globe for Web search, text document for Document processing, messages for Channels, network
nodes for API gateway, database for Data, archive box for Archive and info for About. At 16px / stroke 1.7,
the glyphs follow [useAnimations' hover/reverse interaction](https://useanimations.com/icons/settings): the
whole row's hover or keyboard focus moves one small part, then leaving reverses it. Only the glyph moves;
the hit target and label stay still. Selection does not loop or replay the gesture. Touch hover does not
trigger it, and reduced motion removes the gesture immediately. Reuse the existing SVG and CSS mechanisms.

Each row opens the matching settings job. 通用 keeps launch, proxy, and developer controls, and adds 应用,
权限 (电脑操控权限 and Agent 控制权限), 语言 / 设置应用语言, and 关于. The 语言 row jumps to that control on
通用. 通用 also shows 工作区管理. That card shows the application workspace folder and lets you change it. When it is not set, the folder is `.aster` in the home directory. 外观 keeps light, dark, and system, the color, the UI font, and the code font, and adds 主题, 界面密度,
对话最大宽度, and 页面对比度. 快捷键 lists bindings, rebinds them, clears a binding with Backspace while
recording, and reports a conflict. 个性化 loads, edits, and saves 自定义指令; a failed save or a failed retry
keeps the draft. 浏览器 keeps the toggles, clear, and a visible save failure, and shows 从 Chrome 导入 Cookie
when the platform allows it. 代码审查 opens `/settings/code-review` and chooses 当前会话 or 子会话. 工作树
opens `/settings/worktree`. 归档 lists archived tasks in the same settings card as the other pages:
a 30px search field, a 30px project field, quiet 30px actions with 14px icons, and 36px rows that lead
with a 16px icon. Restore and delete ask in a PopConfirm before they run. macOS menu-bar and Computer Use TCC prompts stay out;
the permission rows are on-device switches. Windows-only controls are not added.

The content title is the page title, padding 24px horizontal, 16px top, 8px bottom. A description under a header
is 15px `#8a8a8a` with 4px above it. The content body uses 24px padding.

## 11. Window chrome

These pages sit in the icon-rail frame.

- The rail is 60px, canvas `#e6e6e6` (dark `#242424`), with no border on its right edge or bottom.
- The content column insets 4px on the top, right, and bottom (`pt-1 pr-1 pb-1`) and has no left padding, so the
  gray beside a rail icon matches the gray on its other side.
- The content card is radius 10px with a 0.5px border. On this window the outer corner is about 14px and the card
  is inset 4px, so the 10px radius shares that corner's center.
- A wide labeled sidebar, when one is already open, uses radius 12px, a full 0.5px border, and 8px bottom padding.
  Dragging the icon rail does not expand it into that sidebar.
- Rail tooltips are a black pill, white 13px medium text, no arrow.
- The first visible rail item is 工作. Wiki belongs to the selected workspace and has no ActivityBar entry. When a team is selected, the managed rail keeps 消息, 团队 and the permitted account destinations after 工作. 日程 and 邮件 live in the team page sidebar. In the personal center, 团队 stays off the rail. In the local Aster tenant, 资料 follows 工作 and opens the personal library at `/app/documents`. Managed 文档 stays off the rail. The personal collection is `materials` under the configured workspace; task/project files retain their existing owners. Its file viewport fills the available pane, with a sticky header and bottom pagination; it has no onboarding cards.
- The personal library heading is 资料. Its folder tree and file list reuse the task workspace's `DocumentFolderIcon` and `DocumentFileIcon`, including type mapping, dimensions and expanded-folder state. Right-click actions target the actual row or folder, preserve a multiple-file selection, and confirm recoverable removal. Root and blank-space menus offer creation, import and refresh without file-specific actions. Page-size menus anchor beside their trigger within the viewport.
- Opening a personal-library file uses the shared editable document surface inside the library, with retained file tabs and native document save receipts. The compact editor toolbar places Back and Hand to Agent on one row. Switching files or library views retains unsaved buffers; closing a dirty file requires confirmation. Rename, move, removal, search preparation and task handoff require saved content. Import and creation keep the current collection; creation opens the new file in place. Narrow panes hide secondary metadata while preserving file actions.
- Search contents and Library wiki are sidebar destinations in the same page. Back preserves the folder, filename filter and selection; opening an original from search returns to the same question, result and captured source scope. Searches may finish in the background. Wiki reading tabs and filters remain mounted across view changes, with shortcuts enabled only while visible. Tasks created from library files reuse the existing conversation and composer in the library, retain their sidebar entry, and submit their staged originals once. Hidden conversations keep their state without claiming the page's shortcuts. The Content search column offers Enable search or Retry for supported files and shows Searchable after confirmed processing. Unsupported files do not offer the action. Model selection stays in place when no default model is configured.
- Personal originals and derived encyclopedia pages remain separate. Aster continuously admits personal materials into the existing knowledge compiler, including first-time compilation without opening the page. The 资料百科 sidebar destination reuses the published knowledge reader, citation inspection and maintenance controls. Generated Wiki and index folders are excluded from source admission. OCR and parsing failures remain visible; partial evidence never claims complete extraction. The original ACP `search_wiki` includes published personal knowledge in `scope=all`, while `scope=current` retains the selected task only. Retrieval is an agent choice. Library evidence identifies its own profile-bound library instead of inventing a task or project, and opens the corresponding personal encyclopedia.

## 12. Page entry

A view that is not ready shows one legacy wordmark. The mark is a left-to-right stroke of the letters, in ink
`#303030` on white. Dark uses `#f2f2f3`. This is the Aster page entry. The shared `PageLoading` in
`packages/ui` stays a tenant-neutral mark and does not draw a tenant wordmark.

### Local workspace gate

The Aster build has no gate at all. There is no welcome screen, no sign-in page, no lock dialog and no product
menu: the mark at the top of the rail is a mark, not a button, and the app opens on the work bench with the
rail, the task list and the composer already live. The only chrome a personal build adds is the mark itself.

## 13. Work surface locks

Work uses the chrome in this file. The locks below are the Work product structure. They live here so Work does
not keep a second design spec.

The agent conversation surface pairs a conversation pane with an optional task workspace. Conversation is
primary. The right pane is `TaskWorkspaceShell` only, not a second chat and not a permanent second column.

### Inspector & Projects IA (locked)

| Concept | Single meaning |
| --- | --- |
| Right pane | `TaskWorkspaceShell` only. Mount only while a substantive tab is open (`browser` / `terminal` / `changes` / `file` / `document` / `office-document` / `lineage`). Empty placeholders must not reserve width. |
| Layout ownership | `AgentConversationWorkspace` owns open, ratio, and fullscreen chrome. Run-side effects belong to `RunWorkspace`, not the layout shell. |
| Projects | Durable folders in the home sidebar management group (`WorkSidebarProjects`). Not an Activity Bar destination. Project-owned tasks stay out of the general task inbox. |

Legacy `AgentTaskDetailPanel` is not the production right pane. Do not mount it beside `TaskWorkspaceShell`.

Composer 自动审查 (`reviewMode`, Claude Science Auto-review / host Science Reviewer) persists through settings
into `RunRequest` / native `review_mode`. Triggers are product intent only: Auto-review after a response the
user can already read, or periodic review on native `TurnEnd` during long work.

## 14. Collection development standard

This is the shared development and review contract for CRUD, search, filters, sorting, pagination and list
operations in Aster, the console, Work and Office. **MUST** is a release requirement. **SHOULD** is the default;
an alternative needs a recorded product reason and equivalent acceptance evidence. The constitution and
[development standards](./docs/development-standards.md) continue to own security, layering and persistence.

Page sizes, debounce timing and selection defaults below are Aster product decisions. The external references
support the interaction principles; they do not prescribe these numbers. Publishing this standard does not
certify existing screens. Keep implementation gaps in the owning ROADMAP and retained evidence.

The default contract below is a quick development reference. The linked sections define its exceptions and
acceptance requirements; a feature MUST record its actual contract before implementation.

| Concern | Aster default |
| --- | --- |
| Collection query | [Owner-side search → filters → stable sort → page](#143-one-state-model-per-collection); current authority limits every stage |
| Search and filters | [300ms remote type-ahead with IME support](#144-search); [AND across fields, OR within a field](#145-filters-and-facet-counts) |
| Pagination | [1-based offset for numbered management lists; cursor for growing histories](#147-pagination-and-bounded-reads); sizes 10/20/50/100, default 20 |
| Selection | [Current page only; query/page changes clear selection](#148-row-actions-selection-and-batch-operations); all-matching selection requires an explicit owner contract |
| Writes | [Reviewed revisions, atomic dependency checks and replayable create intents](#149-create-read-edit-and-removal) |
| Feedback | [Distinct loading, empty, no-match, failure, conflict and uncertain-write states](#1410-loading-errors-feedback-and-reconciliation) |
| Release proof | [Applicable acceptance matrix and completed feature/PR template](#1413-definition-of-done-and-review-template) |

Use the following sequence when developing a collection. The linked sections are the source of each rule;
feature notes record the actual owner contract and evidence using the template in section 14.13.

| Development step | Required decision / review entry |
| --- | --- |
| Define the resource | [Owner, scope, lifecycle and independent capabilities](#141-resource-contract-before-controls) |
| Design the view | [Page structure](#142-standard-page-and-list-structure) and [state transitions](#143-one-state-model-per-collection) |
| Define the query | [Search fields](#144-search), [filter semantics](#145-filters-and-facet-counts) and [stable sorting](#146-sorting) |
| Bound the reads | [Pagination mode, totals, limits, continuation and performance](#147-pagination-and-bounded-reads) |
| Define the operations | [Row/batch scope](#148-row-actions-selection-and-batch-operations), [CRUD, validation and concurrency](#149-create-read-edit-and-removal) |
| Define recovery | [Loading, errors and reconciliation](#1410-loading-errors-feedback-and-reconciliation) and [owner authority](#1411-authority-and-implementation-boundaries) |
| Qualify the workflow | [Accessibility and localization](#1412-accessibility-responsive-layout-and-localization), then [acceptance evidence](#1413-definition-of-done-and-review-template) |

### 14.1 Resource contract before controls

Every collection MUST declare its owner, scope, stable ID, permitted operations, lifecycle, editable fields,
reviewed revision, search fields, filter/sort options, pagination mode and dependent resources. Declare this in
typed owner contracts and the feature's design/acceptance notes; do not infer capabilities from a menu label.
Read, create, update, delete, export and lifecycle commands are independent capabilities. Having one capability
does not imply the others. Related-record reads expose only the fields needed for the authorized workflow.

| Resource | Required interaction |
| --- | --- |
| Mutable record | List, detail, create, edit and permitted delete/archive |
| Published asset | Draft editor, explicit publish/withdraw/suspend, versions and dependency-aware removal |
| Running workload | Detail, authorized lifecycle commands, progress/result and retained execution history |
| Configuration | Typed editor, reviewed save, activation result and explicit secret replacement/clear |
| Append-only ledger | Search, filters, bounded history, detail and authorized export; preserve original facts |
| Derived state or reference catalog | Detail, provenance and owner refresh/publication workflow |

An unsupported operation MUST have an honest explanation where users expect it. Do not render a create,
delete or export control until its owner command can finish. A missing service or failed request MUST NOT
appear as an empty successful collection.

### 14.2 Standard page and list structure

Use this order: page title and purpose; toolbar (including the result count); active filters; list/table; pager
and result range. Detail is a route, drawer or expanded row according to its complexity. A nested history owns its own
scope, loading/error state and pager. Sidebar destinations and subnavigation MUST map to real, restorable views.

- Keep one collection primary action, normally `新建供应商`, at the far right of the table toolbar. Other actions
  are quiet. A page-level action MAY stay in the page header when its scope exceeds this collection. The
  initial empty state offers that same authorized action; a filtered empty state offers a reset instead.
- Keep search, filters, refresh and permitted create available when the list is empty or failed. Disable an
  action only when its own prerequisites are unavailable, and expose the reason.
- Place collection controls immediately above the table, aligned with its left and right edges. Do not put
  search, resource-type switching or refresh beside the page title while filters sit in a separate row below.
  The left group contains search, simple filters and advanced filters; the right group contains resource/view
  switching, last-updated status, refresh, other utilities and the primary action last. Keep advanced filters
  behind one named control. At narrow widths, wrap these groups in the same reading order; a long segmented
  switch MAY scroll within its group, without widening the page or splitting individual controls.
- Name records, states, units and timestamps clearly. Use an em dash for an absent value, never for a loading
  or failed value. Numeric columns align right. Truncated information remains available by keyboard and touch.
- The record name opens detail. Row action buttons do not also trigger row navigation or selection. A row
  click is supplementary; it never replaces a real link or button.
- Reuse section 6 visual controls. A list with several comparable fields uses a table; a short object list
  may use hairline rows. Do not add checkbox selection when the resource has no supported batch action.

Use one availability policy for header, detail, row and batch actions:

| Condition | Presentation |
| --- | --- |
| Operation unsupported or not granted | Omit the executable control; explain unsupported workflows where expected without exposing protected data |
| Granted, but prerequisites unmet | Disable the control with a discoverable reason and an authorized route to resolve it when available |
| Conflicting operation pending | Gate the affected controls and show progress; keep unrelated reads and actions usable |
| Current authority unavailable | Disable writes, withdraw protected data as required by section 14.10, and offer authority recovery |

Column visibility/order preferences MAY be offered for dense tables. Scope saved preferences to the account,
tenant and resource; keep record identity and authorized actions discoverable. A hidden column's active sort
or filter remains visible in the query summary. Provide `重置列` separately from query reset. Display preferences
do not change owner authority, query semantics or the declared export fields.

The separation of collection utilities, row actions and selected-row actions follows
[Carbon data table guidance](https://www.carbondesignsystem.com/building-blocks/core/components/data-table/guidelines).

Default management-table layout (labels are illustrative; omit unsupported operations):

```text
Title and purpose
[Search] [Category / status filters]    [Resource types] [共 N 条] [Updated] [Refresh] [Create]
[Active filter chips — only while filters are active]
☐ | Name | Category | … | Row actions
☐ | …
[Rows per page: 20] [1–20 of 123]              [Previous] [1] [2] [3] [Next]
          ╭ Selected count · current page only  [Batch actions] [Clear selection] ╮   ← floats, no layout height
```

The selected-row bar is conditional and floats at the bottom center of the list viewport; it never changes the
meaning of a collection-wide refresh or create action. Use a named region for the collection controls. Do not add `role="toolbar"` unless its required keyboard
interaction is implemented. All fields and controls remain reachable through normal keyboard navigation.

### 14.3 One state model per collection

Treat account, organization, resource and parent ID as collection identity. Treat committed search, filters,
sort and page size as query identity. Include both identities and the page/cursor in the request/cache key.
Never place session tokens or secret values in keys, URLs or persisted view state.

The logical owner query is: current authority and parent scope → search and filters → stable sort → page.
Compute totals from the matching set before paging; apply the declared facet rule in section 14.5 to the same
authorized scope. This is a semantic contract, not a required database execution plan. Filtering or sorting
an already paged remote response cannot implement collection-wide behavior.

| Event | Page/cursor | Expanded detail and selection | Draft |
| --- | --- | --- | --- |
| Search, filter, sort or page-size change | Page 1 / first cursor | Close old detail; clear selection | Apply dirty-form navigation guard |
| Page change | Requested page/cursor | Close old detail; clear selection by default | Apply dirty-form navigation guard |
| Refresh | Preserve query and page; accept server correction | Reconcile exact IDs and revisions | Never silently overwrite dirty fields |
| Return from detail or reopen the view | Restore the scoped query/page | Restore only a still-valid addressed record | Restore only an explicitly supported, safe draft |
| Delete removes the last page | Nearest valid page returned by the owner | Close deleted detail; remove affected selections | Clear the confirmed deleted record |
| Account, tenant or parent changes | Restore only that identity's own view, otherwise defaults | Clear old identity's records and selection | Withdraw the old editor and clear secrets |
| Authority is withdrawn / logout | Clear protected cache and view data | Close protected detail and actions | Discard the protected draft immediately |

On web routes, shareable view state SHOULD use validated URL parameters and work with Back/Forward. Native
views MAY use scoped route memory. Sensitive search text MUST NOT enter a share URL without an appropriate
product decision. Do not store secret-bearing drafts in local storage or a shared query cache.

### 14.4 Search

Search MUST describe what it searches, and execute over the authorized collection at its owner. Searching
only the currently loaded page is permitted only when visibly labelled `搜索当前页`; it is not collection search.

- Update the input immediately. Remote type-ahead SHOULD commit after 300ms of inactivity; Enter submits
  immediately. Respect IME composition. Explicit-submit and costly searches use a visible submit action.
- Clearing search starts at page 1 and keeps filters/sort. Escape clears a non-empty search only when no
  popup or dialog owns that key. `/` may focus search outside editable controls and modal dialogs.
- Use `search` for new collection wire contracts. Declare normalization, searchable fields, maximum length
  and exact/literal/full-text semantics. Treat wildcard characters as data unless an explicit search language
  is supported. Bind values through the owner query builder.
- Commit the query once, cancel obsolete reads where supported, and ignore late results by full request
  identity. Announce completed result counts without announcing each keystroke.
- Search text never guesses entity IDs, names or semantic intent with a regex or word list. Entity pickers
  submit exact IDs from authorized results. Credentials and private fields are excluded from searchable data.

### 14.5 Filters and facet counts

Use segmented controls for a small, mutually exclusive set; a select/combobox for larger sets; an advanced
filter panel for compound criteria. Large option catalogs MUST support bounded owner-side option search.

- Different filter fields combine with AND; multiple selections in one field combine with OR, unless the
  feature explicitly describes another supported rule. The server and UI MUST share the same semantics.
- Show active filters and removable labels. `清除筛选` clears filters and preserves search; `重置全部` clears
  search, filters and sorting to their documented defaults. Both restart pagination.
- Simple filters apply immediately. A complex panel keeps draft choices until Apply; Cancel discards only
  those draft choices. Applying once issues one committed query.
- Counts MUST describe the whole authorized matching collection, with a declared counting rule. The default
  is search plus all other active filter fields, excluding the facet's own field. Never count just page rows.
- A selected value remains visible and removable when its count becomes zero. Unavailable option metadata
  gets an error/retry state. Never silently remove a filter because a refresh did not return its option.
- Dates declare timezone and boundary semantics; numeric filters declare units and validate ranges. Never
  infer a user's timezone or an entity's identity from display formatting.
- Date-only ranges SHOULD include both calendar dates in the declared business timezone. Translate them
  into a half-open instant range `[start of first day, start of day after last)`; do not synthesize
  `23:59:59.999` or assume every local day lasts 24 hours. Reject reversed ranges before committing a query.
- Declare how absent values differ from empty strings, zero, `false` and an unselected filter. Option labels
  are presentation; submitted values use the owner's typed enum or exact ID. Unknown values produce an
  actionable validation error instead of silently broadening the result set.

### 14.6 Sorting

Only expose sorting supported by the owner query. A sortable header is a button and sets `aria-sort` on its
header cell. The cycle is ascending, descending, then documented default order; announce the resulting order.
Every owner ordering MUST end in a stable unique ID tie-breaker, with explicit null placement and comparison
semantics. Numeric/date sorting uses typed values. Local sorting of one remote page MUST NOT imply global order.

### 14.7 Pagination and bounded reads

Choose one pagination mode per endpoint; the UI and response type MUST match it.

| Mode | Use | Contract and control |
| --- | --- | --- |
| Offset | Moderate management lists needing exact totals and numbered navigation | `page`, `limit`; new responses `{ items, total, page, limit }` with effective values, optionally facets; numbered pager |
| Cursor | Growing or frequently changing history, logs and feeds | `cursor`, `limit`; response `{ items, nextCursor? }`; explicit next/load-more or supported previous navigation |

- Aster offset pages are 1-based. Page-size choices are 10, 20, 50 and 100; tables, including embedded histories,
  default to 20. The owner validates bounds and publishes any smaller resource-specific cap.
- New wire contracts use `search`, typed filter fields, `sortBy` and `sortOrder` (`asc` / `desc`). Publish each
  resource's defaults, supported fields and input bounds. Reject invalid values rather than silently dropping
  criteria; do not rename an existing published parameter without a migration.
- Validate page responses at the client boundary: typed rows with unique IDs, non-negative integer totals
  when supplied, valid effective page/limit and bounded item counts. An exact offset response's rows MUST
  agree with its total and effective page. Invalid metadata is a read failure, never an empty result or a
  guessed total.
- Do not offer an unbounded `全部` page size. Keep the result range and page-size control useful for a single
  page; omit redundant page navigation. Zero matches display `0`, not an invalid range such as `1–0`.
- Show the current range, exact total when available, previous/next boundaries and current-page semantics.
  Long page sequences use ellipses with reachable first/last pages. Do not render thousands of page buttons.
- Place the pager directly below the table. On wide layouts, keep page size and range at the left, navigation
  at the right, vertically centered in one footer. At narrow widths, wrap whole groups; previous, page numbers
  and next remain one compact group. Buttons have at least 30px targets, visible focus, a distinct current-page
  style, localized accessible names and actual disabled states at boundaries or during a pending read. Clicking
  the current page does not issue another read. Shared pager styling MUST work with the consuming app's theme
  entry; do not depend on an unimported legacy stylesheet.
- A total declares whether it is exact, estimated or unavailable. Estimates MUST be labelled and cannot drive
  exact last-page navigation or an all-matching selection count. Never substitute the page length for a total.
- Total, facets and rows MUST use the same authorization and query semantics. An exact offset response uses
  one consistent read snapshot. Return the effective valid page after deletion; zero matches return page 1.
- Stable ordering does not freeze data across separate requests. For a growing feed or a stable export,
  use a cursor/snapshot contract that supports that requirement; do not promise an offset walk is gap-free.
- Cursor tokens are opaque continuation data, bound to the query/sort and appropriate scope. They grant no
  authority. New owner contracts MUST reject malformed, mismatched or expired cursors and offer an explicit
  restart; do not silently return a different slice. Historical fallback behavior is migration debt.
- Do not invent totals, page numbers or a previous-page action for a cursor API that cannot provide them.
  A missing `nextCursor` ends the feed. A short or empty page with a continuation token is not the end; retain
  the continuation control without declaring the whole collection empty. Refuse a repeated non-progressing
  cursor. Prefer an explicit load-more control over automatic infinite scrolling in management interfaces.
- Page at the repository, bound related-record lookups and option metadata, and use indexes appropriate to
  filtering/order. Fetch-all-then-slice and virtualization do not satisfy owner-side pagination.
- Declare a feature-specific read latency and payload budget. Verify a representative large collection,
  including counts/facets and expanded details; avoid one related-record request per row. A costly exact count
  needs a different declared contract, rather than an unlabeled estimate or an unbounded client fallback.
- Pager actions gate duplicate reads while pending. After completion, retain a useful focus position and
  politely announce the new range. Errors retain the query and provide Retry.

Named navigation and placement follow [GOV.UK pagination](https://design-system.service.gov.uk/components/pagination/).
Cursor continuation, opacity and per-request authority follow [AIP-158](https://google.aip.dev/158). Aster wire/hook
details are in the [pagination implementation guide](./apps/studio/docs/references/data/data-pagination-guide.md).

### 14.8 Row actions, selection and batch operations

Keep the frequent safe row action visible; put secondary actions in a named overflow menu and destructive
actions last. Actions MUST be reachable on focus and touch, with names such as `编辑供应商 Acme` and
`更多操作：供应商 Acme`. Hover is not the only way to discover or activate them. Detail exposes the same current
authorized operations as the list.

An inline editor or persistent switch is a write operation. It MUST follow the same validation, reviewed
revision, pending, conflict and reconciliation rules as the full editor in section 14.9. A compact control
does not remove those requirements.

- Selection identifies exact records, independently of detail expansion. A header checkbox selects the
  current page by default and shows a mixed state for partial selection. Label its scope and announce the count.
- Put the checkbox column at the far left, before record identity. Give every row checkbox an accessible
  record name and exact ID; use a 24px or larger target. Checking a row MUST NOT open detail or trigger a row
  action. Support Tab and Space with the real checkbox primitive, and visually distinguish selected rows.
  The batch bar shows `已选择 N 项`, `仅当前页`, supported actions and `清空选择`. It floats at the bottom center of
  the list viewport and takes no layout height (section 6, Selection bar); clearing restores focus to a useful
  selection control. Refresh removes selections for records no longer present; returning to an earlier query
  MUST NOT resurrect a cleared selection.
- Declare eligibility for each offered batch action. If selection excludes ineligible rows, explain their
  exclusions and show the eligible selected count. A disabled checkbox needs a discoverable reason; the batch
  command repeats eligibility checks and reports any records that became ineligible before execution.
- Clear selection on query, tenant, resource or parent change and, by default, page change. A feature that
  supports cross-page selection MUST show the persistent count and provide a clear-selection action.
- `选择全部匹配结果` is a separate explicit operation with an exact query scope and count. It requires an owner
  selection/snapshot contract; the client MUST NOT pretend the currently loaded IDs are all matching records.
- Show a batch action bar only when selection exists. Never apply an operation to hidden records without
  indicating that scope. Recheck eligibility and authority for every addressed record at execution.
- A read-only directory MAY support `复制所选 ID` for IDs already disclosed to the current reader. Copy only
  the exact selected IDs, report clipboard success/failure and keep selection on failure. This utility does
  not imply export or write authority. Batch enable/disable, publish, archive or delete appears only when the
  resource owner supports that command and the current reader has its independent grant; never add inert or
  simulated controls to satisfy the checkbox pattern. Destructive batch confirmation names the operation,
  affected count and consequences, and uses the frozen selection described below.
- Prevent overlapping batch and row writes to the same records while an operation is pending. Freeze the
  submitted selection/query and reviewed revisions; a later selection change cannot change that operation.
- Declare whether a batch is atomic or permits partial completion. Report succeeded, failed and skipped
  records separately, preserve failed selections where useful, and retry only unresolved items safely. An
  unknown outcome remains unresolved until reconciled; never classify it as a confirmed failure to enable retry.
- Export declares current page, selected records or all matching records, includes the effective filters,
  and uses the same authorization. Large exports use a tracked job and bounded owner reads. Never label a
  capped sample as a complete export.

### 14.9 Create, read, edit and removal

| Operation | Required behavior |
| --- | --- |
| Create | Named fields, documented defaults and validation; create once; confirm the authoritative record identity and offer currently authorized detail |
| Read/detail | Load the addressed ID in its scope; show lifecycle, revision and dependencies needed for decisions |
| Edit | Start from a reviewed revision; keep entered values on failure; compare-and-set at commit |
| Delete/archive | Identify target and consequences; review dependencies and revision; confirm; reconcile the remaining collection |

Forms MUST have persistent labels, required indicators, associated help/error text, typed enum/ID inputs and
server-owned validation. Validate on an appropriate blur or submit, without interrupting IME entry. After a
failed submit, show a focused summary linking to invalid fields, with the same messages beside those fields.
Never rely on a placeholder or toast as the field's label/error. Preserve the user's safe input on validation,
conflict and transport failure while authority remains valid. This follows
[GOV.UK error summary guidance](https://design-system.service.gov.uk/components/error-summary/) and
[WAI form notifications](https://www.w3.org/WAI/tutorials/forms/notifications/).

Declare each writable field's type, requiredness, default, bounds and absent/null behavior. Local validation
helps users correct input; the owner repeats validation, including uniqueness and cross-field rules. Enforce
uniqueness atomically with persistence. A preflight availability check is advisory and cannot prevent a race.
Omit read-only fields from write DTOs; a disabled input does not enforce immutability.

An editor MUST track dirty state. Cancel, Escape, outside click and navigation protect unsaved input through
one discard decision; returning to the editor keeps it intact. Authority withdrawal closes the protected
editor without an unsaved-input permission prompt. A large/long form SHOULD use a page or drawer with clear
sections; a wizard is justified by real dependent steps, not by a field count alone.

Updates and removals MUST carry the revision reviewed by the user. The owner compares it and all dependency
conditions atomically with the write. A conflict keeps the draft, explains the changed record, and offers fresh
review; never auto-fetch a new revision and retry the old draft. New HTTP interfaces SHOULD use strong ETag /
`If-Match` with 412; existing typed revision contracts may retain their documented 409 behavior. See
[RFC 9110 conditional requests](https://www.rfc-editor.org/rfc/rfc9110.html#name-if-match).

Retriable creates and non-idempotent commands MUST use owner-supported idempotency: bind the intent to actor,
scope, operation and request fingerprint; persist its receipt atomically with the effect; replay the recorded
outcome for the same intent; reject reuse with different input. Publish receipt retention/expiry behavior.
Retries of a completed create MUST NOT resurrect a subsequently deleted record. A disabled submit button alone
is insufficient. If an outcome is uncertain, reconcile that intent before permitting another create.

Recovery MUST survive leaving and reopening the view. If the submitted input cannot safely survive a restart
(for example, it contains a secret), provide an authorized owner receipt/outcome lookup or cancellation
contract. Cancellation must fence a late commit; an absent receipt alone does not prove the write cannot
still finish. Never discard an unresolved intent merely to enable another submission.

Deletion MUST explain affected dependencies and whether recovery is possible. Prefer archive/restore where
the domain supports it. Use PopConfirm for a concise one-record consequence and ConfirmDialog for compound,
high-impact or multi-record consequences. Type-to-confirm is reserved for high-impact irreversible operations.
Preserve audit/execution history. A dependency conflict offers a route to the blocking records, not a forced
delete that weakens the owner boundary.

Submit once while pending, show a localized progress label, and gate repeat confirmation and dismissal during
a short write. A rejected/false confirmation remains open with an error. An optimistic list write (section
14.10) instead closes its dialog on submit and carries the pending state on the row; a rejection then reopens a
form that holds typed input with its draft and the error. Tie completion to account, scope,
target, reviewed revision and dialog generation: a late result cannot close or modify a reopened/replaced
editor. Use a bounded timeout and intent reconciliation for uncertain results. Long operations move to a
tracked job/status view with an explicit way to leave; never trap a user indefinitely in a pending modal.

Aborting a request, closing a view or losing the connection does not prove a write was canceled. Keep its
intent/operation identity for authorized reconciliation; report cancellation only after an owner-confirmed
outcome. HTTP retry behavior follows [RFC 9110 idempotency](https://www.rfc-editor.org/rfc/rfc9110.html#section-9.2.2).

Secret fields MUST be write-only with explicit keep, replace and clear choices. Do not prefill a stored secret
or treat a blank edit as deletion. Read responses show only approved metadata such as `hasKey`; secrets never
enter logs, analytics, URLs, shared cache, persisted drafts or error text. Reveal/clear actions need accessible
names and appropriate review. Clear temporary secret input after success or authority withdrawal.

### 14.10 Loading, errors, feedback and reconciliation

| State | Required presentation and recovery |
| --- | --- |
| Initial loading | Stable skeleton/progress region and accessible busy state; no false zero count |
| Same-query refresh | Preserve the current view, indicate refresh and gate conflicting actions; do not flash an empty state |
| Query change | Show loading for that query; never label previous-query rows as its results |
| Empty collection | Explain the absence and offer the authorized first action |
| No matches | Show applied criteria and a clear reset action; preserve the user's query |
| Read failure / invalid response | Withdraw cached rows presented as authoritative; retain the query and offer Retry |
| Session/authority unavailable | Disable writes and withdraw protected data/actions; reauthenticate or retry the authority read |
| Validation / conflict | Keep the safe draft, show actionable errors or require fresh review |
| Write outcome unknown | Explain that completion is unconfirmed; retain the operation identity and offer authorized reconciliation |
| Confirmed write, refresh failure | Say the write succeeded and the view could not refresh; offer a read retry, never repeat the write |
| Accepted background command | Show job/operation identity and pending/result state; acceptance is not completion |

Errors MUST be localized, specific enough to recover, and free of infrastructure/secret details. Resource
denial and missing share the not-found contract. Invalid sessions lead to reauthentication. Respect documented
rate-limit retry timing. Automatically retry reads only within a bounded policy; retry writes only through
their idempotency contract.

After success, reconcile the addressed detail, all affected filtered pages, totals, facets, sidebar badges and
related projections within the same authority scope. Reset a form only for its own confirmed operation.

List edits and removals are optimistic. The owner stays authoritative: the list shows the write at once,
then confirms, rolls back or reconciles with the owner's answer.

- **Apply at submit.** A removed row plays the 200ms leave motion and then hides; totals and the pager drop
  with it. An edited row shows the submitted values that the list renders, dimmed to 60% with `aria-busy`
  until the owner answers. Owner-derived fields (status, health, revision) keep their owner values.
- **Accept.** An accepted edit flashes the updated tint; an accepted removal stays hidden. Keep the overlay
  until the reconciled owner page includes the write (8s at most), then drop it. A reconcile keeps the
  current page on screen; it never shows a skeleton or an overlay spinner.
- **Reject or unknown.** Every touched row returns in place with the restored tint. A form that holds typed
  input (text, secrets) reopens with its draft and the error; a confirmation or a picker-only edit reports
  the error with the record's name. An unknown outcome keeps its section 14.9 reconciliation.
- **Scope.** Revision, idempotency and authority rules do not change: the write still carries the reviewed
  revision, and the row's other write actions stay disabled until it settles. A shared-list write whose
  owner input is the whole list (for example, a disabled-tool set) locks the sibling controls until it
  settles, so a second write cannot drop the first. Creates are not optimistic, because the record has no
  owner identity until the owner accepts it.

Keep the user's committed query after a create or edit. The new/changed record may belong on another page or
stop matching the filters; never splice it into a result set it does not belong to. Confirm success with a
record identity and, where currently authorized, a detail link. Explain when it is outside the current view.
An explicit `创建并继续` action may reset the form after success while retaining only documented safe defaults;
it starts a new create intent and clears temporary secrets.

### 14.11 Authority and implementation boundaries

Client visibility is a convenience, never authorization. Every new read and command MUST use current owner
authority for the addressed scope. Recheck write authority at commit with revocation, membership, role scope,
revision and dependencies fenced through the transaction or an equivalent proven owner mechanism. Directory
administration does not implicitly grant management of unrelated platform resources.

Use typed QueryBus/CommandBus contracts for remote owners and published cross-context surfaces. Local Aster
DataApi keeps its database-only effect boundary; filesystem, process and network operations use their existing
dedicated transport. A member session stays in the member plane. Do not add a generic operator/admin proxy.
Reject unknown write fields, unknown query parameters and unsupported enum/query values at the boundary.
Document any retained compatibility aliases explicitly; a typo must not silently broaden the query.

New owner HTTP APIs SHOULD use the following mapping; retain existing published paths until an explicit
migration. Local DataApi/IPC uses its established typed equivalent rather than pretending to be a remote owner.

| Intent | HTTP behavior |
| --- | --- |
| List / detail | GET on a plural collection / addressed ID; 200 with the typed representation; no resource mutations |
| Create | POST to the collection; 201 with record identity and Location; require a replayable create intent |
| Edit | PATCH for declared partial fields or PUT for a complete replacement; document absent/null semantics; 200 with new revision or documented 204 |
| Remove | DELETE on the addressed ID with reviewed revision; 204 only after completion |
| Lifecycle / long operation | Explicit typed action; 200 for a completed result or 202 with a durable operation identity/status route |

Publish a consistent typed error envelope with a stable code, safe message, optional field errors and request
identity. Distinguish malformed input (400), documented field-validation errors (400/422), revision/dependency
conflicts (409 or HTTP precondition 412), missing/denied resources (404), invalid sessions (401), rate
limits (429) and temporary service failure. Preserve existing documented status contracts rather than changing
them in one screen. A successful empty list and a failed list have different response types/statuses.

Reuse these implementations; extend the shared layer when a second surface needs the same behavior:

| Concern | Existing implementation |
| --- | --- |
| Shared numbered/cursor pager | [Pagination.tsx](./packages/ui/src/components/console/Pagination.tsx) |
| Aster localization/page-size adapter | [CollectionPagination.tsx](./apps/studio/src/renderer/components/collection/CollectionPagination.tsx) |
| Aster table controls and page-scoped selection | [CollectionToolbar.tsx](./apps/studio/src/renderer/components/collection/CollectionToolbar.tsx), [useCollectionSelection.ts](./apps/studio/src/renderer/components/collection/useCollectionSelection.ts), [CollectionSelectionBar.tsx](./apps/studio/src/renderer/components/collection/CollectionSelectionBar.tsx) |
| Local bounded-list page recovery | [useCollectionPage.ts](./apps/studio/src/renderer/components/collection/useCollectionPage.ts); remote queries page at their owner |
| Optimistic rows and row motion | [optimisticRows.ts](./apps/studio/src/renderer/components/collection/optimisticRows.ts), [rowMotion.ts](./packages/ui/src/lib/rowMotion.ts) |
| Pending/replaced confirmation protection | [use-confirmation-action.ts](./packages/ui/src/lib/use-confirmation-action.ts) |
| Confirmation presentation | [ConfirmDialog.tsx](./packages/ui/src/components/console/ConfirmDialog.tsx), [PopConfirm.tsx](./packages/ui/src/components/console/PopConfirm.tsx) |
| Aster implementation mapping | [Collection UX guide](./apps/studio/docs/references/data/collection-ux-guide.md) |
| Wire types, hooks and codecs | [Pagination guide](./apps/studio/docs/references/data/data-pagination-guide.md), [API guidelines](./apps/studio/docs/references/data/api-design-guidelines.md) |

### 14.12 Accessibility, responsive layout and localization

New collection workflows MUST meet WCAG 2.2 AA and be operable by keyboard. Use semantic tables with named
headers, labels for selection, `aria-sort`, `aria-expanded`/controlled regions, and a named pagination navigation
with `aria-current="page"`. Use an ARIA grid only when its full keyboard interaction is implemented. Row links,
checkboxes and action buttons work inside an ordinary table; their presence alone does not require a grid.
See the [WAI-ARIA table pattern](https://www.w3.org/WAI/ARIA/apg/patterns/table/).

Dialogs have a visible accessible title, modal semantics, inert background, contained Tab/Shift+Tab focus and
a deliberate initial focus. Restore focus to the trigger or a useful surviving control after deletion. Clean
idle dialogs close on Escape; dirty/pending cases follow section 14.9. Destructive confirmation SHOULD initially
focus the safe action. See [WAI-ARIA modal dialog guidance](https://www.w3.org/WAI/ARIA/apg/patterns/dialog-modal/).

Essential text MUST meet 4.5:1 contrast at normal sizes; status and validation do not depend on color alone.
The existing muted palette and inner focus tint are not an accessibility exception. For essential text, this
rule takes precedence over muted colors in component recipes. Choose the existing Body or Ink role where muted
text fails; provide a visible focus outline with sufficient contrast and keep it
unobscured by sticky headers, footers and dialogs. Follow
[WCAG contrast](https://www.w3.org/WAI/WCAG22/Understanding/contrast-minimum.html) and
[focus visibility](https://www.w3.org/WAI/WCAG22/Understanding/focus-visible.html).

Pointer targets MUST be at least 24×24 CSS pixels; use larger targets where touch is expected. Toolbar, form,
filter and pager controls wrap and remain usable at narrow widths and 400% zoom. A genuinely two-dimensional
table may scroll inside its own region; the surrounding page must reflow without losing actions. Verify
320 CSS-pixel content width where the surface supports it, following
[WCAG target size](https://www.w3.org/WAI/WCAG22/Understanding/target-size-minimum.html) and
[reflow](https://www.w3.org/WAI/WCAG22/Understanding/reflow.html).

All labels, progress, errors, confirmations, empty states, ranges and announcements live in `zh-cn.json` and
`en-us.json`. Use locale-aware numbers/dates, explicit business timezone and pluralization. Test long translations
and reduced motion. Do not build user-visible sentences by concatenating translated fragments.

### 14.13 Definition of done and review template

A collection is complete only when applicable rows below have retained evidence. Record an inapplicable row
with its resource reason; a disabled placeholder or a passing component snapshot does not satisfy it.

| Gate | Required proof |
| --- | --- |
| Resource coverage | Owner/lifecycle/capabilities declared; each sidebar destination and permitted action reaches its real workflow; consistent availability reasons and scoped column preferences when offered |
| CRUD persistence | Authorized create, reopen, edit, reviewed remove/archive and dependency refusal; no fabricated success |
| Query behavior | Search/filter combinations, date boundaries, absent/unknown values, supported sorting, exact full counts and more than two pages of records |
| Paging boundaries | First/last/empty/single page, page-size change, compact non-wrapping navigation, last-page deletion recovery, invalid or inconsistent response metadata; short/empty cursor pages with continuation, end and restart as applicable |
| Read bounds and performance | Representative large collection within its declared latency/payload budget; bounded related reads and no per-row request cascade |
| Forms and asynchronous ownership | Linked field errors, owner validation and uniqueness races, preserved draft, dirty cancel/navigation, duplicate submit, late response, reopened dialog, disconnected write and uncertain outcome |
| Concurrency and authority | Stale revision, idempotent replay/input mismatch and restart recovery, revoked grant, inactive member, other tenant and protected-cache withdrawal |
| List and batch operations | Left checkboxes, keyboard selection, mixed check-all, clear/reset/refresh reconciliation and independent detail expansion; correct selection scope and ineligible rows, every addressed record authorized, partial/unknown outcomes and retry/export scope when supported |
| Accessibility and layout | Actual keyboard/focus flow, screen-reader names/status, contrast, narrow window/zoom and both locales |
| Motion and emotional feedback | Real event-to-state mapping, interruption/reversal, stable controls and reading position, hidden/settled work stopped, and equivalent reduced-motion content and actions |
| Owner storage | Remote owner repository/authority campaigns run unchanged on memory and live PostgreSQL; local collections qualify their actual persistent adapter |
| Delivery | Appropriate checks, actual HTTP/native/GUI persistence for the changed chain, evidence directory and ROADMAP gate update |

Use this template in feature/PR notes. Each entry MUST name the actual contract or evidence, rather than say
only that the feature follows this document.

```text
Resource / owner / scope / lifecycle:
Allowed operations / independent grants:
Action eligibility / unavailable reasons / optional column preferences:
Fields / validation / secrets / reviewed revision / create intent / restart recovery:
Search fields / filter rules / date boundaries / absent values / facet counts / supported sort:
Pagination mode / wire defaults / bounds / response validation / stable order / recovery:
Selection scope / eligibility / batch outcome / removal dependencies:
Affected detail / pages / totals / facets / badges / success outside current view:
Loading / empty / errors / pending / conflicts / draft recovery:
Keyboard / focus / responsive layout / locales:
Motion triggers / state owner / interruption / reduced motion / result and recovery:
Checks / retained evidence / ROADMAP gate / remaining gaps:
```

Use the applicable acceptance rows as the implementation checklist, then qualify the runnable delivery chain.
A document update or scoped test pass does not waive full-lane acceptance.

## 15. Using this document

Aster, the console, Work, Office, and the official site read this file. A control that already exists in
section 6 is reused, not redrawn. Page sidebars share 260px (`PAGE_SIDEBAR_DEFAULT_WIDTH`): the work column
and the settings nav. A layout number that belongs to one page (176px plugin categories, 920px work content
column, 760px composer) stays on that page. Promote a new pattern into section 6 only after a second surface
uses it.

Console tables, trees, pagination, and filter chips in `packages/ui` use these roles: ink `#171717`, field
`#f5f5f5`, selected row `#f2f3f5`, row type 13px, control radius 8px, dialog radius 12px. They do not add a
second palette.

Before shipping chrome, check:

- selected state is `#f2f3f5` and `#171717`, at 13px, on a 36px row;
- search is the 30px `#f5f5f5` field, and segments are the white pill on a `#f5f5f5` track;
- page titles are 24px bold, except the work greeting at 28px;
- keyboard focus is clearly visible; the inset `black/10` tint has an Ink outline where needed;
- settings still floats inside the content card and leaves the rail selection in place;
- feature-card colors and the tenant brand are not used for rows, segments, or the rail;
- a record list uses the hairline row, a data table uses the 48px member row, and a one-line confirm uses PopConfirm;
- collection behavior and retained acceptance evidence satisfy section 14.
- motion follows the shared recipes: real state, stable targets, interruptible movement, quiet completion,
  useful failure recovery, reduced motion and no animation work in an inactive or settled view.
