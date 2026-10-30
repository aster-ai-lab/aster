#!/usr/bin/env python3
"""Check the public homepage, downloads and MDX output without desktop sources or network."""

import argparse
from dataclasses import dataclass, field
from html.parser import HTMLParser
import json
from pathlib import Path
import re
import stat
import struct
import sys
from urllib.parse import unquote, urlsplit
import zlib

from site_contract import DOCS_FILES, DOCS_PAGES, DOCS_ROUTES, DOCS_REFERENCE_URLS, DOWNLOAD_MANIFEST, DOWNLOAD_BRAND_FILES, FONT_FILES, HOME_FILES, LEGAL_FILES, PAGE_FILES, PAGES_URL, REPOSITORY_URL, SCREENSHOT_FILES, SITE_FILES

PUBLIC_ROOT = Path(__file__).resolve().parents[1] / "dist"
DOWNLOAD_URLS = frozenset(item[4] for item in DOWNLOAD_MANIFEST)
VOID_TAGS = {"area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source", "track", "wbr"}


@dataclass
class Element:
    tag: str
    attrs: dict
    children: list = field(default_factory=list)

    def text(self):
        if self.attrs.get("aria-hidden") == "true" or self.tag in {"script", "style"}:
            return ""
        return " ".join(child.text() if isinstance(child, Element) else child for child in self.children).strip()

    def walk(self):
        yield self
        for child in self.children:
            if isinstance(child, Element):
                yield from child.walk()


class Page(HTMLParser):
    def __init__(self, source):
        super().__init__(convert_charrefs=True)
        self.root = Element("document", {})
        self.stack = [self.root]
        self.feed(source)
        self.close()

    def handle_starttag(self, tag, attrs):
        node = Element(tag, dict(attrs))
        self.stack[-1].children.append(node)
        if tag not in VOID_TAGS:
            self.stack.append(node)

    def handle_startendtag(self, tag, attrs):
        self.handle_starttag(tag, attrs)
        if tag not in VOID_TAGS:
            self.handle_endtag(tag)

    def handle_endtag(self, tag):
        for index in range(len(self.stack) - 1, 0, -1):
            if self.stack[index].tag == tag:
                del self.stack[index:]
                break

    def handle_data(self, data):
        self.stack[-1].children.append(data)


def regular(root, name):
    path = root / name
    for component in [path, *path.parents]:
        if component == root.parent:
            break
        if component.is_symlink():
            raise ValueError(f"symlink public input: {name}")
    if not stat.S_ISREG(path.stat().st_mode) or not 0 < path.stat().st_size <= 2 * 1024 * 1024:
        raise ValueError(f"missing, empty or oversize public input: {name}")
    return path


def external_link(url):
    parsed = urlsplit(url)
    if parsed.scheme != "https" or parsed.netloc != "github.com":
        raise ValueError(f"unapproved external host: {url}")
    path = unquote(parsed.path)
    if "\\" in path or any(part in {".", ".."} for part in path.split("/")):
        raise ValueError(f"invalid external path: {url}")
    repository_path = urlsplit(REPOSITORY_URL).path
    if path != repository_path and not path.startswith(repository_path + "/"):
        raise ValueError(f"unapproved repository: {url}")
    if "/releases/download/" in path and url not in DOWNLOAD_URLS:
        raise ValueError(f"unapproved download URL: {url}")
    return url in DOWNLOAD_URLS


def png_header(path):
    data = path.read_bytes()[:33]
    if (len(data) != 33 or data[:8] != b"\x89PNG\r\n\x1a\n"
            or data[8:16] != b"\x00\x00\x00\rIHDR"
            or not all(struct.unpack(">II", data[16:24]))
            or zlib.crc32(data[12:29]) != struct.unpack(">I", data[29:33])[0]):
        raise ValueError(f"invalid PNG screenshot header: {path.name}")


