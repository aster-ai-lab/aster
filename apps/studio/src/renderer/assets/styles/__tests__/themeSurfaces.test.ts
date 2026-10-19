import { readFileSync } from 'node:fs'
import { join } from 'node:path'

import { describe, expect, it } from 'vitest'

const responsive = readFileSync(join(process.cwd(), 'src/renderer/assets/styles/responsive.css'), 'utf8')
const surfaces = readFileSync(join(process.cwd(), 'src/renderer/assets/styles/theme-surfaces.css'), 'utf8')
const composer = readFileSync(join(process.cwd(), 'src/renderer/pages/agents/workbenchComposer.css'), 'utf8')

describe('theme surfaces', () => {
  it('does not lock settings to a light surface', () => {
    expect(responsive).not.toContain('color-scheme: light')
    expect(responsive).not.toContain('Settings stays on')
    expect(responsive).toContain('max-height: 10rem')
  })

  it('gives the dark theme a solid page color and remaps authored white surfaces', () => {
    expect(surfaces).toContain('--cs-background: #111112')
    expect(surfaces).toContain('.dark .bg-white')
    expect(surfaces).not.toContain('--app-wallpaper')
    expect(surfaces).not.toContain('data-wallpaper')
    expect(surfaces).toContain('.dark .text-\\[\\#1c1c1c\\]')
    expect(surfaces).toContain('.writing-paper')
    expect(surfaces).toContain("[data-ui='workbench.feature-thumb']")
    expect(surfaces).toContain("html.dark [data-ui='workbench.wordmark.mark']")
    expect(composer).toContain(":is(html.dark, body.dark) [data-ui~='workbench.composer'] [data-composer-inputbar]")
    expect(composer).toContain('background: #171717')
  })
})
