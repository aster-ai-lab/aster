---
name: paperloom
description: Read one explicitly selected paper inside AsterDream's native paper workspace.
---

Use the `paperloom` host workflow with one explicitly selected workspace PDF.
Reuse AsterDream file selection and saving; do not create another library or ask the
user to import the same file again. Original reading does not start processing.
Bind jobs, answers and saved artifacts to the workspace file and source digest.
The host reads the PDF text layer in-process. It does not call an external processing service.
Do not ask for a service address, an API key, or another OCR provider.
The job fails only when every page's trimmed text is empty. A mixed document succeeds, and markdown omits the empty pages. Do not substitute model output for those pages.
Source reading does not authorize PDF edits, Zotero writes or Obsidian exports.