def check_site(public_root=PUBLIC_ROOT):
    root = Path(public_root).absolute()
    for name in SITE_FILES:
        regular(root, name)
    for name in SCREENSHOT_FILES:
        png_header(root / name)
    actual_page_files = {path.relative_to(root).as_posix() for path in (root / "download").rglob("*") if path.is_file() or path.is_symlink()}
    if actual_page_files != set(PAGE_FILES + SCREENSHOT_FILES + FONT_FILES):
        raise ValueError("unexpected or missing download-page file")
    source = {name: (root / name).read_text(encoding="utf-8") for name in PAGE_FILES}
    if any(brand in text.casefold() for text in source.values() for brand in ("minimax", "asterdream")):
        raise ValueError("foreign product branding in download page")
    page = Page(source["download/index.html"])
    nodes = list(page.root.walk())
    ids = {}
    for node in nodes:
        identifier = node.attrs.get("id")
        if identifier:
            if identifier in ids:
                raise ValueError(f"duplicate HTML id: {identifier}")
            ids[identifier] = node
    if not any(node.tag == "h1" and "Aster" in node.text() for node in nodes):
        raise ValueError("missing Aster page heading")
    seen_assets, seen_downloads, seen_images = set(), set(), set()

    def local_url(value, origin):
        parsed = urlsplit(value)
        if parsed.scheme or parsed.netloc or parsed.query or "\\" in value:
            raise ValueError(f"nonlocal public asset: {value}")
        decoded = unquote(parsed.path)
        if decoded.startswith("/"):
            raise ValueError(f"absolute public asset path breaks project Pages: {value}")
        if not decoded and not parsed.fragment:
            raise ValueError("empty public asset reference")
        target = ((root / origin).parent / decoded if decoded else root / origin).resolve()
        if not target.is_relative_to(root.resolve()):
            raise ValueError(f"public asset escapes site: {value}")
        name = target.relative_to(root.resolve()).as_posix()
        if name not in SITE_FILES:
            raise ValueError(f"unapproved local asset: {value}")
        regular(root, name)
        if parsed.fragment and (name != "download/index.html" or unquote(parsed.fragment) not in ids):
            raise ValueError(f"missing local fragment: {value}")
        seen_assets.add(name)
        return name

    for node in nodes:
        if node.tag in {"select", "datalist", "base", "style", "iframe", "object", "embed"}:
            raise ValueError(f"unsupported public element: {node.tag}")
        if any(key.startswith("on") or key in {"style", "srcset"} for key in node.attrs):
            raise ValueError(f"inline behavior/style or unsupported srcset: {node.tag}")
        if node.tag == "button":
            labelled_by = node.attrs.get("aria-labelledby", "").split()
            if any(identifier not in ids for identifier in labelled_by):
                raise ValueError("button has a missing accessible label reference")
            name = node.attrs.get("aria-label") or " ".join(ids[key].text() for key in labelled_by) or node.text()
            if not name.strip():
                raise ValueError("button has no accessible name")
        if node.tag == "a":
            href = node.attrs.get("href", "")
            if not href:
                raise ValueError("anchor has no destination")
            if href.startswith("#"):
                if unquote(href[1:]) not in ids:
                    raise ValueError(f"missing local fragment: {href}")
            elif urlsplit(href).scheme or urlsplit(href).netloc:
                if external_link(href):
                    if not node.text() and not node.attrs.get("aria-label"):
                        raise ValueError("download link has no accessible name")
                    seen_downloads.add(href)
            else:
                local_url(href, "download/index.html")
        if node.tag == "script":
            if not node.attrs.get("src") or any(isinstance(child, str) and child.strip() for child in node.children):
                raise ValueError("inline script in public page")
        for attr in ("src", "poster"):
            if attr in node.attrs:
                name = local_url(node.attrs[attr], "download/index.html")
                if node.tag == "img" and attr == "src":
                    seen_images.add(name)
        for attr in ("href", "xlink:href"):
            if attr in node.attrs and node.tag not in {"a", "link"}:
                local_url(node.attrs[attr], "download/index.html")
        if node.tag == "link" and "href" in node.attrs:
            if node.attrs.get("rel") == "canonical":
                if node.attrs["href"] != PAGES_URL + "download/":
                    raise ValueError("unexpected canonical page URL")
            else:
                local_url(node.attrs["href"], "download/index.html")
    if seen_downloads != DOWNLOAD_URLS:
        raise ValueError("page does not expose all six verified desktop/CLI download links")
    if not set(SCREENSHOT_FILES).issubset(seen_images):
        raise ValueError("page does not reference all four PNG screenshots")
    css = re.sub(r"/\*.*?\*/", "", source["download/styles.css"], flags=re.S)
    if re.search(r"@import\b", css, flags=re.I):
        raise ValueError("CSS imports are not part of the standalone site")
    for match in re.finditer(r"url\(\s*(['\"]?)(.*?)\1\s*\)", css, flags=re.I):
        local_url(match.group(2), "download/styles.css")
    required_download_assets = set(PAGE_FILES + SCREENSHOT_FILES + FONT_FILES + DOWNLOAD_BRAND_FILES) - {"download/index.html"}
    if not required_download_assets.issubset(seen_assets):
        raise ValueError("page/CSS does not reference all approved public assets")
    for filename in ("download/app.js", "download/canvas.js"):
        javascript = source[filename]
        if re.search(r"\b(?:import|require)\b", javascript):
            raise ValueError("script imports are not part of the standalone site")
        if re.search(r"createElement\s*\(\s*['\"](?:select|datalist)['\"]", javascript, flags=re.I):
            raise ValueError("script builds a native choice control")
        for url in re.findall(r"https?://[^\s'\"<>`]+", javascript):
            external_link(url)
    home = check_home_page(root)
    docs = check_docs(root)
    legal = check_legal_pages(root)
    return {"status": "passed", "files": len(SITE_FILES + HOME_FILES + DOCS_FILES), "download_links": len(seen_downloads),
            "named_buttons": sum(node.tag == "button" for node in nodes), "home_tabs": home["tabs"],
            "docs_pages": docs["pages"], "legal_pages": legal["pages"],
            "private_sources_required": False}


