"""Shared catalog, source pinning, and workspace preparation for ../tasks."""

from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import tempfile
import zipfile
import xml.etree.ElementTree as ET


HERE = Path(__file__).resolve().parent
CATALOG_PATH = HERE / "catalog.json"
PIN_PATH = HERE / "expected" / "source-pin.json"
TASKS_ROOT = HERE.parents[4].parent / "tasks"
NS = {"m": "http://schemas.openxmlformats.org/spreadsheetml/2006/main"}
REL_NS = "{http://schemas.openxmlformats.org/officeDocument/2006/relationships}"


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def load_catalog() -> list[dict]:
    cases = json.loads(CATALOG_PATH.read_text(encoding="utf-8"))["cases"]
    ids = [case["id"] for case in cases]
    if len(ids) != 25 or len(set(ids)) != 25:
        raise ValueError("catalog must contain 25 distinct cases")
    return cases


def case_by_id(case_id: str) -> dict:
    for case in load_catalog():
        if case["id"] == case_id:
            return case
    raise KeyError(case_id)


def case_dir(case: dict) -> Path | None:
    if "dir" not in case:
        return None
    return TASKS_ROOT / case["dir"]


def source_files(root: Path = TASKS_ROOT) -> dict[str, str]:
    found = {}
    for path in root.rglob("*"):
        if path.is_symlink() or not path.is_file():
            continue
        if any(part.startswith(".") for part in path.relative_to(root).parts):
            continue
        found[path.relative_to(root).as_posix()] = sha256(path)
    return dict(sorted(found.items()))


def sheet_cell(sheet_name: str, cell_ref: str, workbook: Path = TASKS_ROOT / "list.xlsx") -> str:
    with zipfile.ZipFile(workbook) as archive:
        shared = []
        if "xl/sharedStrings.xml" in archive.namelist():
            root = ET.fromstring(archive.read("xl/sharedStrings.xml"))
            for item in root.findall("m:si", NS):
                shared.append("".join(node.text or "" for node in item.findall(".//m:t", NS)))
        book = ET.fromstring(archive.read("xl/workbook.xml"))
        rels = ET.fromstring(archive.read("xl/_rels/workbook.xml.rels"))
        targets = {rel.attrib["Id"]: rel.attrib["Target"] for rel in rels}
        for sheet in book.findall("m:sheets/m:sheet", NS):
            if sheet.attrib.get("name") != sheet_name:
                continue
            target = targets[sheet.attrib[REL_NS + "id"]].lstrip("/")
            if not target.startswith("xl/"):
                target = "xl/" + target
            body = ET.fromstring(archive.read(target))
            for cell in body.findall(".//m:c", NS):
                if cell.attrib.get("r") != cell_ref:
                    continue
                value = cell.find("m:v", NS)
                if cell.attrib.get("t") == "s" and value is not None and value.text:
                    return shared[int(value.text)]
                inline = cell.find("m:is", NS)
                if inline is not None:
                    return "".join(node.text or "" for node in inline.findall(".//m:t", NS))
                return value.text if value is not None and value.text else ""
    raise KeyError(f"{sheet_name}!{cell_ref}")


def prompt_for(case: dict) -> str:
    if "prompt" in case:
        return case["prompt"]
    if "prompt_file" in case:
        text = (case_dir(case) / case["prompt_file"]).read_text(encoding="utf-8")
        return text.lstrip("\ufeff").strip()
    cell = sheet_cell("功能点", f"C{case['list_row']}")
    marker = case.get("prompt_split")
    if marker:
        if marker not in cell:
            raise ValueError(f"{case['id']} cell has no {marker}")
        return cell.split(marker, 1)[1].strip()
    return cell.strip()


def prepare_job(case: dict, job: Path | None = None) -> Path:
    """Copy only allow-listed files. The prompt stays in the job root."""
    if job is None:
        job = Path(tempfile.mkdtemp(prefix=f"a3s-studio-reference-{case['id']}-"))
        os.chmod(job, 0o700)
    job.mkdir(parents=True, exist_ok=True)
    (job / ".owned").touch()
    (job / "instruction.txt").write_text(prompt_for(case), encoding="utf-8")
    workspace = job / "workspace"
    workspace.mkdir()
    source = case_dir(case)
    copied = {}
    for relative in case["allow"]:
        origin = source / relative
        if not origin.is_file() or origin.is_symlink():
            raise ValueError(f"allow entry is not a regular file: {relative}")
        target = workspace / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(origin.read_bytes())
        copied[relative] = sha256(target)
    for relative in case["deny"]:
        if (workspace / relative).exists():
            raise ValueError(f"denied file entered the workspace: {relative}")
    (job / "copied.json").write_text(json.dumps(copied, ensure_ascii=False, indent=2), encoding="utf-8")
    return job
