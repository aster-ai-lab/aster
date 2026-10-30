"""Small offline proofs for public-site refusal and artifact boundaries."""

from pathlib import Path
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
import shutil
import json
import struct
import sys
import tempfile
import threading
import unittest
import zlib
from urllib.error import HTTPError
from urllib.parse import urljoin
from urllib.request import urlopen

WEB_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(WEB_ROOT / "scripts"))
from check_site import check_site
from site_contract import BRAND_FILES, DOCS_FILES, DOCS_PAGES, DOWNLOAD_MANIFEST, HOME_FILES, LEGAL_FILES, SHARED_FILES, SITE_FILES, documentation_pages
from stage_site import stage_site

SCREENSHOTS = tuple(f"download/screenshots/{name}.png" for name in ("workspace", "office", "review", "lineage"))
FONTS = tuple(f"download/fonts/ubuntu-latin-{weight}-normal.woff2" for weight in (400, 500, 700))
FONT_LICENSE = "download/fonts/Ubuntu-LICENSE.txt"


def png_fixture(width=1, height=1):
    def chunk(kind, data):
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))

    header = struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", header) + chunk(b"IDAT", zlib.compress(b"\x00\x00\x5a\xbb\xff")) + chunk(b"IEND", b"")


class SiteTest(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.public = self.root / "public"
        self.page = self.public / "download/index.html"
        self.page.parent.mkdir(parents=True)
        for name in BRAND_FILES + HOME_FILES + DOCS_FILES + LEGAL_FILES + SHARED_FILES:
            destination = self.public / name
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(WEB_ROOT / "dist" / name, destination)
        downloads = "".join(f'<a href="{item[4]}">{item[0]} {item[1]} {item[2]} {item[3]}</a>' for item in DOWNLOAD_MANIFEST)
        self.html = ('<!doctype html><html lang="zh-CN"><head><title>Aster</title>'
                     '<link rel="stylesheet" href="styles.css">'
                     '<link rel="icon" href="../tenant/aster/favicon.png">'
                     '<script src="app.js" defer></script></head><body>'
                     '<script src="canvas.js" defer></script>'
                     '<h1 id="top">Aster</h1><img src="../tenant/aster/logo.svg" alt="Aster">'
                     '<a href="#top">顶部</a><button aria-label="选择平台"><span aria-hidden="true">+</span></button>'
                     + downloads + '</body></html>')
        self.page.write_text(self.html, encoding="utf-8")
        self.css = self.public / "download/styles.css"
        self.css.write_text("body { color: #005abb; }", encoding="utf-8")
        self.javascript = self.public / "download/app.js"
        self.javascript.write_text('document.documentElement.dataset.ready = "true";', encoding="utf-8")
        (self.page.parent / 'canvas.js').write_text('document.documentElement.dataset.canvas = "true";', encoding='utf-8')
        self.add_screenshots_and_fonts()

    def rewrite(self, old, new):
        self.page.write_text(self.html.replace(old, new), encoding="utf-8")

    def add_screenshots_and_fonts(self):
        for name in SCREENSHOTS:
            path = self.public / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(png_fixture())
        for name in (*FONTS, FONT_LICENSE):
            path = self.public / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(b"offline font fixture" if name in FONTS else b"font license fixture")
        images = ''.join(f'<img src="{name.removeprefix("download/")}" alt="Aster screenshot">' for name in SCREENSHOTS)
        self.html = self.html.replace('</body>', images + '<a href="fonts/Ubuntu-LICENSE.txt">Ubuntu license</a></body>')
        self.page.write_text(self.html, encoding="utf-8")
        self.css.write_text('body { color: #005abb; }\n' + '\n'.join(
            f'@font-face {{ font-family: Ubuntu; font-weight: {weight}; src: url("fonts/ubuntu-latin-{weight}-normal.woff2"); }}'
            for weight in (400, 500, 700)), encoding="utf-8")

    def test_four_png_screenshots_and_three_local_fonts_with_license_are_public(self):
        self.assertEqual(check_site(self.public)["files"], len(SITE_FILES + HOME_FILES + DOCS_FILES))
        destination = self.root / 'with-real-ui'
        receipt = stage_site(self.public, destination)
        self.assertEqual(len(receipt['files']), len(SITE_FILES + HOME_FILES + DOCS_FILES))
        for name in (*SCREENSHOTS, *FONTS, FONT_LICENSE):
            self.assertEqual((destination / name).read_bytes(), (self.public / name).read_bytes())

    def test_legal_pages_are_staged_and_refuse_remote_or_injected_resources(self):
        self.assertEqual(check_site(self.public)['legal_pages'], 2)
        for name, heading, other in (
            ('terms/index.html', 'Aster 用户协议', '../privacy/'),
            ('privacy/index.html', 'Aster 隐私政策', '../terms/'),
        ):
            source = (self.public / name).read_text()
            self.assertIn(heading, source)
            self.assertIn(other, source)
            self.assertIn('id="content"', source)
        destination = self.root / 'legal-pages'
        stage_site(self.public, destination)
        for name in LEGAL_FILES:
            self.assertEqual((destination / name).read_bytes(), (self.public / name).read_bytes())

        page = self.public / 'terms/index.html'
        original = page.read_text()
        page.write_text(original.replace('</body>', '<script src="https://example.com/tracker.js"></script></body>'))
        with self.assertRaisesRegex(ValueError, 'unsupported legal-page'):
            check_site(self.public)
        page.write_text(original)
        extra = self.public / 'privacy/private.txt'
        extra.write_text('not public')
        with self.assertRaisesRegex(ValueError, 'unexpected or missing legal-page'):
            check_site(self.public)

    def test_documentation_catalog_supports_products_versions_and_closed_routes(self):
        catalog = json.loads((WEB_ROOT / 'content/docs/catalog.json').read_text())
        original = documentation_pages(catalog)
        with self.assertRaisesRegex(ValueError, 'only a projects array'):
            documentation_pages({**catalog, 'unexpected': True})
        invalid = json.loads(json.dumps(catalog))
        invalid['projects'][0]['versions'][0]['pages'][0]['section'] = ''
        with self.assertRaisesRegex(ValueError, 'invalid documentation section'):
            documentation_pages(invalid)
        duplicate = json.loads(json.dumps(catalog))
        version = duplicate['projects'][0]['versions'][0]
        version['pages'].append({**version['pages'][0], 'slug': 'another/'})
        with self.assertRaisesRegex(ValueError, 'duplicate documentation file'):
            documentation_pages(duplicate)
        extra = json.loads(json.dumps(catalog['projects'][0]))
        extra['id'] = 'another-product'
        extra['versions'] = [dict(extra['versions'][0], pages=[extra['versions'][0]['pages'][0]])]
        catalog['projects'].append(extra)
        expanded = documentation_pages(catalog)
        self.assertEqual(expanded[:len(original)], original)
        self.assertIn(f"docs/another-product/{extra['defaultVersion']}/index.html", expanded)
        self.assertIn('docs/another-product/index.html', expanded)
        project = catalog['projects'][-1]
        project['versions'].append(dict(project['versions'][0], id='10.0'))
        self.assertIn('docs/another-product/10.0/index.html', documentation_pages(catalog))
        for value in ('../', '/absolute/', 'guide/../', 'index.html'):
            project['versions'][-1]['pages'] = [{**project['versions'][0]['pages'][0]}, {**project['versions'][0]['pages'][0], 'slug': value, 'file': 'guide'}]
            with self.subTest(slug=value), self.assertRaisesRegex(ValueError, 'invalid'):
                documentation_pages(catalog)
        project['versions'][-1]['pages'] = [project['versions'][0]['pages'][0]]
        project['defaultVersion'] = 'missing'
        with self.assertRaisesRegex(ValueError, 'default documentation version'):
            documentation_pages(catalog)

    def test_documentation_refuses_injected_behavior_external_assets_and_broken_navigation(self):
        page = self.public / 'docs/index.html'
        original = page.read_text()
        for old, new, reason in (
            ('</body>', '<script src="https://example.com/tracker.js"></script></body>', 'unsupported documentation'),
            ('href="./styles.css"', 'href="https://example.com/style.css"', 'nonlocal documentation'),
            ('src="./navigation.js"', 'src="./styles.css"', 'unsupported documentation script'),
            ('<script src="./navigation.js" defer=""></script>', '<script src="./navigation.js" defer="">alert(1)</script>', 'unsupported documentation script'),
            ('href="#content"', 'href="#missing-section"', 'missing documentation fragment'),
            ('id="content"', 'id="content" onclick="alert(1)"', 'inline documentation'),
            ('href="./../"', 'href="../../private/"', 'escapes site'),
        ):
            with self.subTest(reason=reason):
                self.assertIn(old, original)
                page.write_text(original.replace(old, new, 1))
                with self.assertRaisesRegex(ValueError, reason):
                    check_site(self.public)
        page.write_text(original)
        stylesheet = self.public / 'docs/generated.css'
        original_css = stylesheet.read_text()
        for payload, reason in (
            ('@import url("https://example.com/styles.css");', 'CSS imports'),
            ('.injected { background: url("https://example.com/tracker.png"); }', 'nonlocal documentation'),
        ):
            with self.subTest(reason=reason):
                stylesheet.write_text(original_css + payload)
                with self.assertRaisesRegex(ValueError, reason):
                    check_site(self.public)
        stylesheet.write_text(original_css)
        extra = self.public / 'docs/private.mdx'
        extra.write_text('not a public document')
        with self.assertRaisesRegex(ValueError, 'unexpected or missing documentation'):
            check_site(self.public)
        extra.unlink()
        self.assertEqual(check_site(self.public)['docs_pages'], len(DOCS_PAGES))

    def test_missing_invalid_or_oversize_png_screenshot_refuses(self):
        image = self.public / SCREENSHOTS[0]
        image.unlink()
        with self.assertRaises(OSError):
            check_site(self.public)
        for data in (b'not a PNG', png_fixture(height=0), png_fixture()[:29] + b'\x00\x00\x00\x00',
                     png_fixture() + b'\x00' * (2 * 1024 * 1024)):
            with self.subTest(bytes=len(data)):
                image.write_bytes(data)
                with self.assertRaisesRegex(ValueError, 'PNG|oversize'):
                    check_site(self.public)

    def test_each_screenshot_is_an_explicit_html_image_and_font_assets_are_referenced(self):
        original = '<img src="screenshots/workspace.png" alt="Aster screenshot">'
        for replacement in ('', '<a href="screenshots/workspace.png">Screenshot</a>'):
            with self.subTest(replacement=replacement):
                self.rewrite(original, replacement)
                with self.assertRaisesRegex(ValueError, 'four PNG'):
                    check_site(self.public)
        self.page.write_text(self.html, encoding='utf-8')
        self.css.write_text(self.css.read_text().replace('src: url("fonts/ubuntu-latin-500-normal.woff2");', ''))
        with self.assertRaisesRegex(ValueError, 'all approved public assets'):
            check_site(self.public)

    def test_extra_screenshot_and_symlink_font_are_not_staged(self):
        font = self.public / FONTS[0]
        original = font.read_bytes()
        font.unlink()
        font.symlink_to(self.root / 'private-font.woff2')
        with self.assertRaisesRegex(ValueError, 'symlink'):
            stage_site(self.public, self.root / 'refuse-symlink')
        self.assertFalse((self.root / 'refuse-symlink').exists())
        font.unlink()
        font.write_bytes(original)
        (self.public / 'download/screenshots/private-preview.png').write_bytes(png_fixture())
        with self.assertRaisesRegex(ValueError, 'unexpected'):
            stage_site(self.public, self.root / 'refuse-extra')
        self.assertFalse((self.root / 'refuse-extra').exists())

    def test_valid_page_has_named_controls_and_all_six_exact_release_links(self):
        result = check_site(self.public)
        self.assertEqual(result["download_links"], 6)
        self.assertEqual(result["named_buttons"], 1)

    def test_same_document_svg_symbol_references_resolve_and_unknown_symbols_refuse(self):
        self.rewrite('</body>', '<svg><symbol id="brand-symbol"></symbol><use href="#brand-symbol"/></svg></body>')
        self.assertEqual(check_site(self.public)['status'], 'passed')
        self.rewrite('</body>', '<svg><use href="#absent-symbol"/></svg></body>')
        with self.assertRaisesRegex(ValueError, 'fragment'):
            check_site(self.public)

    def test_missing_asset_fragment_or_duplicate_id_refuses(self):
        for old, new in (("styles.css", "missing.css"), ('href="#top"', 'href="#absent"'),
                         ('<h1 id="top">', '<span id="top"></span><h1 id="top">')):
            with self.subTest(new=new):
                self.rewrite(old, new)
                with self.assertRaises((ValueError, OSError)):
                    check_site(self.public)

    def test_unnamed_or_invalid_labelled_control_refuses(self):
        for replacement in ('<button></button>', '<button aria-labelledby="absent"></button>'):
            with self.subTest(replacement=replacement):
                self.rewrite('<button aria-label="选择平台"><span aria-hidden="true">+</span></button>', replacement)
                with self.assertRaisesRegex(ValueError, "button"):
                    check_site(self.public)

    def test_native_choices_inline_code_and_remote_scripts_refuse(self):
        for snippet in ('<select><option>Aster</option></select>', '<datalist></datalist>',
                        '<script>alert(1)</script>', '<style>body{}</style>',
                        '<button onclick="alert(1)">Go</button>', '<script src="https://cdn.example/app.js"></script>'):
            with self.subTest(snippet=snippet):
                self.rewrite('</body>', snippet + '</body>')
                with self.assertRaises(ValueError):
                    check_site(self.public)

    def test_download_host_path_or_unknown_release_refuses(self):
        original = DOWNLOAD_MANIFEST[0][4]
        for changed in (original.replace('github.com', 'github.com.evil.example'),
                        original.replace('/aster-ai-lab/aster/', '/someone/aster/'),
                        original.replace('/' + original.split('/releases/download/')[1].split('/')[0] + '/', '/v9.9.9/'), original + '?redirect=1'):
            with self.subTest(changed=changed):
                self.rewrite(original, changed)
                with self.assertRaisesRegex(ValueError, "host|repository|download"):
                    check_site(self.public)

    def test_missing_one_verified_download_refuses(self):
        self.rewrite(f'<a href="{DOWNLOAD_MANIFEST[0][4]}">', '<a href="https://github.com/aster-ai-lab/aster/releases">')
        with self.assertRaisesRegex(ValueError, "six verified"):
            check_site(self.public)

    def test_foreign_brand_or_private_script_import_refuses(self):
        for brand in ('MiniMax', 'AsterDream'):
            with self.subTest(brand=brand):
                self.rewrite('<h1 id="top">Aster</h1>', f'<h1 id="top">{brand}</h1>')
                with self.assertRaisesRegex(ValueError, "branding"):
                    check_site(self.public)
        self.page.write_text(self.html, encoding="utf-8")
        for code in ('import "../../studio/src/main.js";', 'import("../../studio/source.js");',
                     'require("../../studio/source.js");', 'document.createElement("select");'):
            with self.subTest(code=code):
                self.javascript.write_text(code, encoding="utf-8")
                with self.assertRaisesRegex(ValueError, "imports|choice"):
                    check_site(self.public)

    def test_remote_css_and_escaping_resource_refuse(self):
        self.css.write_text('body { background: url("https://cdn.example/hero.png"); }', encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "nonlocal"):
            check_site(self.public)
        self.css.write_text('body {}', encoding="utf-8")
        self.rewrite('../tenant/aster/logo.svg', '../../../private-source.svg')
        with self.assertRaisesRegex(ValueError, "escapes"):
            check_site(self.public)
        self.rewrite('</body>', '<svg><use href="https://cdn.example/sprite.svg#icon"></use></svg></body>')
        with self.assertRaisesRegex(ValueError, "nonlocal"):
            check_site(self.public)

    def test_symlink_source_and_extra_download_file_refuse(self):
        original = self.css.read_bytes()
        self.css.unlink()
        external = self.root / 'private.css'
        external.write_bytes(b'private source fixture')
        self.css.symlink_to(external)
        with self.assertRaisesRegex(ValueError, "symlink"):
            check_site(self.public)
        self.css.unlink()
        self.css.write_bytes(original)
        (self.page.parent / 'private-source.txt').write_text('not public')
        with self.assertRaisesRegex(ValueError, "unexpected"):
            check_site(self.public)

    def test_stage_contains_only_public_allowlist_and_project_relative_homepage(self):
        other = self.public / 'tenant/toyou/private.txt'
        other.parent.mkdir(parents=True)
        other.write_text('not this public product')
        destination = self.root / 'staged'
        receipt = stage_site(self.public, destination)
        self.assertEqual(set(receipt['files']), set(SITE_FILES + HOME_FILES + DOCS_FILES))
        homepage = (destination / 'index.html').read_text()
        self.assertIn('id="scenarios"', homepage)
        self.assertIn('href="./terms/"', homepage)
        self.assertIn('href="./privacy/"', homepage)
        for name in SITE_FILES:
            self.assertEqual((destination / name).read_bytes(), (self.public / name).read_bytes())
        self.assertFalse((destination / 'tenant/toyou').exists())
        self.assertEqual(check_site(destination)['status'], 'passed')

    def test_existing_stage_is_preserved_and_bad_source_cannot_activate_output(self):
        destination = self.root / 'staged'
        destination.mkdir()
        sentinel = destination / 'keep.txt'
        sentinel.write_bytes(b'existing artifact')
        with self.assertRaisesRegex(ValueError, 'already exists'):
            stage_site(self.public, destination)
        self.assertEqual(sentinel.read_bytes(), b'existing artifact')
        self.page.write_text('<h1>MiniMax</h1>', encoding='utf-8')
        missing = self.root / 'must-not-exist'
        with self.assertRaises(ValueError):
            stage_site(self.public, missing)
        self.assertFalse(missing.exists())

    def test_staged_assets_resolve_under_a_project_prefix_and_other_tenants_return_404(self):
        stage_site(self.public, self.root / 'aster')

        class QuietHandler(SimpleHTTPRequestHandler):
            def log_message(self, *_):
                pass

        server = ThreadingHTTPServer(('127.0.0.1', 0), partial(QuietHandler, directory=str(self.root)))
        worker = threading.Thread(target=server.serve_forever, daemon=True)
        worker.start()
        try:
            origin = f'http://127.0.0.1:{server.server_port}/aster/'
            for relative in SITE_FILES + HOME_FILES + DOCS_FILES:
                with self.subTest(relative=relative), urlopen(urljoin(origin, relative), timeout=2) as response:
                    self.assertEqual(response.status, 200)
                    self.assertEqual(response.read(), (self.public / relative).read_bytes())
            with urlopen(origin, timeout=2) as response:
                homepage = response.read()
                self.assertIn(b'home.css', homepage)
                self.assertIn('让想法落地'.encode(), homepage)
            for relative in ('tenant/toyou/INFO.md', 'src/private.js', 'content/docs/catalog.json', 'content/docs/code/index.mdx', 'content/docs/code/9.1.5/index.mdx', 'content/docs/code/9.1.5/_components.mjs', 'content/docs/code/9.1.5/_diagrams/architecture.mmd'):
                with self.subTest(relative=relative), self.assertRaises(HTTPError) as refused:
                    urlopen(origin + relative, timeout=2)
                self.assertEqual(refused.exception.code, 404)
                refused.exception.close()
        finally:
            server.shutdown()
            server.server_close()
            worker.join(timeout=2)


if __name__ == '__main__':
    unittest.main()
