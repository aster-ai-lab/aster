"""Inspect a real A3S Test download; no rendering or conversion is mocked."""

import json
from pathlib import Path
import struct
import sys
import zlib


def inspect_png(artifact: Path, gray: int) -> dict:
    data = artifact.read_bytes()
    if len(data) > 1_048_576 or not data.startswith(b"\x89PNG\r\n\x1a\n"):
        raise ValueError("Unexpected PNG download")
    position = 8
    encoded = b""
    dimensions = None
    while position < len(data):
        size = struct.unpack("!I", data[position:position + 4])[0]
        kind = data[position + 4:position + 8]
        body = data[position + 8:position + 8 + size]
        position += 12 + size
        if kind == b"IHDR":
            dimensions = struct.unpack("!2I", body[:8])
        elif kind == b"IDAT":
            encoded += body
    if dimensions != (4, 4):
        raise ValueError(f"Unexpected output dimensions: {dimensions}")
    # The production encoder emits filter-zero RGBA scanlines for this fixture.
    expected = b"".join(b"\0" + bytes([gray, gray, gray, 255]) * 4 for _ in range(4))
    if zlib.decompress(encoded) != expected:
        raise ValueError("Downloaded pixels differ from the reviewed grayscale result")
    return {
        "artifact": str(artifact),
        "width": 4,
        "height": 4,
        "expected_rgba": [gray, gray, gray, 255],
        "pixel_check": "passed",
    }


def main() -> None:
    report = json.loads(Path(sys.argv[1]).read_text())
    if report["status"] != "passed":
        raise ValueError("The browser regression did not pass")
    results = []
    for name, gray in {"downloads/source-gray.png": 71, "downloads/third-gray.png": 92}.items():
        downloads = [
            evidence
            for scenario in report["scenarios"]
            for step in scenario["steps"]
            for evidence in (step.get("output") or {}).get("evidence", [])
            if evidence["name"] == name
        ]
        if len(downloads) != 1:
            raise ValueError(f"Expected exactly one PNG download: {name}")
        results.append(inspect_png(Path(downloads[0]["path"]), gray))
    result = {"run_id": report["run_id"], "downloads": results, "pixel_check": "passed"}
    Path(sys.argv[2]).write_text(json.dumps(result, indent=2) + "\n")


if __name__ == "__main__":
    main()
