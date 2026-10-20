# Synthetic feature fixtures

These cases supplement the reference workbook; they are not additional user
documents or original benchmark tasks. `studio-tables` maps exactly to
`../tests/features.xlsx`, `Sheet1!C2:C5` (F01-F04). Its instruction preserves
those four prompts and adds explicit sample files, deduplication rules and
output schemas so results can be checked without guessing user intent.

All people, projects, addresses, invoices and identifiers are synthetic.
`example.test` addresses are test data. The invoices say they are not valid
for payment. `build_table_fixtures.py` authors three XLSX files and a three-page
text PDF. Artifact Tool and its dependency runtime are unavailable on this host,
so the builder uses the permitted local-library fallback in a private temporary
environment: openpyxl 3.1.5, reportlab 5.0.1 and pypdf 6.20.1. Inputs are reopened
and compared against explicit source values; every PDF page was rendered with
Poppler and visually checked.

The live driver copies only `inputs/` into the model workspace and records each
file's SHA-256. Input changes fail acceptance. Graders stay out of the agent VM;
they enter a different fresh VM only after the complete agent VM is destroyed.
`test_outputs.py` contains synthetic expected outcomes, not an oracle answer
given to the model. `test.sh` is a byte-identical copy of the reference's pinned
Python 3.13/pytest 8.4.1/pytest-json-ctrf 0.3.5 launcher.

Run `run.py --task studio-tables --source <this-directory>` with the same explicit
binary arguments as the terminal lane. The optional `--verifier-image` accepts
the separately prepared `a3s-studio-reference:verifier-ubuntu24.04` tool cache;
it changes neither `test.sh` nor its versions. This is a text-PDF/data-transform
leg. Passing it does not certify scanned-PDF OCR, AsterDream attachment selection,
spreadsheet rendering, the native GUI, or the entire F01-F04 acceptance matrix.

`studio-content` covers C10:C17 (F09-F16) with eight independent Markdown
deliverables. Automated checks verify source facts; a retained PM review must
also check prose, unsupported completion claims and schedule arithmetic. A
heading such as “第一条” may sit outside its exact quoted clause body. The grader
does not decide a booking claim's meaning with a phrase list. Its first revision
incorrectly rejected these valid formatting/paraphrase choices; both original
failures and the revised checker hashes remain retained. Never use an automated
fact-check pass alone to accept these content cases.

`studio-proofreading` covers W01-W04 with explicit prompt profiles, local sample
files, ambiguity, selected checks, a local whitelist, exact evidence, mixed
dispatch, relevant/irrelevant security material and a correct-text negative
case. It does not exercise the installed Word review package or prove its UI.

`studio-web` covers C6:C9 (F05-F08). Its four grader tests check HTML structure,
local assets, control presence and document completeness. They do not execute
JavaScript or browser clicks. A real browser must still verify timer behavior,
CTA, form confirmation, prototype navigation, filter state and narrow layouts.
