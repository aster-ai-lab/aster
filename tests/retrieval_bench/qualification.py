"""Frozen source-level labels for actual-owner retrieval qualification."""

import hashlib
import json
from collections import Counter
from pathlib import Path, PurePosixPath


MAX_FIXTURE_BYTES = 1024 * 1024
CATEGORIES = {"symbol", "behavior", "architecture", "cjk", "memory", "sources"}
OWNERS = {"workspace", "memory", "source"}


def load_fixture(path):
    with Path(path).open("rb") as source:
        raw = source.read(MAX_FIXTURE_BYTES + 1)
    if len(raw) > MAX_FIXTURE_BYTES:
        raise ValueError("qualification fixture exceeds 1 MiB")
    fixture = json.loads(raw.decode("utf-8"))
    if fixture["schema_version"] != 1 or fixture["ranking_unit"] != "source-id":
        raise ValueError("unsupported qualification schema/ranking unit")
    documents = fixture["documents"]
    queries = fixture["queries"]
    if not (1 <= len(documents) <= 256 and 60 <= len(queries) <= 256):
        raise ValueError("qualification fixture has invalid record counts")
    by_id = {}
    paths = set()
    for document in documents:
        path = document["path"]
        relative = PurePosixPath(path)
        if (relative.is_absolute() or relative.as_posix() != path
                or ".." in relative.parts or "\\" in path or ":" in path):
            raise ValueError("qualification document path is not safely relative")
        if document["id"] in by_id or path in paths:
            raise ValueError("duplicate qualification document identity")
        if document["owner"] not in OWNERS:
            raise ValueError("unknown qualification owner")
        content = document["content"].encode("utf-8")
        if not content or len(content) > 4096 or b"\r" in content:
            raise ValueError("qualification source must be short UTF-8/LF text")
        if hashlib.sha256(content).hexdigest() != document["sha256"]:
            raise ValueError("qualification source digest mismatch")
        by_id[document["id"]] = document
        paths.add(path)
    seen_queries = set()
    for query in queries:
        if query["id"] in seen_queries:
            raise ValueError("duplicate qualification query ID")
        seen_queries.add(query["id"])
        if query["category"] not in CATEGORIES or query["owner"] not in OWNERS:
            raise ValueError("unknown qualification category/owner")
        if not query["query"].strip() or len(query["query"].encode("utf-8")) > 1024:
            raise ValueError("invalid qualification query")
        if type(query["negative"]) is not bool:
            raise ValueError("qualification negative flag must be boolean")
        if query["negative"] != (not query["grades"]):
            raise ValueError("negative queries cannot grade relevant evidence")
        for source_id, grade in query["grades"].items():
            if (source_id not in by_id or by_id[source_id]["owner"] != query["owner"]
                    or type(grade) is not int or not 1 <= grade <= 3):
                raise ValueError("invalid qualification relevance label")
        evidence_ids = set()
        for span in query["evidence"]:
            source_id = span["source_id"]
            if source_id not in query["grades"]:
                raise ValueError("evidence does not belong to a graded source")
            lines = by_id[source_id]["content"].splitlines()
            start, end = span["start_line"], span["end_line"]
            if (type(start) is not int or type(end) is not int
                    or not 1 <= start <= end <= len(lines)
                    or not span["quote"] or span["quote"] not in "\n".join(lines[start - 1:end])):
                raise ValueError("qualification evidence is outside its source span")
            evidence_ids.add(source_id)
        if evidence_ids != set(query["grades"]):
            raise ValueError("every relevant source needs independent evidence")
    counts = Counter(query["category"] for query in queries)
    if set(counts) != CATEGORIES or min(counts.values()) < 10:
        raise ValueError("qualification requires six categories with ten queries each")
    fixture["fixture_sha256"] = hashlib.sha256(raw).hexdigest()
    return fixture


def write_workspace(fixture, root):
    if root.exists() and any(root.iterdir()):
        raise ValueError("qualification corpus must be an empty directory")
    for document in fixture["documents"]:
        if document["owner"] == "workspace":
            destination = root / document["path"]
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_bytes(document["content"].encode("utf-8"))


def workspace_ranking(fixture, hits):
    """Normalize returned identities without consulting relevance labels."""
    documents = {d["path"]: d for d in fixture["documents"] if d["owner"] == "workspace"}
    ranking = []
    seen = set()
    for hit in hits:
        document = documents.get(hit["path"])
        if document is None:
            raise ValueError("retrieval attributed evidence to an unknown source")
        lines = document["content"].splitlines()
        start, end = hit["start_line"], hit["end_line"]
        if not 1 <= start <= end <= len(lines):
            raise ValueError(f"retrieval returned invalid source bounds: {hit['path']}:{start}-{end}")
        if hit.get("content_digest") != "sha256:" + document["sha256"]:
            raise ValueError(f"retrieval source digest mismatch: {hit['path']}")
        if hit.get("digest_verified") is not True or type(hit.get("snippet_truncated")) is not bool:
            raise ValueError(f"retrieval returned unverified source evidence: {hit['path']}")
        rendered = hit.get("text", "").splitlines()
        if not rendered or rendered[0] != "":
            raise ValueError("retrieval source text disagrees with the frozen bytes")
        numbered = [line for line in rendered[1:] if line != "       | ..."]
        if len(numbered) != end - start + 1:
            raise ValueError("retrieval source text has the wrong line count")
        for number, rendered_line in zip(range(start, end + 1), numbered):
            anchor, separator, shown = rendered_line.partition(" | ")
            original = lines[number - 1]
            matches = shown == original
            if hit["snippet_truncated"] and not matches:
                prefix, suffix = shown.startswith("..."), shown.endswith("...")
                fragment = shown[3 if prefix else 0:len(shown) - 3 if suffix else len(shown)]
                matches = bool(fragment) and ((prefix and suffix and fragment in original)
                          or (prefix and not suffix and original.endswith(fragment))
                          or (suffix and not prefix and original.startswith(fragment)))
            if anchor != f"{number:>6}" or separator != " | " or not matches:
                raise ValueError("retrieval source text disagrees with the frozen bytes")
        if document["id"] not in seen:
            seen.add(document["id"])
            ranking.append(document["id"])
    return ranking
