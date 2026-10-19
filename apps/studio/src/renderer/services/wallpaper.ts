const WALLPAPER_STORAGE_KEY = 'workbench.wallpaper:v1'
const WALLPAPER_EVENT = 'a3s:wallpaper'
const MAX_DATA_URL = 1_500_000
const SAFE_IMAGE = /^data:image\/(?:jpeg|png|webp);base64,[a-z0-9+/]+=*$/i

export const WALLPAPER_PRESETS = [
  {
    id: 'mist',
    image: 'linear-gradient(160deg, #e7f0ff 0%, #f7fbff 46%, #d5e3f6 100%)',
    dark: 'linear-gradient(160deg, #1e3358 0%, #17243c 46%, #2a4570 100%)'
  },
  {
    id: 'dawn',
    image: 'linear-gradient(165deg, #fff1e4 0%, #ffe4ef 48%, #e4efff 100%)',
    dark: 'linear-gradient(165deg, #4a3028 0%, #4a2838 48%, #2a3c58 100%)'
  },
  {
    id: 'dune',
    image: 'linear-gradient(180deg, #f8ead8 0%, #e4c6a2 58%, #c9a888 100%)',
    dark: 'linear-gradient(180deg, #4a3824 0%, #5c4630 58%, #3a2c1c 100%)'
  },
  {
    id: 'tide',
    image: 'linear-gradient(155deg, #d9f1f6 0%, #f5fcff 42%, #b7d6ec 100%)',
    dark: 'linear-gradient(155deg, #164048 0%, #143038 42%, #1e5060 100%)'
  },
  {
    id: 'bloom',
    image: 'radial-gradient(120% 90% at 18% 12%, #fff6d6 0%, #d9f3e6 46%, #c9d6ff 100%)',
    dark: 'radial-gradient(120% 90% at 18% 12%, #4a4024 0%, #1e4034 46%, #2a3058 100%)'
  },
  {
    id: 'paper',
    image: 'linear-gradient(180deg, #f8f5ef 0%, #efe6d4 100%)',
    dark: 'linear-gradient(180deg, #3a342c 0%, #4a4034 100%)'
  },
  {
    id: 'aurora',
    image: 'linear-gradient(140deg, #d8f4ff 0%, #e6e0ff 52%, #ffd8ee 100%)',
    dark: 'linear-gradient(140deg, #164858 0%, #322858 52%, #4a2844 100%)'
  },
  {
    id: 'stone',
    image: 'linear-gradient(160deg, #eeece8 0%, #d7d4ce 100%)',
    dark: 'linear-gradient(160deg, #323230 0%, #45423c 100%)'
  },
  {
    id: 'lagoon',
    image: 'radial-gradient(90% 80% at 72% 28%, #e7fbff 0%, #b7e6ea 48%, #8ed0c4 100%)',
    dark: 'radial-gradient(90% 80% at 72% 28%, #0e4044 0%, #1a5854 48%, #146048 100%)'
  },
  {
    id: 'ember',
    image: 'linear-gradient(165deg, #ffe8dc 0%, #f4c6b6 50%, #e7b496 100%)',
    dark: 'linear-gradient(165deg, #4a3028 0%, #5c3a30 50%, #4a2c22 100%)'
  },
  {
    id: 'frost',
    image: 'linear-gradient(180deg, #f4f8fc 0%, #e3eef8 56%, #d4e2f1 100%)',
    dark: 'linear-gradient(180deg, #1c2c40 0%, #2a3c54 56%, #203448 100%)'
  },
  {
    id: 'grove',
    image: 'radial-gradient(80% 90% at 28% 72%, #f4ffe8 0%, #d6efc6 42%, #c4e0ea 100%)',
    dark: 'radial-gradient(80% 90% at 28% 72%, #2c4024 0%, #244434 42%, #1c3840 100%)'
  }
] as const

