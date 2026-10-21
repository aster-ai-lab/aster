# 论文精读

The `a3s/paperloom` AIX package extends AsterDream's installed `@` agents. Select
one PDF from the current workspace, then invoke `@论文精读` to read or ask
about that source. With no selection, reuse AsterDream's workspace file picker or
provide the exact workspace path. No separate library, collection manager,
upload workflow or applet window is declared.

The workspace follows `DESIGN.md` and uses AsterDream's existing file and save
controls. Original PDF bytes come from the workspace snapshot. Derived output
is saved back to the same workspace without overwriting its source.

The host processor is in-process Rust. It does not start an external processing
service, and it does not store a service address or API key. Pages that
contain a text layer become Markdown. The job fails only when every page's
trimmed text is empty. A mixed document succeeds: a textless page stays in the
page count and progress, and markdown omits it. This package does not
rasterize pages and does not call Gateway OCR. Translation, layout,
glossaries, Zotero, and Obsidian are not part of this package.

The edge agent is only the `@论文精读` entry. It is not the processor.
Automatic OS deployment and Secret injection remain separate, unpassed gates.
Installing this archive does not claim those gates have passed.

The release contract is `docs/plans/desktop-paperloom-release-2026-10-17.md` in A3S.
