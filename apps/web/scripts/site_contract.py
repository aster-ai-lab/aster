"""Closed public files and verified release links; build data, not runtime config."""

import json
from pathlib import Path
import re

PAGE_FILES = (
    "download/index.html",
    "download/styles.css",
    "download/app.js",
    "download/canvas.js",
)
HOME_FILES = (
    "index.html",
    "home.css",
    "home.js",
    "cosmic.js",
)
def documentation_routes(catalog):
    """Map each published page to its version-pinned canonical path."""
    if set(catalog) != {"projects"} or not isinstance(catalog["projects"], list):
        raise ValueError("documentation catalog must contain only a projects array")
    routes, ids = {"docs/index.html": "docs/"}, set()

    def add(route, canonical):
        filename = f"docs/{route}index.html"
        if filename in routes:
            raise ValueError(f"duplicate documentation route: {route}")
        routes[filename] = f"docs/{canonical}"

    for project in catalog["projects"]:
        if set(project) - {"id", "title", "label", "description", "defaultVersion", "versions"}:
            raise ValueError("unknown documentation project field")
        identifier = project.get("id")
        if not isinstance(identifier, str) or not re.fullmatch(r"[a-z][a-z0-9-]*", identifier) or identifier in ids:
            raise ValueError(f"invalid or duplicate documentation project: {identifier}")
        ids.add(identifier)
        for key in ("title", "label", "description"):
            if not isinstance(project.get(key), str) or not project[key].strip():
                raise ValueError(f"missing documentation project {key}")
        if not isinstance(project.get("versions"), list) or not project["versions"]:
            raise ValueError("documentation project needs versions")
        versions = set()
        for version in project["versions"]:
            if set(version) - {"id", "label", "sourceRef", "pages", "components"}:
                raise ValueError("unknown documentation version field")
            if "components" in version and (not isinstance(version["components"], str) or not re.fullmatch(r"[a-z_][a-z0-9_-]*", version["components"])):
                raise ValueError("invalid documentation version components")
            version_id = version.get("id")
            if not isinstance(version_id, str) or version_id == "index.html" or not re.fullmatch(r"[a-z0-9]+(?:[.-][a-z0-9]+)*", version_id) or version_id in versions:
                raise ValueError(f"invalid or duplicate documentation version: {version_id}")
            versions.add(version_id)
            if not isinstance(version.get("label"), str) or not version["label"].strip():
                raise ValueError("missing documentation version label")
            ref = version.get("sourceRef")
            if not isinstance(ref, str) or not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._/-]*", ref) or any(part in {"", ".", ".."} for part in ref.split("/")):
                raise ValueError("invalid documentation source ref")
            if not isinstance(version.get("pages"), list) or not version["pages"] or version["pages"][0].get("slug") != "":
                raise ValueError("documentation version needs a landing page")
            slugs, files = set(), set()
            for page in version["pages"]:
                if set(page) - {"slug", "file", "label", "title", "description", "section"}:
                    raise ValueError("unknown documentation page field")
                if "section" in page and (not isinstance(page["section"], str) or not page["section"].strip()):
                    raise ValueError("invalid documentation section")
                slug, filename = page.get("slug"), page.get("file")
                if not isinstance(slug, str) or not re.fullmatch(r"(?:[a-z0-9-]+/)*", slug) or slug in slugs:
                    raise ValueError("invalid or duplicate documentation route")
                if not isinstance(filename, str) or not re.fullmatch(r"[a-z0-9_-]+(?:/[a-z0-9_-]+)*", filename) or filename in files:
                    raise ValueError("invalid or duplicate documentation file")
                slugs.add(slug)
                files.add(filename)
                for key in ("label", "title", "description"):
                    if not isinstance(page.get(key), str) or not page[key].strip():
                        raise ValueError(f"missing documentation page {key}")
                route = f"{identifier}/{version_id}/{slug}"
                add(route, route)
        if project.get("defaultVersion") not in versions:
            raise ValueError("unknown default documentation version")
        version = next(item for item in project["versions"] if item["id"] == project["defaultVersion"])
        for page in version["pages"]:
            add(f"{identifier}/{page['slug']}", f"{identifier}/{version['id']}/{page['slug']}")
    return routes


def documentation_pages(catalog):
    return tuple(documentation_routes(catalog))


DOCS_ROUTES = documentation_routes(json.loads(
    (Path(__file__).resolve().parents[1] / "content/docs/catalog.json").read_text(encoding="utf-8")))
DOCS_PAGES = tuple(DOCS_ROUTES)

DOCS_FILES = DOCS_PAGES + ("docs/styles.css", "docs/generated.css", "docs/navigation.js")
DOCS_REFERENCE_URLS = frozenset((
    "https://developers.openai.com/blog/codex-as-a-platform",
    "https://code.claude.com/docs/en/agent-sdk/overview",
    "https://docs.langchain.com/oss/python/deepagents/overview",
    "https://docs.langchain.com/oss/python/langgraph/overview",
    "https://arxiv.org/abs/2310.02304",
    "https://arxiv.org/abs/2505.22954",
))
SCREENSHOT_FILES = (
    "download/screenshots/workspace.png",
    "download/screenshots/office.png",
    "download/screenshots/review.png",
    "download/screenshots/lineage.png",
)
FONT_FILES = (
    "download/fonts/ubuntu-latin-400-normal.woff2",
    "download/fonts/ubuntu-latin-500-normal.woff2",
    "download/fonts/ubuntu-latin-700-normal.woff2",
    "download/fonts/Ubuntu-LICENSE.txt",
)
BRAND_FILES = (
    "tenant/aster/logo.svg",
    "tenant/aster/mark-light.png",
    "tenant/aster/favicon.png",
)
DOWNLOAD_BRAND_FILES = (
    "tenant/aster/logo.svg",
    "tenant/aster/favicon.png",
)
SITE_FILES = PAGE_FILES + SCREENSHOT_FILES + FONT_FILES + BRAND_FILES

REPOSITORY_URL = "https://github.com/aster-ai-lab/aster"
PAGES_URL = "https://aster-ai-lab.github.io/aster/"

# These are existing public stable artifacts, not a claim that a new desktop
# generation has been published. Updating this set requires checking the release.
DOWNLOAD_MANIFEST = (
    ("desktop", "macos", "0.1.1", "aarch64", REPOSITORY_URL + "/releases/download/v0.1.1/aster_0.1.1_aarch64.dmg"),
    ("desktop", "windows", "0.1.1", "x64", REPOSITORY_URL + "/releases/download/v0.1.1/aster_0.1.1_x64-setup.exe"),
    ("desktop", "kylin", "0.1.2", "x86_64", REPOSITORY_URL + "/releases/download/v0.1.2/aster_0.1.2_kylin-v10_x86_64.run"),
    ("cli", "macos", "0.1.1", "release archive", REPOSITORY_URL + "/releases/download/aster-cli-v0.1.1/aster-cli-v0.1.1-macos.tar.gz"),
    ("cli", "linux", "0.1.1", "release archive", REPOSITORY_URL + "/releases/download/aster-cli-v0.1.1/aster-cli-v0.1.1-linux.tar.gz"),
    ("cli", "windows", "0.1.1", "release archive", REPOSITORY_URL + "/releases/download/aster-cli-v0.1.1/aster-cli-v0.1.1-windows.zip"),
)
