# 演示文稿

Package `a3s/presenton@1.0.0`. The member surface is the homepage composer mode `slides`, titled 演示文稿. The package does not declare a mini app and does not open a window.

| Surface | Id | Role |
| --- | --- | --- |
| Skill | `presenton` | Generates one step of a deck: outline, layout structure, or one slide's content. |
| Agent | `presenton` | Edge package harness, entry `agent/run.mjs`, 500 cpu millis, 256 MiB. |
| Composer mode | `slides` | Homepage work mode. Controls are page count, language, tone, verbosity, title slide, and instructions. |

Nothing runs on the host outside AsterDream. There is no FastAPI or Next.js server, no port, no Python, and no Docker.

The package runs [Presenton](https://github.com/presenton/presenton) (Apache-2.0) in the same form as `a3s/g1-arm`: the view is Presenton's own frontend, the package agent calls the model through the a3s gateway turn by turn, and everything renders locally under the AsterDream view policy.

## Workbench

AsterDream does not mount these files and does not open them in a window. The bundled `ui/workbench` document remains in the package archive.

The view is Presenton's Next.js frontend (upload, outline, and presentation pages), built into one document by `tools/build-view.mjs`, with Chinese copy:

1. **输入**: topic, slide count (auto or 1–20), language, and the advanced settings (tone, verbosity, title slide, instructions).
2. **大纲与模板**: the outline streams in and can be edited and reordered; the member picks one of Presenton's General, Modern, Standard, and Swift templates.
3. **生成**: slides stream in one by one in Presenton's editor.
4. **编辑与演示**: Presenton's slide editor (text, blocks, tables, charts, infographics, icons, layout, undo and redo, slide reordering, new slides from blank or a template layout) and present mode.

Upstream's pages call their FastAPI server with `fetch` and `EventSource`. In the view both are answered in the page by `view/backend`, in upstream's response shapes: generation endpoints run agent turns; reads and writes go to the one presentation the view document holds, saved with `aix.view.store` (64 KiB, gzip). An edited slide is stored as a patch against what its layout and content produce, so decks stay small.

### What the build changes in upstream's pages

Upstream sources are read from a checkout and edited on the way into the bundle; nothing is copied into this repository. `tools/view-patches.mjs` holds exact-string behaviour edits, each of which must match or the build fails:

- Removed: the AI chat and outline assistant, export, Smart mode, community designs, document attachments, the table-of-contents option, the user guide, and the GitHub/Discord links. These need a network, a host service, or features the agent does not provide.
- The new-slide prompt box is hidden (it hands its prompt to the chat).
- `next/*` and analytics are replaced by `view/` (in-memory router, links, images; Mixpanel is a no-op).
- Slide limit 20; language default Chinese (Simplified).

`tools/view-zh.mjs` and `tools/view-zh/*.mjs` hold the Chinese copy. `tools/view-translate.mjs` applies it through the TypeScript syntax tree, so only string literals, JSX text, and template literals can change. Every pair must match, and a translated string literal must be translated in every bundled file that contains it, because code can compare copy across files.

## Generation

Each turn is one `aix.agent.invoke` with `parameters.step`:

| Step | Model requests | Output |
| --- | --- | --- |
| `outline` | 1–3 | `outline`: one Markdown string per slide. |
| `structure` | 1–3 | `layouts`: one layout id per slide; `source` is `rule` when the model was unavailable and layouts were assigned in template order. |
| `slide` | 1–3 | `content`: the layout's content object with `__speaker_note__`. |

`agent/deck.mjs` builds upstream's prompts (`generate_presentation_outlines.py`, `generate_presentation_structure.py`, `generate_slide_content.py`). Each request is a chat body (`stream: false`, `response_format: json_object`, `reasoning_effort: none`) for the default model `bailian/deepseek-v4.1-flash`, or the safe model id given in `parameters.model`. The agent validates the reply against the step's JSON schema and, like upstream's `generate_structured_with_schema_retries`, sends the validation errors back for a corrected reply. A turn stops after three requests or 45 s of retries, inside the host's 16-request and 90 s limits. When fixes run out, the last parsed reply is used, as upstream does.

Adaptations to the box:

- Languages are upstream's 90 labels (`agent/languages.mjs`; the build fails when upstream's list changes). Template text limits count Latin characters, so for the four Chinese, Japanese, and Korean labels the agent halves each `minLength` and `maxLength`.
- An automatic slide count lets the model choose from 1 to 20 slides.
- Icon fields are bound to the 1,512 vendored Phosphor names. The prompt lists them, and a name outside the list fails validation. The view resolves names to icons locally.
- Image fields keep the template placeholder image.

