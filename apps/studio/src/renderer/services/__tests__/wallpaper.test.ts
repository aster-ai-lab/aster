import { afterEach, describe, expect, it } from 'vitest'

import {
  applyWallpaper,
  clearWallpaper,
  readWallpaper,
  WALLPAPER_PRESETS,
  writeWallpaper
} from '@renderer/services/wallpaper'

afterEach(() => {
  localStorage.removeItem('workbench.wallpaper:v1')
  document.documentElement.classList.remove('dark')
  document.documentElement.removeAttribute('data-wallpaper')
  document.documentElement.style.removeProperty('--app-wallpaper')
})

describe('wallpaper', () => {
  it('keeps a preset and ignores a custom value that is not an image data url', () => {
    writeWallpaper({ id: 'dawn', custom: 'https://example.test/a.png', hidden: false })

    expect(readWallpaper()).toEqual({ id: 'dawn', custom: null, hidden: false })
    expect(document.documentElement.getAttribute('data-wallpaper')).toBe('preset')
    expect(document.documentElement.style.getPropertyValue('--app-wallpaper')).toContain('linear-gradient')
  })

  it('drops a broken record and a custom choice that has no image', () => {
    localStorage.setItem('workbench.wallpaper:v1', '{')
    expect(readWallpaper()).toEqual({ id: 'none', custom: null, hidden: false })

    writeWallpaper({ id: 'custom', custom: null, hidden: false })
    expect(readWallpaper().id).toBe('none')
    expect(document.documentElement.hasAttribute('data-wallpaper')).toBe(false)
  })

  it('removes the wallpaper variable when the choice is none', () => {
    applyWallpaper({ id: 'mist', custom: null, hidden: false })
    applyWallpaper({ id: 'none', custom: null, hidden: false })

    expect(document.documentElement.style.getPropertyValue('--app-wallpaper')).toBe('')
    expect(document.documentElement.hasAttribute('data-wallpaper')).toBe(false)
  })

  it('paints the dark twin of the same choice when the theme is dark', () => {
    writeWallpaper({ id: 'dawn', custom: null, hidden: false })
    const light = document.documentElement.style.getPropertyValue('--app-wallpaper')
    expect(light).toContain('#fff1e4')

    document.documentElement.classList.add('dark')
    applyWallpaper(readWallpaper())
    const dark = document.documentElement.style.getPropertyValue('--app-wallpaper')

    expect(readWallpaper().id).toBe('dawn')
    expect(dark).toContain('#4a3028')
    expect(dark).not.toContain('#fff1e4')
    expect(dark.startsWith('linear-gradient')).toBe(true)
  })

  it('keeps a custom photo when the theme is dark', () => {
    const custom = 'data:image/jpeg;base64,abc='
    document.documentElement.classList.add('dark')
    writeWallpaper({ id: 'custom', custom, hidden: false })

    expect(readWallpaper()).toEqual({ id: 'custom', custom, hidden: false })
    expect(document.documentElement.style.getPropertyValue('--app-wallpaper')).toBe(`url("${custom}")`)
  })

  it('clears a saved choice so it cannot keep painting', () => {
    writeWallpaper({ id: 'dawn', custom: null, hidden: false })
    clearWallpaper()

    expect(localStorage.getItem('workbench.wallpaper:v1')).toBeNull()
    expect(readWallpaper().id).toBe('none')
    expect(document.documentElement.hasAttribute('data-wallpaper')).toBe(false)
    expect(document.documentElement.style.getPropertyValue('--app-wallpaper')).toBe('')
  })

  it('gives every preset a dark image of the same gradient kind', () => {
    expect(WALLPAPER_PRESETS).toHaveLength(12)
    for (const preset of WALLPAPER_PRESETS) {
      const kind = preset.image.startsWith('radial-gradient') ? 'radial-gradient' : 'linear-gradient'
      expect(preset.dark.startsWith(kind)).toBe(true)
      expect(preset.dark).not.toBe(preset.image)
    }
  })
})
