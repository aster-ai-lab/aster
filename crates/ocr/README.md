# Aster OCR

Provider-oriented Rust OCR interfaces and the native executable shipped in
[`aster/aster-ocr`](https://github.com/aster-ai-lab/plugins/tree/main/plugins/operations/aster/aster-ocr).
The official distribution is one self-contained `.aix` containing the executable,
ONNX Runtime, PP-OCRv6 models, licenses, stdio MCP entry and `aster-use-ocr` Skill.
The desktop base uses the provider-neutral API and calls the plugin through MCP.

```text
aster-use-ocr doctor --json
aster-use-ocr extract "/local/发票 1.png" --json
aster-use-ocr serve --mcp --profile document
```

Use the executable from the verified installed package's `payload/` directory;
Windows uses `aster-use-ocr.exe`. MCP exposes `ocr_doctor` and `ocr_extract`, with
`{"path":"/exact/local/image.png"}`. Structured results contain provider, engine,
model, original source evidence, text, blocks and warnings. Qualified tool names
come from host discovery. The retired built-in `aster use ocr` route is unavailable.

The packaged executable binds its own adjacent library and models, ahead of any
old user cache. General detection is bounded to 4000 pixels; the document profile
uses 1536. Recognition stays local and never downloads assets. The host renders
PDF pages before image recognition. Native execution is not an OS sandbox.

`OcrClient` validates bounded source input and owns its path, media type, size and
SHA-256. Injected `OcrProvider` implementations supply recognition content without
replacing that source evidence. Build with no default features for this API alone.

The existing explicit `install_ppocr_v6`, `repair_ppocr_v6`, and
`uninstall_managed_ppocr_v6` APIs now manage the complete official AIX through the
native extension lifecycle. They no longer download a separate model archive.
Unchanged verified releases reuse their installed generation. A small local
reference locates models in the canonical installed package; it does not copy them.
For normal AI use, install and authorize through the host plugin manager described
in the [plugins Skill](https://github.com/aster-ai-lab/plugins/tree/main/skills/aster-plugins).

Source and Skill live here. Release source metadata and signed catalogs live in
the plugins repository. Build `aster-use-ocr` with the pinned workspace toolchain,
then run `scripts/package_plugin.py` with explicit native executable, runtime,
model directory, notices directory, output directory and `aster-use` builder.
It checks the four pinned model digests and invokes native `registry package
build/check/prepare`; do not create ZIP files and rename them to `.aix`.

Only targets in the signed official catalog are released and runtime-qualified.
See `THIRD_PARTY_NOTICES.md` for model and runtime provenance.
