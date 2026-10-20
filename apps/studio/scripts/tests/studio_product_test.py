"""Release identity and actual package-entry fixtures; never installs an app."""

import importlib.util
import json
import os
from pathlib import Path
import plistlib
import shutil
import signal
import subprocess
import sys
import tempfile
import unittest

SCRIPTS = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("studio_product", SCRIPTS / "studio_product.py")
PRODUCT = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PRODUCT)


class ProductIdentityTests(unittest.TestCase):
    def test_only_declared_products_have_release_identities(self):
        self.assertEqual(PRODUCT.identity("aster"), {"product": "AsterDream", "identifier": "dev.a3s.studio.aster", "feature": "tenant-aster"})
        self.assertEqual(PRODUCT.identity("toyou"), {"product": "ToWork", "identifier": "dev.a3s.studio", "feature": "tenant-toyou"})
        for tenant in ("", " aster", "ASTER", "a3s", "towork-cua", "../aster"):
            with self.subTest(tenant=tenant), self.assertRaisesRegex(ValueError, "product tenant"):
                PRODUCT.identity(tenant)

    def test_missing_or_symlink_icon_refuses_tauri_generation(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            brand = root / "aster"
            brand.mkdir()
            for name in ("icon.icns", "icon.png", "icon.ico"):
                (brand / name).write_bytes(b"fixture icon")
            config = PRODUCT.tauri_config("aster", root)
            self.assertEqual(config["identifier"], "dev.a3s.studio.aster")
            icon = brand / "icon.ico"
            icon.unlink()
            with self.assertRaisesRegex(ValueError, "missing product icon"):
                PRODUCT.tauri_config("aster", root)
            icon.symlink_to(brand / "icon.png")
            with self.assertRaises(ValueError):
                PRODUCT.tauri_config("aster", root)

    def test_renderer_identity_requires_one_exact_root_without_duplicate_attributes(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            index = root / "windows/main/index.html"
            index.parent.mkdir(parents=True)
            for html in (
                '<html data-tenant="toyou" data-a3s-product="ToWork"></html>',
                '<html data-tenant="aster" data-a3s-product="ToWork"></html>',
                '<html data-tenant="aster" data-tenant="toyou" data-a3s-product="AsterDream"></html>',
                '<html data-tenant="aster" data-a3s-product="AsterDream"></html><html></html>',
                '<p data-tenant="aster" data-a3s-product="AsterDream"></p>',
            ):
                with self.subTest(html=html):
                    index.write_text(html)
                    with self.assertRaisesRegex(ValueError, "renderer identity"):
                        PRODUCT.validate_renderer(root, "aster")

    def test_renderer_identity_refuses_oversize_input(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            index = root / "windows/main/index.html"
            index.parent.mkdir(parents=True)
            index.write_bytes(b'<html data-tenant="aster" data-a3s-product="AsterDream">' + b" " * (1024 * 1024))
            with self.assertRaisesRegex(ValueError, "invalid renderer identity input"):
                PRODUCT.validate_renderer(root, "aster")

    @unittest.skipUnless(os.name == "posix", "The package entry uses POSIX commands")
    def test_package_entry_seals_the_selected_product_and_never_the_other_tenant(self):
        for tenant, product, identifier in (
            ("aster", "AsterDream", "dev.a3s.studio.aster"), ("toyou", "ToWork", "dev.a3s.studio")
        ):
            with self.subTest(tenant=tenant), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                studio = root / "apps/studio"
                scripts = studio / "scripts"
                scripts.mkdir(parents=True)
                for name in ("package-host.sh", "studio_delivery.py", "studio_product.py"):
                    shutil.copy2(SCRIPTS / name, scripts / name)
                (scripts / "subset-layout-font.py").write_text("# Fixture font step.\n")
                for slug in ("aster", "toyou"):
                    brand = root / "apps/web/public/tenant" / slug
                    brand.mkdir(parents=True)
                    for name in ("INFO.md", "logo-light.png", "mark-light.png", "icon.icns", "icon.png", "icon.ico"):
                        (brand / name).write_bytes(f"{slug}:{name}".encode())
                fonts = studio / ".py-fonttools/fontTools"
                fonts.mkdir(parents=True)
                (fonts / "__init__.py").write_text("")
                tools = root / "tools"
                tools.mkdir()
                self.command(tools / "cargo", "import json,os\nprint(json.dumps({'target_directory':os.environ['FIXTURE_TARGET']}))\n")
                for name in ("pnpm", "codesign", "ditto"):
                    self.command(tools / name, "")
                renderer = studio / "node_modules/@rsbuild/core/bin/rsbuild.js"
                renderer.parent.mkdir(parents=True)
                renderer.write_text("const fs=require('fs');fs.mkdirSync('.studio-package/windows/main',{recursive:true});"
                    "const t=process.env.A3S_TENANT_SLUG;const p=t==='aster'?'AsterDream':'ToWork';"
                    "fs.writeFileSync('.studio-package/windows/main/index.html',`<html data-tenant=\"${t}\" data-a3s-product=\"${p}\"></html>`);")
                self.command(studio / "node_modules/.bin/tauri", """import json,os,pathlib,plistlib,sys
config=json.loads(sys.argv[sys.argv.index('--config')+1])
assert '--' in sys.argv and '--no-default-features' in sys.argv[sys.argv.index('--')+1:], 'Cargo feature flags must follow the Tauri runner separator'
expected='tenant-'+os.environ['A3S_TENANT_SLUG']
assert set(sys.argv[sys.argv.index('--features')+1].split(','))=={'custom-protocol',expected}

app=pathlib.Path(os.environ['FIXTURE_TARGET'])/'release/bundle/macos'/(config['productName']+'.app')
for name in ('MacOS/Studio','Resources/agent/a3s-code-acp','Resources/agent/a3s-office'):
    path=app/'Contents'/name;path.parent.mkdir(parents=True,exist_ok=True);path.write_bytes(b'fixture binary')
info={'CFBundleExecutable':'Studio','CFBundleName':config['productName'],'CFBundleDisplayName':config['productName'],
      'CFBundleIdentifier':config['identifier'],'CFBundleShortVersionString':'2.1.3'}
(app/'Contents/Info.plist').write_bytes(plistlib.dumps(info))
""")
                env = dict(os.environ, PATH=f"{tools}:{os.environ['PATH']}", A3S_TENANT_SLUG=tenant,
                           FIXTURE_TARGET=str(root / "target"))
                env.pop("STUDIO_RENDERER_DIST", None)
                with subprocess.Popen(["bash", str(scripts / "package-host.sh")], env=env,
                                      stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True,
                                      start_new_session=True) as run:
                    try:
                        _, stderr = run.communicate(timeout=20)
                    except subprocess.TimeoutExpired:
                        os.killpg(run.pid, signal.SIGKILL)
                        run.communicate()
                        raise
                    self.assertEqual(run.returncode, 0, stderr)
                app = root / "target/release/bundle/macos" / f"{product}.app"
                info = plistlib.loads((app / "Contents/Info.plist").read_bytes())
                self.assertEqual(info["CFBundleIdentifier"], identifier)
                marker = json.loads((app / "Contents/Resources/studio-delivery.json").read_text())
                self.assertEqual(marker["tenant"], tenant)
                self.assertEqual(list((app / "Contents/Resources/tenant").iterdir()), [app / "Contents/Resources/tenant" / tenant])

    @staticmethod
    def command(path, script):
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text("#!" + sys.executable + "\n" + script)
        path.chmod(0o755)


if __name__ == "__main__":
    unittest.main()
