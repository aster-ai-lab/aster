interface ObsidianAPI {
  getVaults: () => Promise<Array<{ path: string; name: string }>>
  getFiles: (vaultName: string) => Promise<Array<{ path: string; type: 'folder' | 'markdown'; name: string }>>
}

interface DesktopIpcRenderer {
  invoke: (...args: any[]) => Promise<any>
  on: (channel: string, listener: (...args: any[]) => void) => () => void
  send: (...args: any[]) => void
  once: (channel: string, listener: (...args: any[]) => void) => void
  off: (channel: string, listener: (...args: any[]) => void) => void
  removeListener: (channel: string, listener: (...args: any[]) => void) => void
  removeAllListeners: (channel?: string) => void
}

interface Window {
  /** Installed by the Tauri window bridge. */
  api: any
  obsidian: ObsidianAPI
  root: HTMLElement
  /** Present only when a legacy page stub installs it. The Tauri window does not. */
  electron?: {
    process?: {
      platform?: string
      env?: Record<string, string | undefined>
    }
    ipcRenderer?: DesktopIpcRenderer
  }
}
