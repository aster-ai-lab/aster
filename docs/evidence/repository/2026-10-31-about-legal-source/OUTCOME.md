# Retained about/legal source integration

Merge dirty-source checkpoint `d361a576f6e214c90695d2e77ba2d8c2aaefa256`.
All 20 conflicted files and 57 automatic-merge changes were reviewed before
resolution. The source tree after resolution equals the pre-merge HEAD.

The current implementation already carries the package-context API. Retain
the distinct `PackageKnowledgeBindingV1` and `PackageKnowledgeCitationV1`
public types, package citation/binding metadata keys and current test names.
The incoming generic Knowledge names would undo the later authority
separation. Matching Core callers, exports, tests and documentation remain
unchanged together.

The automatic changes largely reproduce the frozen fixture and hash variants
reviewed during PR 208 integration. Fifty-two files match that already-reviewed
incoming variant exactly. The five other changes were inspected directly:
three alter fixture digests or package identifiers, and two accidentally
replace the ordinary phrase "cognitive load" with "package load" in the
Dashboard specification. Retain the original coherent fixtures, test inputs,
registry signatures and specification meaning. The mock-registry producer
retains the original fixture digest and byte count as well.

## Verification actually completed

- All 77 reviewed paths are restored to their pre-merge bytes; individual
  SHA-256 values were recorded during resolution.
- `git diff --cached --name-only` is empty before this evidence addition.
- `git ls-files -u` is empty. Incoming source remains reachable through the
  actual merge ancestry.

## Explicitly not claimed

No runtime behavior changes, fresh Rust compilation or new test pass is
claimed by this source-equivalent merge. No workspace cleanup, push, DMG,
notarization or release is established here.
