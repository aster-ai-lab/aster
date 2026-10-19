/** Desktop host calls for the Tauri window. This module does not install an Electron global. */

export const LOG_TO_HOST_COMMAND = 'log_to_host'

type HostPlatform = 'darwin' | 'win32' | 'linux'

type TauriInvoke = (command: string, args?: Record<string, unknown>) => Promise<unknown>

function tauriInvoke(): TauriInvoke | undefined {
  const tauri = (globalThis as { __TAURI__?: { core?: { invoke?: TauriInvoke } } }).__TAURI__
  const invoke = tauri?.core?.invoke
  return typeof invoke === 'function' ? invoke : undefined
}

export type DesktopChrome = 'overlay' | 'native'

export type DesktopCapabilities = {
  platform?: HostPlatform
  arch?: 'x64' | 'arm64'
  chrome: DesktopChrome
  openExternal: boolean
  revealInFolder: boolean
  chooseDirectory: boolean
  screenshot: boolean
  singleInstance: boolean
  autoUpdate: boolean
  chromeCookieImport: boolean
}

const CLOSED_CAPABILITIES: DesktopCapabilities = {
  chrome: 'native',
  openExternal: false,
  revealInFolder: false,
  chooseDirectory: false,
  screenshot: false,
  singleInstance: false,
  autoUpdate: false,
  chromeCookieImport: false
}

function flag(value: unknown): boolean {
  return value === true
}

/** Capabilities frozen by the Tauri document-start script. Missing means every action is off. */
export function desktopCapabilities(): DesktopCapabilities {
  const raw = (globalThis as { __A3S_STUDIO_DESKTOP__?: unknown }).__A3S_STUDIO_DESKTOP__
  if (!raw || typeof raw !== 'object') return CLOSED_CAPABILITIES
  const record = raw as Record<string, unknown>
  const platform =
    record.platform === 'darwin' || record.platform === 'win32' || record.platform === 'linux'
      ? record.platform
      : undefined
  const arch = record.arch === 'x64' || record.arch === 'arm64' ? record.arch : undefined
  return {
    platform,
    arch,
    chrome: record.chrome === 'overlay' ? 'overlay' : 'native',
    openExternal: flag(record.openExternal),
    revealInFolder: flag(record.revealInFolder),
    chooseDirectory: flag(record.chooseDirectory),
    screenshot: flag(record.screenshot),
    singleInstance: flag(record.singleInstance),
    autoUpdate: flag(record.autoUpdate),
    chromeCookieImport: flag(record.chromeCookieImport)
  }
}

export function commandMod(platform: HostPlatform | undefined): 'cmd' | 'ctrl' {
  return platform === 'darwin' ? 'cmd' : 'ctrl'
}

/** Platform written by the host capability object. Unset in tests and plain browsers. */
export function hostPlatform(): HostPlatform | undefined {
  return desktopCapabilities().platform
}

/**
 * Forward one renderer log through the Tauri command channel.
 * Resolves with the command result. Rejects only when the channel itself rejects.
 * When the page has no Tauri bridge, the promise resolves so a missing host cannot throw.
 */
export function logToHost(source: unknown, level: string, message: string, data: unknown): Promise<unknown> {
  const invoke = tauriInvoke()
  if (!invoke) return Promise.resolve(undefined)
  return Promise.resolve(invoke(LOG_TO_HOST_COMMAND, { source, level, message, data }))
}