def check_home_page(root):
    """Validate the Aster homepage with the same closed public-resource boundary."""
    root = Path(root).absolute()
    allowed = set(SITE_FILES + HOME_FILES)
    for name in HOME_FILES:
        regular(root, name)
    source = {name: (root / name).read_text(encoding="utf-8") for name in HOME_FILES}
    if any(brand in text.casefold() for text in source.values() for brand in ("minimax", "asterdream", "workbuddy")):
        raise ValueError("foreign product branding in homepage")
    page = Page(source["index.html"])
    nodes = list(page.root.walk())
    ids = {}
    for node in nodes:
        identifier = node.attrs.get("id")
        if identifier:
            if identifier in ids:
                raise ValueError(f"duplicate homepage HTML id: {identifier}")
            ids[identifier] = node
    if not any(node.tag == "h1" and "Aster" in node.text() for node in nodes):
        raise ValueError("missing Aster homepage heading")
    seen_assets, seen_downloads, seen_images = set(), set(), set()

    def local_url(value):
        parsed = urlsplit(value)
        if parsed.scheme or parsed.netloc or parsed.query or "\\" in value:
            raise ValueError(f"nonlocal homepage asset: {value}")
        decoded = unquote(parsed.path)
        if decoded.startswith("/"):
            raise ValueError(f"absolute homepage asset path breaks project Pages: {value}")
        if not decoded and not parsed.fragment:
            raise ValueError("empty homepage asset reference")
        target = ((root / decoded) if decoded else (root / "index.html")).resolve()
        if not target.is_relative_to(root.resolve()):
            raise ValueError(f"homepage asset escapes site: {value}")
        if target.is_dir():
            target = target / "index.html"
        name = target.relative_to(root.resolve()).as_posix()
        if name not in allowed:
            raise ValueError(f"unapproved homepage asset: {value}")
        regular(root, name)
        if parsed.fragment and (name != "index.html" or unquote(parsed.fragment) not in ids):
            raise ValueError(f"missing homepage fragment: {value}")
        seen_assets.add(name)
        return name

    for node in nodes:
        if node.tag in {"select", "datalist", "base", "style", "iframe", "object", "embed"}:
            raise ValueError(f"unsupported homepage element: {node.tag}")
        if any(key.startswith("on") or key in {"style", "srcset"} for key in node.attrs):
            raise ValueError(f"inline homepage behavior/style or unsupported srcset: {node.tag}")
        if node.tag == "button":
            labelled_by = node.attrs.get("aria-labelledby", "").split()
            if any(identifier not in ids for identifier in labelled_by):
                raise ValueError("homepage button has a missing accessible label reference")
            name = node.attrs.get("aria-label") or " ".join(ids[key].text() for key in labelled_by) or node.text()
            if not name.strip():
                raise ValueError("homepage button has no accessible name")
        if node.tag == "a":
            href = node.attrs.get("href", "")
            if not href:
                raise ValueError("homepage anchor has no destination")
            if href.startswith("#"):
                if unquote(href[1:]) not in ids:
                    raise ValueError(f"missing homepage fragment: {href}")
            elif urlsplit(href).scheme or urlsplit(href).netloc:
                if external_link(href):
                    seen_downloads.add(href)
            elif href == "./docs/":
                regular(root, "docs/index.html")
            else:
                local_url(href)
        if node.tag == "script":
            if not node.attrs.get("src") or any(isinstance(child, str) and child.strip() for child in node.children):
                raise ValueError("inline script in homepage")
        for attr in ("src", "poster"):
            if attr in node.attrs:
                name = local_url(node.attrs[attr])
                if node.tag == "img" and attr == "src":
                    seen_images.add(name)
        for attr in ("href", "xlink:href"):
            if attr in node.attrs and node.tag not in {"a", "link"}:
                local_url(node.attrs[attr])
        if node.tag == "link" and "href" in node.attrs:
            if node.attrs.get("rel") == "canonical":
                if node.attrs["href"] != PAGES_URL:
                    raise ValueError("unexpected homepage canonical URL")
            else:
                local_url(node.attrs["href"])
    if seen_downloads != DOWNLOAD_URLS:
        raise ValueError("homepage does not expose all six verified desktop/CLI download links")
    if not set(SCREENSHOT_FILES).issubset(seen_images):
        raise ValueError("homepage does not reference all four PNG screenshots")
    css = re.sub(r"/\*.*?\*/", "", source["home.css"], flags=re.S)
    if re.search(r"@import\b", css, flags=re.I):
        raise ValueError("CSS imports are not part of the standalone homepage")
    for match in re.finditer(r"url\(\s*(['\"]?)(.*?)\1\s*\)", css, flags=re.I):
        local_url(match.group(2))
    required_assets = (set(HOME_FILES + SCREENSHOT_FILES + FONT_FILES) |
                       {"tenant/aster/logo.svg", "tenant/aster/mark-light.png", "tenant/aster/favicon.png"}) - {"index.html"}
    if not required_assets.issubset(seen_assets):
        missing = sorted(required_assets - seen_assets)
        raise ValueError(f"homepage does not reference required public assets: {missing}")
    for filename in ("home.js", "cosmic.js"):
        javascript = source[filename]
        if re.search(r"\b(?:import|require)\b", javascript):
            raise ValueError("script imports are not part of the standalone homepage")
        if re.search(r"createElement\s*\(\s*['\"](?:select|datalist)['\"]", javascript, flags=re.I):
            raise ValueError("homepage script builds a native choice control")
        for url in re.findall(r"https?://[^\s'\"<>`]+", javascript):
            external_link(url)
    tabs = [node for node in nodes if node.attrs.get("role") == "tab"]
    panels = [node for node in nodes if node.attrs.get("role") == "tabpanel"]
    if len(tabs) != 4 or len(panels) != 4:
        raise ValueError("homepage must expose four task scenario tabs and panels")
    return {"status": "passed", "tabs": len(tabs), "download_links": len(seen_downloads)}


