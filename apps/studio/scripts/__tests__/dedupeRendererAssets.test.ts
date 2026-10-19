import { describe, expect, it } from 'vitest'

import { identicalAssetGroups, rewriteAssetReferences, woff2OnlySrc } from '../dedupeRendererAssets'

describe('identical renderer assets', () => {
  it('keeps the stable public path and drops the hashed copy of the same bytes', () => {
    const bytes = new Uint8Array([1, 2, 3, 4])
    const padded = new Uint8Array(32 * 1024)
    padded.set(bytes)
    const groups = identicalAssetGroups([
      { name: 'static/wasm/b4e447b653.module.wasm', bytes: padded },
      { name: 'vendor/embedpdf/pdfium.wasm', bytes: padded },
      { name: 'static/js/index.js', bytes: new Uint8Array([9]) }
    ])
    expect(groups).toEqual([{ keep: 'vendor/embedpdf/pdfium.wasm', drop: ['static/wasm/b4e447b653.module.wasm'] }])
    const rewritten = rewriteAssetReferences('fetch("/static/wasm/b4e447b653.module.wasm")', groups)
    expect(rewritten).toBe('fetch("/vendor/embedpdf/pdfium.wasm")')
  })

  it('keeps a bundler public prefix when its relative asset is deduplicated', () => {
    const groups = [{ keep: 'vendor/embedpdf/pdfium.wasm', drop: ['static/wasm/b4e447b653.module.wasm'] }]
    const bundled = 'module.exports=runtime.p+"static/wasm/b4e447b653.module.wasm"'
    expect(rewriteAssetReferences(bundled, groups)).toBe('module.exports=runtime.p+"vendor/embedpdf/pdfium.wasm"')
  })

  it('leaves a font face that has no woff2 alone and drops the extra formats when woff2 is present', () => {
    expect(woff2OnlySrc('url(codicon.ttf) format("truetype")')).toBe('url(codicon.ttf) format("truetype")')
    expect(
      woff2OnlySrc(
        'url(KaTeX.woff2) format("woff2"), url(KaTeX.woff) format("woff"), url(KaTeX.ttf) format("truetype")'
      )
    ).toBe('url(KaTeX.woff2) format("woff2")')
  })
})
