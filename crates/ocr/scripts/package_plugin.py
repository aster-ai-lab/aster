#!/usr/bin/env python3
"""Assemble the self-contained OCR plugin with the native Aster AIX tools."""
import argparse
import hashlib
import json
from pathlib import Path
import platform
import shutil
import subprocess
import tempfile

WORKSPACE = Path(__file__).resolve().parents[3]
MODEL_FILES = {
    "det/inference.onnx": "d73e0058b7a8086bbd57f3d10b8bcd4ff95363f67e06e2762b5e814fe9c9410e",
    "det/inference.yml": "193f435274bf9f0b5f71a929bbfbcf148282df7e633b34e7c373e8f44741b516",
    "rec/inference.onnx": "5435fd747c9e0efe15a96d0b378d5bd157e9492ed8fd80edf08f30d02fa24634",
    "rec/inference.yml": "ab078671bb49f06228eadccd34f1bb501e157f7a047095ffb943ba81512c77d1",
}


def digest(path):
    value = hashlib.sha256()
    with Path(path).open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            value.update(chunk)
    return value.hexdigest()


def host_token():
    return {("Darwin", "arm64"): "darwin-arm64", ("Darwin", "x86_64"): "darwin-x86_64",
            ("Linux", "aarch64"): "linux-arm64", ("Linux", "x86_64"): "linux-x86_64",
            ("Windows", "AMD64"): "windows-x86_64"}[(platform.system(), platform.machine())]


def stage_plugin(source, binary, runtime, models, notices, target):
    source.mkdir(parents=True)
    executable = "aster-use-ocr.exe" if target.startswith("windows-") else "aster-use-ocr"
    library = ("onnxruntime.dll" if target.startswith("windows-") else
               "libonnxruntime.dylib" if target.startswith("darwin-") else "libonnxruntime.so")
    payload = source / "payload"
    payload.mkdir()
    for path, name in ((binary, executable), (runtime, library)):
        if path.is_symlink() or not path.is_file() or not 0 < path.stat().st_size <= 256 * 1024 * 1024:
            raise ValueError("invalid OCR native build input: " + str(path))
        shutil.copy2(path, payload / name)
    (payload / executable).chmod(0o755)
    for name, expected in MODEL_FILES.items():
        original = models / name
        if original.is_symlink() or digest(original) != expected:
            raise ValueError("OCR model checksum mismatch: " + name)
        destination = payload / "ocr-models/PP-OCRv6_small" / name
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(original, destination)
    shutil.copytree(notices, payload / "licenses")
    shutil.copy2(WORKSPACE / "crates/ocr/THIRD_PARTY_NOTICES.md", payload / "licenses/ocr-NOTICES.md")
    return finish_source(source, target)


def finish_source(source, target):
    template = WORKSPACE / "plugins/plugins/aster/aster-ocr"
    for name in ("README.md", "LICENSE", "aster-use-extension.acl"):
        shutil.copy2(template / name, source / name)
    executable = "aster-use-ocr.exe" if target.startswith("windows-") else "aster-use-ocr"
    shutil.copytree(WORKSPACE / "crates/ocr/skills/aster-use-ocr", source / "skills/aster-use-ocr")
    manifest = source / "aster-use-extension.acl"
    text = manifest.read_text(encoding="utf-8").replace("payload/aster-use-ocr\"", "payload/" + executable + "\"")
    manifest.write_text(text, encoding="utf-8")
    (source / "runtime.acl").write_text(
        'runtime_component {\n schema_version = 1\n kind = "aster-ocr"\n target = "' + target + '"\n}\n', encoding="utf-8")
    content = hashlib.sha256()
    for path in sorted(source.rglob("*")):
        if path.is_symlink():
            raise ValueError("linked OCR package input")
        if path.is_file():
            content.update(path.relative_to(source).as_posix().encode("utf-8") + b"\0")
            content.update(bytes.fromhex(digest(path)))
    version = "0.1.0+" + content.hexdigest()[:40]
    manifest.write_text(text.replace('version = "0.1.0"', 'version = "' + version + '"'), encoding="utf-8")
    return version


def catalog_metadata(target):
    return {"schemaVersion": 1, "displayName": "Aster OCR", "description": "Local OCR engine, runtime, models, MCP and Skill.",
            "keywords": ["desktop", "images", "local", "ocr"], "categories": ["operations"],
            "target": target, "channel": "stable", "license": "MIT",
            "permissionCeiling": {"schema": "aster.use.plugin-permissions.v1", "surfaces": [{
                "surface": {"kind": "mcp", "id": "ocr"}, "nativeExecution": True,
                "childProcess": False, "filesystem": [{"scope": "temporary", "path": ".", "access": "read-write"},
                                                         {"scope": "workspace", "path": ".", "access": "read"}],
                "networkEgress": [], "privateService": False, "secrets": [], "uiHttp": [],
                "resources": {"cpuMillis": 1000, "memoryBytes": 2147483648, "pids": 32, "ephemeralStorageBytes": 67108864}}]}}


def package(source, out, aster, version, target):
    archive = out / f"aster-ocr-{version}-{target}.aix"
    def run(*arguments):
        result = subprocess.run([str(aster), "registry", "package", *map(str, arguments), "--json"],
                                capture_output=True, text=True, encoding="utf-8", cwd=WORKSPACE)
        if result.returncode:
            raise RuntimeError(result.stdout + result.stderr)
        return json.loads(result.stdout)
    if not archive.exists():
        run("build", source, "--output", archive)
    evidence = run("check", archive)["data"]["aixPackage"]
    metadata = out / (archive.stem + ".catalog-metadata.json")
    metadata.write_text(json.dumps(catalog_metadata(target), indent=2) + "\n", encoding="utf-8")
    with tempfile.TemporaryDirectory(dir=out) as temporary:
        temporary = Path(temporary)
        publication = run("prepare", archive, "--repository", out / "repository" / target, "--domain", "operations",
                          "--catalog-metadata", metadata, "--output", temporary / "oci.json",
                          "--planning-output", temporary / "planning.json")
        (temporary / "oci.json").replace(out / (archive.stem + ".oci.json"))
        (temporary / "planning.json").replace(out / (archive.stem + ".planning.json"))
    (out / (archive.stem + ".publication.json")).write_text(json.dumps(publication, indent=2) + "\n", encoding="utf-8")
    return {"packageId": "aster/aster-ocr", "version": version, "required": False, "target": target,
            "archive": archive.name, "sha256": digest(archive), "size": archive.stat().st_size,
            "packageSha256": evidence["packageSha256"], "manifestSha256": digest(source / "aster-use-extension.acl")}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("binary", "runtime", "models", "notices", "out", "aster-use-bin"):
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    out, target = args.out.resolve(), host_token()
    out.mkdir(parents=True, exist_ok=True)
    repository = out / "repository" / target
    source = repository / "plugins/aster/aster-ocr"
    if source.exists():
        shutil.rmtree(source)
    version = stage_plugin(source, args.binary.resolve(), args.runtime.resolve(), args.models.resolve(), args.notices.resolve(), target)
    shutil.copyfile(WORKSPACE / "plugins/domains.acl", repository / "domains.acl")
    pin = package(source, out, args.aster_use_bin.resolve(), version, target)
    print(json.dumps(pin, indent=2))


if __name__ == "__main__":
    main()