def check_legal_pages(root):
    """Validate the standalone legal pages and their closed local-resource links."""
    root = Path(root).absolute()
    for name in LEGAL_FILES:
        regular(root, name)
    for directory, expected in (("terms", {"terms/index.html"}), ("privacy", {"privacy/index.html"}), ("legal", {"legal/styles.css"})):
        actual = {path.relative_to(root).as_posix() for path in (root / directory).rglob("*")
                  if path.is_file() or path.is_symlink()}
        if actual != expected:
            raise ValueError(f"unexpected or missing legal-page file: {directory}")

    allowed = set(SITE_FILES + HOME_FILES + DOCS_FILES)
    pages, identifiers = {}, {}
    for name in ("terms/index.html", "privacy/index.html"):
        nodes = list(Page((root / name).read_text(encoding="utf-8")).root.walk())
        pages[name] = nodes
        ids = [node.attrs["id"] for node in nodes if node.attrs.get("id")]
        if len(ids) != len(set(ids)):
            raise ValueError(f"duplicate legal-page HTML id: {name}")
        identifiers[name] = set(ids)
        if sum(node.tag == "h1" and "Aster" in node.text() for node in nodes) != 1:
            raise ValueError(f"legal page needs one Aster heading: {name}")
        if not any(node.tag == "main" and node.attrs.get("id") == "content" for node in nodes):
            raise ValueError(f"legal page needs a main landmark: {name}")

    def local_url(value, origin):
        parsed = urlsplit(value)
        if parsed.scheme or parsed.netloc or parsed.query or "\\" in value:
            raise ValueError(f"nonlocal legal-page asset: {value}")
        decoded = unquote(parsed.path)
        if decoded.startswith("/") or (not decoded and not parsed.fragment):
            raise ValueError(f"invalid legal-page path: {value}")
        target = ((root / origin).parent / decoded if decoded else root / origin).resolve()
        if not target.is_relative_to(root.resolve()):
            raise ValueError(f"legal-page asset escapes site: {value}")
        if target.is_dir():
            target = target / "index.html"
        name = target.relative_to(root.resolve()).as_posix()
        if name not in allowed:
            raise ValueError(f"unapproved legal-page asset: {value}")
        regular(root, name)
        if parsed.fragment and unquote(parsed.fragment) not in identifiers.get(name, set()):
            raise ValueError(f"missing legal-page fragment: {value}")
        return name

    for name, nodes in pages.items():
        for node in nodes:
            if node.tag in {"script", "select", "datalist", "style", "iframe", "base", "object", "embed"}:
                raise ValueError(f"unsupported legal-page element: {node.tag}")
            if any(key.startswith("on") or key in {"style", "srcset"} for key in node.attrs):
                raise ValueError(f"inline legal-page behavior/style: {node.tag}")
            if node.tag == "img" and "alt" not in node.attrs:
                raise ValueError(f"legal-page image needs alt text: {name}")
            if node.tag == "a":
                href = node.attrs.get("href", "")
                if not href:
                    raise ValueError("legal-page anchor has no destination")
                parsed = urlsplit(href)
                if parsed.scheme or parsed.netloc:
                    external_link(href)
                else:
                    local_url(href, name)
            if node.tag == "link" and "href" in node.attrs:
                if node.attrs.get("rel") == "canonical":
                    route = "terms/" if name.startswith("terms/") else "privacy/"
                    if node.attrs["href"] != PAGES_URL + route:
                        raise ValueError(f"unexpected legal-page canonical: {name}")
                else:
                    local_url(node.attrs["href"], name)
            for attr in ("src", "poster"):
                if attr in node.attrs:
                    local_url(node.attrs[attr], name)
            for attr in ("href", "xlink:href"):
                if attr in node.attrs and node.tag not in {"a", "link"}:
                    local_url(node.attrs[attr], name)

    name = "legal/styles.css"
    css = re.sub(r"/\*.*?\*/", "", (root / name).read_text(encoding="utf-8"), flags=re.S)
    if re.search(r"@import\b", css, flags=re.I):
        raise ValueError("CSS imports are not part of legal pages")
    for match in re.finditer(r"url\(\s*(['\"]?)(.*?)\1\s*\)", css, flags=re.I):
        local_url(match.group(2), name)
    return {"pages": len(pages)}


