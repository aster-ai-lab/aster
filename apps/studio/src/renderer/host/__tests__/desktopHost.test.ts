import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'

import { beforeEach, describe, expect, it, vi } from 'vitest'

import { LOG_TO_HOST_COMMAND, commandMod, desktopCapabilities, logToHost } from '@renderer/host/desktopHost'

import type * as LoggerModule from '../../services/LoggerService'

const bridgePath = resolve(__dirname, '../../shell/studioMainBridge.js')

describe('Tauri host bridge', () => {
  beforeEach(() => {
    document.head.innerHTML = '<meta name="logger-window-source" content="mainWindow" />'
    document.body.innerHTML = '<div id="root"></div>'
    delete (globalThis as { electron?: unknown }).electron
    delete (globalThis as { ipcRenderer?: unknown }).ipcRenderer
    delete (globalThis as { __A3S_CHERRY_MAIN__?: unknown }).__A3S_CHERRY_MAIN__
    delete (globalThis as { __TAURI__?: unknown }).__TAURI__
    vi.spyOn(console, 'error').mockImplementation(() => {})
    vi.spyOn(console, 'warn').mockImplementation(() => {})
    vi.spyOn(console, 'info').mockImplementation(() => {})
  })

  it('loads the real window bridge without installing electron or ipcRenderer', () => {
    const source = readFileSync(bridgePath, 'utf8')
    expect(source).not.toMatch(/globalThis\.electron\s*=/)
    expect(source).not.toMatch(/window\.electron\s*=/)
    expect(source).not.toMatch(/ipcRenderer/)
    expect(source).toContain('globalThis.api')

    const run = new Function(source)
    run()

    expect((globalThis as { electron?: unknown }).electron).toBeUndefined()
    expect((globalThis as { ipcRenderer?: unknown }).ipcRenderer).toBeUndefined()
    expect(globalThis.api).toBeTypeOf('object')
  })

  it('completes the log host call through the Tauri command channel', async () => {
    const invoke = vi.fn().mockResolvedValue(undefined)
    ;(globalThis as { __TAURI__?: { core: { invoke: typeof invoke } } }).__TAURI__ = { core: { invoke } }

    await expect(logToHost({ process: 'renderer', window: 'mainWindow' }, 'error', 'boom', [])).resolves.toBeUndefined()
    expect(invoke).toHaveBeenCalledWith(LOG_TO_HOST_COMMAND, {
      source: { process: 'renderer', window: 'mainWindow' },
      level: 'error',
      message: 'boom',
      data: []
    })

    invoke.mockClear()
    // `@logger` is mocked for the rest of the renderer suite. Load the shipped class.
    const { LoggerService } = await vi.importActual<typeof LoggerModule>('../../services/LoggerService')
    const logger = new LoggerService()
    logger.error('from the window')
    expect(invoke).toHaveBeenCalledWith(
      LOG_TO_HOST_COMMAND,
      expect.objectContaining({ level: 'error', message: 'from the window' })
    )
    await expect(Promise.resolve(invoke.mock.results[0]?.value)).resolves.toBeUndefined()
  })

  it('does not throw when the page has no invoke function', async () => {
    ;(globalThis as { __TAURI__?: { core: Record<string, unknown> } }).__TAURI__ = { core: {} }
    await expect(logToHost({ process: 'renderer' }, 'warn', 'quiet', [])).resolves.toBeUndefined()
  })
})

type DesktopGlobal = { __A3S_STUDIO_DESKTOP__?: unknown }

const CLOSED = {
  chrome: 'native' as const,
  openExternal: false,
  revealInFolder: false,
  chooseDirectory: false,
  screenshot: false,
  singleInstance: false,
  autoUpdate: false,
  chromeCookieImport: false
}

function freezeDesktop(value: Record<string, unknown>) {
  const frozen = Object.freeze({ ...value })
  ;(globalThis as DesktopGlobal).__A3S_STUDIO_DESKTOP__ = frozen
  return frozen
}

describe('desktop capabilities', () => {
  beforeEach(() => {
    delete (globalThis as DesktopGlobal).__A3S_STUDIO_DESKTOP__
  })

  it('fails closed when the host object is missing or not an object', () => {
    expect(desktopCapabilities()).toEqual(CLOSED)
    expect(desktopCapabilities().platform).toBeUndefined()
    ;(globalThis as DesktopGlobal).__A3S_STUDIO_DESKTOP__ = 'darwin'
    expect(desktopCapabilities()).toEqual(CLOSED)
    expect(commandMod(undefined)).toBe('ctrl')
  })

  it.each([
    ['darwin', 'overlay', true, true, true, 'cmd'],
    ['win32', 'native', false, false, true, 'ctrl'],
    ['linux', 'native', false, true, false, 'ctrl']
  ] as const)(
    'reads a frozen %s capability object',
    (platform, chrome, screenshot, singleInstance, chromeCookieImport, mod) => {
      const frozen = freezeDesktop({
        platform,
        arch: platform === 'darwin' ? 'arm64' : 'x64',
        chrome,
        openExternal: true,
        revealInFolder: true,
        chooseDirectory: true,
        screenshot,
        singleInstance,
        autoUpdate: screenshot,
        chromeCookieImport
      })

      expect(Object.isFrozen(frozen)).toBe(true)
      expect(desktopCapabilities()).toEqual({
        platform,
        arch: platform === 'darwin' ? 'arm64' : 'x64',
        chrome,
        openExternal: true,
        revealInFolder: true,
        chooseDirectory: true,
        screenshot,
        singleInstance,
        autoUpdate: screenshot,
        chromeCookieImport
      })
      expect(commandMod(platform)).toBe(mod)
    }
  )

  it('treats unknown platforms and non-boolean flags as closed', () => {
    freezeDesktop({
      platform: 'freebsd',
      arch: 'ia32',
      chrome: 'custom',
      openExternal: 1,
      revealInFolder: 'yes',
      chooseDirectory: false,
      screenshot: true,
      singleInstance: true,
      autoUpdate: false,
      chromeCookieImport: true
    })

    expect(desktopCapabilities()).toEqual({
      ...CLOSED,
      screenshot: true,
      singleInstance: true,
      chromeCookieImport: true
    })
    expect(commandMod(desktopCapabilities().platform)).toBe('ctrl')
  })
})
