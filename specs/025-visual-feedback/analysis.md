# Pre-implementation analysis

## Post-implementation correction

The pre-implementation inventory below is not proof of parity. A renewed source/doc audit after the user's coverage question found omitted subfeatures within the broad pin, long-capture, OCR, canvas, import and candidate groups. The original frozen guide's hash matches its receipt, so these gaps predate the implementation. T017 and the [source comparison](../../docs/evidence/desktop/2026-10-29-visual-feedback/polish/README.md#parity-correction-after-the-users-coverage-question) track them separately from T016's missing native acceptance. The current outcome is partial implementation and partial native qualification.

| Check | Finding | Resolution |
| --- | --- | --- |
| Requirements coverage | FR-001–013 mapped to T001–008; user follow-ups FR-014/015 map to T010/011 | Native library/chrome passed; full original acceptance remains tracked in T007/T012 |
| Ambiguity | “Rust equivalent” could imply new GUI | Existing GUI retained, native processing Rust; stated to user |
| Schema fidelity | Upstream literal `EditHere` provenance | Aster names itself honestly; no false exact-schema claim |
| Privacy | Original plus feedback could leak redacted pixels | Sanitize all exported sources before hashing/attachment |
| Binary response | Native macOS project export returned number arrays, which File converted to decimal text | Reuse the existing decoder for screenshot/project/archive; 61 renderer tests pass; actual corrected save/reopen still requires native evidence |
| Coordinates | Existing editor uses CSS pixels | New document boundary uses image pixels; explicit renderer transform |
| Session binding | Shared global queue could target wrong composer | Completion uses initiating callback; cancellation/timeout submits nothing |
| License/source | Upstream is noncommercial source-available | Functional docs/schema only, independent implementation and Aster assets |
| Platform evidence | macOS hardware available | No Windows/Linux native pass inferred |
| Scope | Full requested behavior is substantial | All capability rows remain tracked; no silently deferred parity |

**Verdict**: Ready to implement. No NON-NEGOTIABLE violation or unresolved clarification marker. This is a plan verdict, not a test pass.