def check_docs(root):
    """Check generated MDX pages without widening the download asset boundary."""
    root = Path(root).absolute()
    for name in DOCS_FILES:
        regular(root, name)
    actual = {path.relative_to(root).as_posix() for path in (root / "docs").rglob("*")
              if path.is_file() or path.is_symlink()}
    if actual != set(DOCS_FILES):
        raise ValueError("unexpected or missing documentation file")
    pages, identifiers = {}, {}
    for name in DOCS_PAGES:
        nodes = list(Page((root / name).read_text(encoding="utf-8")).root.walk())
        pages[name] = nodes
        ids = [node.attrs["id"] for node in nodes if node.attrs.get("id")]
        if len(ids) != len(set(ids)):
            raise ValueError(f"duplicate documentation HTML id: {name}")
        identifiers[name] = set(ids)
        if sum(node.tag == "h1" and bool(node.text()) for node in nodes) != 1:
            raise ValueError(f"documentation needs one page heading: {name}")
        if not any(node.tag == "main" and node.attrs.get("id") == "content" for node in nodes):
            raise ValueError(f"documentation needs a main landmark: {name}")

    def local_url(value, origin):
        parsed = urlsplit(value)
        if parsed.scheme or parsed.netloc or parsed.query or "\\" in value:
            raise ValueError(f"nonlocal documentation asset: {value}")
        decoded = unquote(parsed.path)
        if decoded.startswith("/") or (not decoded and not parsed.fragment):
            raise ValueError(f"invalid documentation path: {value}")
        target = ((root / origin).parent / decoded if decoded else root / origin).resolve()
        if not target.is_relative_to(root.resolve()):
            raise ValueError(f"documentation asset escapes site: {value}")
        if target.is_dir():
            target = target / "index.html"
        name = target.relative_to(root.resolve()).as_posix()
        if name not in DOCS_FILES + SITE_FILES + HOME_FILES:
            raise ValueError(f"unapproved documentation asset: {value}")
        regular(root, name)
        if parsed.fragment and unquote(parsed.fragment) not in identifiers.get(name, set()):
            raise ValueError(f"missing documentation fragment: {value}")
        return name

    for name, nodes in pages.items():
        if sum(node.tag == "script" for node in nodes) != 2:
            raise ValueError("unsupported documentation script count")
        for node in nodes:
            if node.tag == "script":
                src = node.attrs.get("src", "")
                if node.children or set(node.attrs) != {"src", "defer"}:
                    raise ValueError("unsupported documentation script")
                if local_url(src, name) not in {"docs/navigation.js", "docs/search.js"}:
                    raise ValueError("unsupported documentation script")
            if node.tag in {"select", "datalist", "style", "iframe", "base", "object", "embed"}:
                raise ValueError(f"unsupported documentation element: {node.tag}")
            if any(key.startswith("on") or key in {"style", "srcset"} for key in node.attrs):
                raise ValueError(f"inline documentation behavior/style: {node.tag}")
            if node.tag == "link" and node.attrs.get("rel") == "canonical":
                if node.attrs.get("href") != PAGES_URL + DOCS_ROUTES[name]:
                    raise ValueError(f"unexpected documentation canonical: {name}")
                continue
            if node.tag == "img" and "alt" not in node.attrs:
                raise ValueError(f"documentation image needs alt text: {name}")
            if node.tag == "svg":
                labels = node.attrs.get("aria-labelledby", "").split()
                if not labels or any(label not in identifiers[name] for label in labels):
                    raise ValueError(f"documentation diagram needs an accessible label: {name}")
            for attr in ("src", "href", "xlink:href"):
                if attr not in node.attrs:
                    continue
                value = node.attrs[attr]
                parsed = urlsplit(value)
                if node.tag == "a" and (parsed.scheme or parsed.netloc):
                    if value not in DOCS_REFERENCE_URLS:
                        external_link(value)
                else:
                    local_url(value, name)
    for name in (name for name in DOCS_FILES if name.endswith(".css")):
        css = re.sub(r"/\*.*?\*/", "", (root / name).read_text(encoding="utf-8"), flags=re.S)
        if re.search(r"@import\b", css, flags=re.I):
            raise ValueError("CSS imports are not part of documentation")
        for match in re.finditer(r"url\(\s*(['\"]?)(.*?)\1\s*\)", css, flags=re.I):
            local_url(match.group(2), name)
    return {"pages": len(pages)}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--public-root", type=Path, default=PUBLIC_ROOT)
    try:
        print(json.dumps(check_site(parser.parse_args().public_root)))
    except (ValueError, OSError, UnicodeError) as error:
        print(f"site check failed: {error}", file=sys.stderr)
        raise SystemExit(1)
