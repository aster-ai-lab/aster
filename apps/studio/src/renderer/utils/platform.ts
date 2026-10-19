import { hostPlatform } from '@renderer/host/desktopHost'

export const platform = hostPlatform()
export const isMac = platform === 'darwin'
export const isWin = platform === 'win32' || platform === 'win64'
export const isLinux = platform === 'linux'
// The Tauri host does not run a dev Electron preload. Logging stays at the packaged level.
export const isDev = false
export const isProd = platform !== undefined
