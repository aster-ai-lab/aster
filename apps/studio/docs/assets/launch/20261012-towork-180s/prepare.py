"""Verify deterministic seeks, framing, capture provenance and the source logo."""
import asyncio
import hashlib
import json
from pathlib import Path
from playwright.async_api import async_playwright

ROOT = Path(__file__).resolve().parent

async def main():
    errors, framing = [], []
    async with async_playwright() as pw:
        browser = await pw.chromium.launch(headless=True)
        page = await browser.new_page(viewport={"width": 1920, "height": 1080})
        page.on("pageerror", lambda e: errors.append(str(e)))
        await page.goto((ROOT / "film.html").as_uri())
        await page.evaluate("window.__ready")
        for frame in range(5401):
            t = frame / 30
            misses = await page.evaluate("""t => {
              window.__seek(t); const tracks=window.__track(), bad=[];
              for(const [id,[a,b]] of Object.entries(window.__meta.inFrame)){
                if(t<a||t>b)continue; const r=tracks[id];
                if(!r||r.x<0||r.y<0||r.x+r.w>1920||r.y+r.h>1080)bad.push(id);
              } return bad;
            }""", t)
            if misses:
                framing.append({"t": t, "ids": misses})
        hashes = []
        for t in (66.0, 14.0, 66.0):
            await page.evaluate("window.__seek", t)
            hashes.append(hashlib.sha256(await page.screenshot()).hexdigest())
        assert hashes[0] == hashes[2], "Repeated seek pixels differ"
        await page.evaluate("window.__seek(178)")
        await page.screenshot(path=str(ROOT / "poster.png"))
        (ROOT / "events.json").write_text(json.dumps(await page.evaluate("window.__events()"), indent=2) + "\n")
        used = await page.evaluate("SOURCES")
        proof = []
        for name in used:
            ax = (ROOT / "capture" / f"{name}.txt").read_text()
            assert "heading 插件" not in ax, name
            png = ROOT / "capture" / f"{name}.png"
            proof.append({"name": name, "sha256": hashlib.sha256(png.read_bytes()).hexdigest()})
        assert (ROOT.parents[3] / "src/renderer/assets/images/logo.png").read_bytes() == (ROOT / "brand/towork-logo.png").read_bytes()
        await browser.close()
    assert not errors, errors
    assert not framing, framing[:10]
    result = {"sampled_frames": 5401, "seek_errors": errors, "framing_errors": framing,
              "repeated_seek_pixels_identical": True, "pixel_hashes": hashes,
              "used_captures": proof, "plugin_pages": 0, "logo_matches_source": True}
    (ROOT / "composition-check.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps({k: v for k, v in result.items() if k != "used_captures"}))

if __name__ == "__main__":
    asyncio.run(main())
