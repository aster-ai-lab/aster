# Homepage and complete documentation restoration — 2026-10-30

The first recovery restored the accepted galaxy but checked only the older
two-product documentation catalog. That was insufficient: the fetched main
revision `e2beb284d3fe99a1fd8b4ad78f13a9988187c9f9` contains 16 products and
134 source pages. The complete source catalog and its existing Astro build
pipeline have now been restored together.

## Restored behavior

- Galaxy code and its original regression suite are byte-identical to
  `1fac3c2e368419e4c291395f49dc28aabc245b3e`: violet logarithmic arms,
  layered travelling stars and the continuous gather/spread cycle.
  The associated historical glass material was restored in the existing
  download-theme CSS block. Other homepage CSS and JavaScript were preserved.
- Documentation now has **16 products, 17 versions and 134 source pages**.
  Products retain the fetched catalog's names and ordering: Code, ACP, Boot,
  ORM, Search, Effect, Use, Memory, Event, Lane, Sandbox, Office, Gateway,
  Power, Vec and Apofasi.
- The final site contains **242 documentation URLs**, including default-version
  aliases. All **130 previous documentation URLs** remain present. Every
  registered MDX source exists; the restored catalog equals the fetched
  authoritative catalog.
- Recovered the corresponding diagrams, source walks, search, product/version
  navigation, chapter generators, Astro routes, dependency manifest and
  publication workflow. Generated HTML, navigation and diagram assets come
  from the existing build, with `dist/` as the delivery output. Legacy public
  generated files were retired as required by that source migration.
- Local ACP coverage/session additions, RSI examples and the Meta Harness
  explorer were merged rather than replaced by older prose. The graph's local
  runtime and styles were moved into the restored source-owned modules.
  Its regression inputs now read the actual Astro output; graph-dialog
  selection is scoped because the restored code viewer also has a dialog.
- ACP's introductory method counts now match the existing working source:
  21 handler variants and 27 method names/families. No ACP runtime code was
  changed by this recovery.

Before applying the documentation recovery, all existing website files and
the original Git index were snapshotted under the task's ignored target
directory. The approved `cosmic.js`, `home.css`, `home.js` and galaxy tests
remained byte-identical throughout documentation recovery. Existing brand
assets and the original index were also verified unchanged.

## Checks

From the repository root:

```sh
npm --prefix apps/web ci --workspaces=false --ignore-scripts --no-audit --no-fund
JS_DOM_MODULE="$PWD/packages/ui/node_modules/.pnpm/jsdom@26.1.0/node_modules/jsdom" \
  npm --prefix apps/web test
python3 apps/web/scripts/stage_site.py target/web-docs-restoration-20261030/site
```

- **149 tests passed:** 20 Python publication/resource checks and 129
  JavaScript, DOM, Canvas and documentation checks. The same canonical test
  command used by the recovered workflow built the site, checked regeneration
  stability, validated the final public boundary and ran the complete suites.
  [Full test log](site-test.log.gz).
- **246 HTML pages** built: 242 documentation routes, homepage, download,
  terms and privacy. The final publication contract passed for **294 files**.
- The actual staged output was reopened: its complete file set equals the
  owner's combined site/home/document contracts, and each file matches `dist/`
  byte for byte. [Staging receipt](staging.log.gz).
- HTTP GET of all **242** documentation routes on the actual local server
  returned 200 and bytes identical to their built HTML, each with its heading.
  This was checked separately from the fixture tests.
- The original galaxy regressions caught the old implementation: **8 passed,
  5 failed** before restoration; **13 passed, 0 failed** afterward.
  [Before](cosmos-before-regressions.log.gz),
  [after](cosmos-after-regressions.log.gz). These 13 are included in the
  final 129 JavaScript cases, not added to the total twice.
- The imported lock initially refused a clean install because two transitive
  entries were missing. It was regenerated with the existing npm workflow
  (`npm install --package-lock-only`, same isolation flags), then a fresh
  `npm ci` passed. Direct dependency versions remained unchanged.
  [Initial refusal](install.log.gz), [regeneration](lock-regeneration.log.gz),
  [successful clean install](install-final.log.gz).
- Scoped `git diff --check` passed. Machine counts and preservation checks
  are in [verification.json](verification.json).

Native Quark inspection confirmed the restored violet galaxy and glass cards,
the recovered Office article and its diagram, and the complete 16-product
documentation homepage. A hard refresh removed the browser's cached two-product
preview. The A3S Test adapter refused version admission (installed 0.3.2,
required 0.4.x); native CUA was used for visual inspection. No passing A3S
browser run or saved screenshot artifact is claimed.

## Explicitly not claimed

- No commit, push or production deployment of this website recovery.
- No replacement of unrelated repository changes or staged work.
- No new Rust build, runtime acceptance campaign or measured browser
  performance benchmark. The accepted galaxy design was recovered, not retuned.
