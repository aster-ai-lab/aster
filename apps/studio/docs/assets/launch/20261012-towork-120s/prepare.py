"""Check every frame's seek, repeated pixels and native-capture provenance."""
import asyncio
import hashlib
import json
from pathlib import Path
from playwright.async_api import async_playwright

ROOT = Path(__file__).resolve().parent

async def main():
    errors = []
    async with async_playwright() as pw:
        browser = await pw.chromium.launch(headless=True)
        page = await browser.new_page(viewport={"width": 1920, "height": 1080})
        page.on("pageerror", lambda e: errors.append(str(e)))
        await page.goto((ROOT / "film.html").as_uri())
        await page.evaluate("window.__ready")
        for frame in range(3601):
            await page.evaluate("t => window.__seek(t)", frame / 30)
        hashes = []
        for t in (63.0, 12.0, 63.0):
            await page.evaluate("t => window.__seek(t)", t)
            hashes.append(hashlib.sha256(await page.screenshot()).hexdigest())
        assert hashes[0] == hashes[2], "Repeated seek must reproduce identical pixels"
        await page.evaluate("window.__seek(118)")
        await page.screenshot(path=str(ROOT / "poster.png"))
        events = await page.evaluate("window.__events()")
        (ROOT / "events.json").write_text(json.dumps(events, indent=2) + "\n")
        used = await page.evaluate("SOURCES")
        proof = []
        for name in used:
            ax = (ROOT / "capture" / f"{name}.txt").read_text()
            assert "heading 插件" not in ax and "未知错误" not in ax and "这个操作没有完成" not in ax, name
            png = ROOT / "capture" / f"{name}.png"
            proof.append({"name": name, "sha256": hashlib.sha256(png.read_bytes()).hexdigest()})
        source_logo = ROOT.parents[3] / "src/renderer/assets/images/logo.png"
        assert source_logo.read_bytes() == (ROOT / "brand/towork-logo.png").read_bytes()
        await browser.close()
    assert not errors, errors
    report = {"sampled_frames": 3601, "seek_errors": errors, "repeated_seek_pixels_identical": True, "pixel_hashes": hashes, "used_captures": proof, "plugin_pages": 0, "logo_matches_source": True}
    (ROOT / "composition-check.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({k:v for k,v in report.items() if k != "used_captures"}))

if __name__ == "__main__":
    asyncio.run(main())