export type WallpaperPresetId = (typeof WALLPAPER_PRESETS)[number]['id']
export type WallpaperId = 'none' | 'custom' | WallpaperPresetId
export type WallpaperTone = 'light' | 'dark'

export type WallpaperState = {
  id: WallpaperId
  custom: string | null
  hidden: boolean
}

const EMPTY: WallpaperState = { id: 'none', custom: null, hidden: false }

function isPresetId(value: unknown): value is WallpaperPresetId {
  return WALLPAPER_PRESETS.some((preset) => preset.id === value)
}

function isSafeImage(value: string): boolean {
  return value.length <= MAX_DATA_URL && SAFE_IMAGE.test(value)
}

export function readWallpaper(): WallpaperState {
  try {
    const raw = localStorage.getItem(WALLPAPER_STORAGE_KEY)
    if (!raw) return EMPTY
    const parsed = JSON.parse(raw) as Partial<WallpaperState>
    const custom = typeof parsed.custom === 'string' && isSafeImage(parsed.custom) ? parsed.custom : null
    const id: WallpaperId = parsed.id === 'custom' && custom ? 'custom' : isPresetId(parsed.id) ? parsed.id : 'none'
    return { id, custom, hidden: parsed.hidden === true }
  } catch {
    return EMPTY
  }
}

export function wallpaperTone(): WallpaperTone {
  return document.documentElement.classList.contains('dark') ? 'dark' : 'light'
}

export function wallpaperImage(state: WallpaperState, tone: WallpaperTone = wallpaperTone()): string | null {
  if (state.id === 'custom') return state.custom ? `url("${state.custom}")` : null
  const preset = WALLPAPER_PRESETS.find((item) => item.id === state.id)
  if (!preset) return null
  return tone === 'dark' ? preset.dark : preset.image
}

export function applyWallpaper(state: WallpaperState): void {
  const root = document.documentElement
  const image = wallpaperImage(state)
  if (!image) {
    root.style.removeProperty('--app-wallpaper')
    root.removeAttribute('data-wallpaper')
    return
  }
  root.style.setProperty('--app-wallpaper', image)
  root.setAttribute('data-wallpaper', state.id === 'custom' ? 'custom' : 'preset')
}

export function clearWallpaper(): void {
  localStorage.removeItem(WALLPAPER_STORAGE_KEY)
  const root = document.documentElement
  root.style.removeProperty('--app-wallpaper')
  root.removeAttribute('data-wallpaper')
  window.dispatchEvent(new Event(WALLPAPER_EVENT))
}

export function writeWallpaper(state: WallpaperState): void {
  const next = state.id === 'custom' && !state.custom ? { ...state, id: 'none' as const } : state
  localStorage.setItem(WALLPAPER_STORAGE_KEY, JSON.stringify(next))
  applyWallpaper(next)
  window.dispatchEvent(new Event(WALLPAPER_EVENT))
}

export async function wallpaperFromFile(file: File): Promise<string | null> {
  if (!file.type.startsWith('image/') || typeof createImageBitmap !== 'function') return null
  const bitmap = await createImageBitmap(file).catch(() => null)
  if (!bitmap) return null
  const scale = Math.min(1, 1600 / Math.max(bitmap.width, bitmap.height, 1))
  const width = Math.max(1, Math.round(bitmap.width * scale))
  const height = Math.max(1, Math.round(bitmap.height * scale))
  const canvas = document.createElement('canvas')
  canvas.width = width
  canvas.height = height
  const context = canvas.getContext('2d')
  if (!context) {
    bitmap.close?.()
    return null
  }
  context.drawImage(bitmap, 0, 0, width, height)
  bitmap.close?.()
  try {
    let quality = 0.82
    let url = canvas.toDataURL('image/jpeg', quality)
    while (url.length > 1_200_000 && quality > 0.5) {
      quality -= 0.12
      url = canvas.toDataURL('image/jpeg', quality)
    }
    return isSafeImage(url) ? url : null
  } catch {
    return null
  }
}
