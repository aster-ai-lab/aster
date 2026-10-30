# Retained companion source integration

The retained `2c418923b` source is integrated without reversing the subsequently
qualified release pipelines. Desktop publication, the Core namespace, original
governance checks, Windows coverage, source-bound qualification and installed
archive smoke gates are preserved.

The three incoming Core goal-feedback production files are already represented
by the current source. ACP already uses the image-aware prompt-content API and
one shared successful-tool receipt decoder. Those existing implementations are
retained instead of restoring the older text-only API or local decoder copies.
The only ACP Rust delta is a diagnostic message on an unchanged test assertion.

An automatic merge also added a second Python test class with the same name as
the existing release contract class. It would replace the current tests during
discovery. The duplicate was removed; the qualified tests remain byte-identical
to the preceding merge.

## Record fidelity and verification

Both historical Core 9.1.6 qualification records remain intact, including their
different test counts, source identities and binary hashes. Separate headings
state that these records do not qualify the current merged source. The current
scope text reflects actual ACP image handling and the product tag namespace.

The release workflows and both release test files match the preceding qualified
merge exactly: **25 local tests**, six parsed workflow/job graphs and **86 Bash
steps**. There is no production Core/ACP code delta to requalify in this merge.
The historical records were re-opened and compared with both original conflict
inputs, rather than checking only their keywords.

## Explicitly not claimed

No new full Core/ACP run, fresh release build, hosted platform qualification,
native application acceptance or publication is claimed. Original historical
test counts and hashes remain separate from current delivery evidence.
