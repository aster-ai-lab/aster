import { readFileSync } from 'node:fs'
import { fileURLToPath, URL as NodeUrl } from 'node:url'

import { JSDOM } from 'jsdom'
import { afterEach, describe, expect, it, vi } from 'vitest'

const script = readFileSync(fileURLToPath(new NodeUrl('./studioMainBridge.js', import.meta.url)), 'utf8')
const windows: JSDOM[] = []
const key = 'app.harness.assemble'
const store = 'a3s.settings.preferences'
const legacy = 'a3s.studio.harness-assemble.v1'

function createBridge(saved: Record<string, unknown> = {}, old?: string, prepare?: (window: JSDOM['window']) => void) {
  const dom = new JSDOM('<html><head></head><body><div id="root"></div></body></html>', {
    url: 'http://studio.local',
    runScripts: 'outside-only'
  })
  windows.push(dom)
  const window = dom.window
  window.localStorage.setItem(store, JSON.stringify(saved))
  if (old) window.localStorage.setItem(legacy, old)
  Object.assign(window, {
    __A3S_PREFERENCE_DEFAULTS__: { [key]: 'default', 'app.language': 'zh-CN' },
    fetch: async () => new globalThis.Response('{}', { status: 200 })
  })
  prepare?.(window)
  window.eval(script)
  return window as unknown as globalThis.Window & typeof globalThis
}

afterEach(() => {
  for (const dom of windows.splice(0)) dom.window.close()
  vi.restoreAllMocks()
})

describe('Studio Harness Preference bridge', () => {
  it('migrates a legacy choice once and gives an existing unified choice precedence', async () => {
    const first = createBridge({ 'app.language': 'en-US' }, 'lean')
    expect(await first.api.preference.get(key)).toBe('lean')
    expect(JSON.parse(first.localStorage.getItem(store)!)).toEqual({ [key]: 'lean', 'app.language': 'en-US' })
    expect(first.localStorage.getItem(legacy)).toBeNull()
    const second = createBridge({ [key]: 'host' }, 'lean')
    expect(await second.api.preference.get(key)).toBe('host')
    expect(second.localStorage.getItem(legacy)).toBeNull()
  })

  it('synchronizes external choices and preserves another window’s newer settings on a write', async () => {
    const window = createBridge({ [key]: 'lean', 'app.language': 'zh-CN' })
    const observed: unknown[] = []
    const stop = window.api.preference.onChanged((name, value) => {
      if (name === key) observed.push(value)
    })
    window.localStorage.setItem(store, JSON.stringify({ [key]: 'host', 'app.language': 'en-US' }))
    window.dispatchEvent(new window.StorageEvent('storage', { key: store }))
    expect(await window.api.preference.get(key)).toBe('host')
    expect(observed).toEqual(['host'])
    await window.api.preference.set(key, 'default')
    expect(JSON.parse(window.localStorage.getItem(store)!)).toEqual({ [key]: 'default', 'app.language': 'en-US' })
    stop()
  })

  it('keeps the last persisted choice when storage rejects an update', async () => {
    const window = createBridge({ [key]: 'lean' })
    vi.spyOn(window.Storage.prototype, 'setItem').mockImplementation(() => {
      throw new Error('storage denied')
    })
    await expect(window.api.preference.set(key, 'host')).rejects.toThrow('storage denied')
    expect(await window.api.preference.get(key)).toBe('lean')
  })

  it('refuses a new capture when the unified store is unreadable without destroying the legacy choice', async () => {
    const window = createBridge({}, 'lean', (window) => window.localStorage.setItem(store, '{broken'))
    expect(await window.api.preference.get(key)).toBeUndefined()
    expect(window.localStorage.getItem(legacy)).toBe('lean')
    expect(window.localStorage.getItem(store)).toBe('{broken')
  })

  it('retains the unified choice when only legacy cleanup is denied', async () => {
    const window = createBridge({ [key]: 'host' }, 'lean', (window) => {
      vi.spyOn(window.Storage.prototype, 'removeItem').mockImplementation(() => {
        throw new Error('cleanup denied')
      })
    })
    expect(await window.api.preference.get(key)).toBe('host')
    expect(JSON.parse(window.localStorage.getItem(store)!)[key]).toBe('host')
  })
})
