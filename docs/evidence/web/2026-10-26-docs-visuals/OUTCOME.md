# Static code highlighting and Pele diagrams

Date: 2026-10-26. This records the visual-infrastructure phase, before the tutorial-series expansion. At this point the site had 13 MDX pages and 34 published files.

## Changes

- Shiki 4.5.0 highlights fenced code at build time. The GitHub Light High Contrast theme replaced the initial GitHub Light candidate after a generated orange token failed the 4.5:1 contrast assertion. All final emitted code colors pass against their background.
- ACL uses a small TextMate grammar based on `crates/acl/src/lexer.rs`. It colors literal values, comments and assignment syntax; it does not parse or authorize configuration.
- Code retains original characters, indentation and trailing newlines. Blocks have language labels, accessible names, keyboard focus and local horizontal overflow. Browser JavaScript is not required.
- Pele 1.10.0 renders the architecture, task sequence and Meta Harness assembly diagrams from authored `.mmd` files. White backgrounds, graphite text, gray groups and Ubuntu match the document theme. Natural SVG dimensions prevent unwanted enlargement or unreadable shrinking.
- Shiki token styles and Pele SVG styles are extracted into one local `docs/generated.css`. Inline styles, scripts and remote assets remain disallowed. Publication validates the new stylesheet too. Pele links and images are disabled, and diagrams have stable prefixed accessibility IDs.

## Validation

| Check | Result |
| --- | --- |
| `npm --prefix apps/web run build` / `run check` | 13 pages plus generated CSS; deterministic and synchronized |
| Full website Node suite | 51 passed; [log](node-tests.log) |
| `python3 -m unittest discover -s apps/web/tests -p 'test_*.py' -q` | 19 passed; [log](python-tests.log) |
| `python3 apps/web/scripts/check_site.py` | 34 public files, 13 docs pages, 6 download links, 4 homepage tabs |
| `python3 apps/web/scripts/stage_site.py target/aster-site-docs-visuals-20261026` | Exact 34-file staging; [receipt](stage-receipt.json) |
| `a3s-test check` / `run apps/web/tests/docs.acl --browser-driver standalone --json` | 3 scenarios, 66 steps passed; [report](browser-report.json) |

New assertions check every fenced block against the MDX source, focus and language metadata, emitted palette contrast, static Pele SVG accessibility and style extraction, stale generated CSS, and refusal of remote stylesheet imports or URLs. Interactive review finished successfully: [result](interactive-review.json).

Screenshots: [architecture](screenshots/pele-architecture-desktop-final.png), [sequence](screenshots/pele-workflow-desktop.png), [desktop code](screenshots/shiki-rust-desktop.png), [mobile code](screenshots/shiki-rust-mobile.png).

Primary implementation references: [Shiki Rehype](https://shiki.style/packages/rehype), [style extraction](https://shiki.style/packages/transformers#transformerstyletoclass), [Pele API](https://pele.run/api), [Pele theming](https://pele.run/theming), [WCAG contrast minimum](https://www.w3.org/TR/WCAG22/#contrast-minimum).

## Explicitly not claimed

No deployment or Rust runtime change. Code contrast assertions cover the actual emitted palette, not a complete WCAG audit. The standalone browser suite ran on Chromium; Safari, Firefox and assistive-technology behavior were not qualified. Diagram rendering uses Pele's server-side text measurer; no claim of font-metric identity across operating systems. The interactive horizontal-wheel attempt is not proof of a measured scroll displacement; the visible scrollbar, CSS overflow and focusability were checked.
