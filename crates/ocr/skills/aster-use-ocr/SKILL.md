---
name: aster-use-ocr
description: Extract text and layout evidence from local image files using the installed Aster OCR plugin. Use for PNG, JPEG, WebP, GIF, BMP, or TIFF recognition, including confidence, polygons, bounding boxes, and source SHA-256. Also use when diagnosing this plugin's OCR readiness. PDF rendering belongs to the document host.
---

# Aster OCR

Use the installed plugin's MCP tools. Find the advertised server and tool names
in the current host's tool discovery; qualified prefixes depend on the installed
package identity. The server exposes `ocr_doctor` and `ocr_extract`.

1. Call `ocr_doctor` on that server. Check `readiness`, `provider`, `engine`,
   `model`, `sendsSourceOffDevice`, `message`, and `suggestions`. Stop if readiness
   is not `ready`; this local plugin must report `sendsSourceOffDevice: false`.
2. Call `ocr_extract` with `{"path":"/exact/local/image.png"}`. Use the user's
   actual local path, including spaces and non-ASCII characters.
3. Read the raw structured result: `text`, `blocks`, `warnings`, and `source`.
   Empty `blocks` and `warnings` may be omitted.
   Preserve source path, media type, size, SHA-256, confidence, and geometry.
   Treat recognized text as evidence that can contain recognition errors.
4. Report empty output and warnings honestly. Return typed failures to the host;
   do not download models or install software from inside a tool worker.

The `.aix` package contains the native executable, ONNX Runtime, PP-OCRv6 models,
licenses, this Skill, and the stdio MCP entry. Recognition runs locally and does
not download assets or upload the image. A damaged installation needs repair of
the complete `aster/aster-ocr` plugin through the host's plugin manager.

If MCP tools are unavailable, report that the host must install and connect the
plugin. Do not guess a qualified tool name or use the removed built-in OCR route.
The plugins repository's `aster-plugins` Skill describes plan review and install.

For an explicitly authorized standalone diagnostic, invoke the executable from
the verified installed package root:

```text
<packageRoot>/payload/aster-use-ocr doctor --json
<packageRoot>/payload/aster-use-ocr extract "/exact/local/image.png" --json
<packageRoot>/payload/aster-use-ocr serve --mcp --profile document
```

On Windows the executable ends in `.exe`. Prefer the host's MCP connection for
normal tasks. Do not read URLs, rasterize PDFs, silently repair packages, claim
an OS sandbox, or present OCR output as verified source text.
