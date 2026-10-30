# ACL, Flow and documentation ordering — 2026-10-30

Added ACL 0.3.0 and Flow 2.0.0 using the existing product documentation
structure: overview, integration, operations, a Pele architecture diagram,
version metadata, fixed source references and chapter navigation.

## Content and source fidelity

- Six MDX pages, two version-owned component modules and two Mermaid sources.
  All source citations use the existing snapshot
  `8729d7c84a2d887c61dc206ba47bf8a63425e768`; build-time `SourceRef`
  resolution verifies exact matches and produces real source line links.
- ACL documents typed syntax, bounded parsing, closed Schema admission,
  diagnostics and truncation, canonical bytes/digests and host-owned activation.
  Function calls remain AST data. Schema-aware canonicalization is explicitly
  separate from Schema admission.
- Flow documents deterministic decisions, durable history, stable step IDs,
  waits, Store selection, Worker compatibility, cancellation and unknown
  Activity outcomes. External effects retain the at-least-once boundary;
  the in-memory example makes no restart-durability claim.
- Both complete Rust examples were copied byte-for-byte from their MDX source
  into Markdown and executed by `rustdoc --test` with the actual workspace
  libraries. The ACL example parses, validates and generates a digest; the
  Flow example asserts the completed greeting output.
- The original sixteen product records, their versions and chapter ordering
  remain intact. The user subsequently requested product menu ordering, so
  product records were reordered in the single catalog rather than duplicated
  into a second menu definition.

The shared order is Code, ACP, ACL, Flow, Effect, Lane, Event, Memory, Search,
Vec, Apofasi, Use, Office, Sandbox, Boot, ORM, Gateway, Power. The left menu,
product switcher and documentation homepage cards use this same order.

## Validation

Commands ran from the repository root with existing dependencies:

```sh
JS_DOM_MODULE="$PWD/packages/ui/node_modules/.pnpm/jsdom@26.1.0/node_modules/jsdom" \
  npm --prefix apps/web test
cargo build --locked -p a3s-acl -p a3s-flow --no-default-features --message-format=json
npm --prefix apps/web run check
JS_DOM_MODULE="$PWD/packages/ui/node_modules/.pnpm/jsdom@26.1.0/node_modules/jsdom" \
  node --test apps/web/tests/acl-flow-docs.test.mjs
python3 apps/web/scripts/stage_site.py target/acl-flow-docs-20261030/site-ordered
```

| Check | Result | Evidence |
| --- | --- | --- |
| Complete site suite before the later ordering request | 20 Python + 132 JavaScript cases passed | [Full suite](site-test.log.gz) |
| Final ordered site build, regeneration and publication boundary | Passed; 258 HTML pages and 306 allowed files | [Final check](ordered-site-check.log.gz) |
| Final documentation/order cases | 4 passed | [Focused tests](ordered-docs-tests.log.gz) |
| Actual ACL and Flow library build | Passed, locked pinned workspace toolchain | [Build](rust-build.log.gz) |
| ACL MDX Rust example | 1 passed, 0 ignored | [ACL example](acl-doctest.log.gz) |
| Flow MDX Rust example | 1 passed, 0 ignored | [Flow example](flow-doctest.log.gz) |
| Final deployment staging | 306 files, reopened and byte-identical to `dist/` | [Staging](staging-ordered.json.gz) |

The focused tests check real rendered headings, canonical/version aliases,
chapter selection, pinned source links, exact search records, the architecture
SVGs and identical ordering across the three navigation surfaces.

The final site contains **18 products, 140 source pages and 254 documentation
URLs**. All previous **242 documentation URLs** remain. HTTP GET of every
final documentation URL returned 200 and bytes identical to its built file.
The final stage contains exactly the combined site/home/document allowlist.
Machine counts are retained in [verification.json](verification.json).

Native Quark inspection confirmed ACL's overview, architecture diagram and
three-page navigation, then the final eighteen-product sidebar order.
The subsequent native Flow screenshot request timed out and reset the CUA
kernel; no successful Flow visual inspection is claimed. Flow's generated
pages, diagram, source references, routes, search records and Rust example
passed the recorded checks.

The accepted homepage HTML/CSS/JavaScript hashes and the original Git index
were rechecked unchanged. Scoped `git diff --check` passed. Generated site
files were produced by the existing Astro/MDX pipeline.

## Explicitly not claimed

- No commit, push or production deployment.
- No ACL or Flow library implementation changes, new SDKs or dependencies.
- No cross-process durable execution, SQL adapter campaign, external service
  calls or full library acceptance suite. Runtime execution here is limited
  to the two documented examples.