## Not in this package

- **PPTX / PDF export.** The view has no downloads, no network, and no file access. Export would need a AsterDream host surface (for example, a save-file bridge). That is a change to the view's least-privilege boundary and is not made here.
- **Images.** No image generation, stock search, or upload: the box has no network, the gateway has no image route for packages, and the 64 KiB view state cannot hold images. The image picker says so.
- **AI chat, web search, document input, Smart mode, custom templates.**
- **Upstream's other 12 templates.** Each is 0.7–2.3 MB of layout data and 3–13 MB of images; the four kept templates fit the package.

<a id="studio-view-policy"></a>

## AsterDream view policy

The view document is sandboxed (`allow-scripts`, opaque origin) with `connect-src 'none'`, `img-src 'self' data:`, and no `'unsafe-eval'`. The page fetches nothing: every file it loads is a view file, and every file is under the API's 8 MiB limit.

## Building

```sh
git clone https://github.com/presenton/presenton /tmp/presenton
(cd /tmp/presenton/servers/nextjs && npm ci --ignore-scripts)
npm i --prefix /tmp/presenton-deps @fontsource/poppins@5 @fontsource/montserrat@5 \
  @fontsource/playfair-display@5 @fontsource/albert-sans@5 @fontsource/inter@5
node tools/vendor-presenton.mjs /tmp/presenton /tmp/presenton-deps/node_modules
node tools/build-view.mjs /tmp/presenton            # add --check to verify edits without writing
```

`tools/vendor-presenton.mjs` writes the four templates and the Phosphor icons as modules under `view/vendor/`, their images under `ui/workbench/vendor/`, the template fonts, and `agent/templates.mjs` and `agent/icons.mjs`; it computes the layout schemas with upstream's Python (`uv run` with pydantic, jsonschema, and openai; `tools/template_schemas.py`). `tools/build-view.mjs` bundles the pages with esbuild and Tailwind, copies the public files the bundle names, converts upstream's UI fonts to woff2, and writes `THIRD_PARTY_NOTICES.md`. `tools/render-cover.mjs` renders `brand/cover.jpg`.

## Tests

```sh
cd apps/use-registry/packages/presenton
node --test tests/*.test.mjs
node tests/browser.proof.mjs [out-dir]
```

The unit tests cover the package shape and the view's AsterDream policy, the agent's prompts, schemas, validation and feedback loop, the edge protocol, and the view backend (hydration, icons, slide patches). The browser proof runs the view under AsterDream's exact document policy in Playwright and, for every template, goes through upstream's pages with the real agent step code and a fake model that answers each schema: input, outline, template, generation, a text edit, a title change, present mode, and a reload that restores the deck without new turns. It checks for console errors, broken images, and requests outside the package. `tests/`, `tools/`, `view/`, and `brand/` are package source only. The API publishes an explicit file list (`apps/api/src/contexts/plugin/infrastructure/presenton_package.rs`) that excludes them; its test fails when this directory and the list differ.

## Supply

This directory is package source. The admission is `a3s/presenton` in `admissions.acl`. Operators publish it with `a3s-use-registry-tools assemble`. The API seeds the same files as the catalog plugin `a3s-presenton` (`workbench_seed.rs` builds the archive).

Third-party notices: `ui/workbench/THIRD_PARTY_NOTICES.md`.
