import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import process from 'node:process'
import { fileURLToPath, URL as NodeUrl } from 'node:url'

import { JSDOM } from 'jsdom'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { nativePreferenceTransport } from '../../../tests/helpers/nativePreferences'

vi.unmock('@data/PreferenceService')

const executable = process.env.A3S_CLOUD_TEST_PREFERENCE_EXECUTABLE
const script = readFileSync(fileURLToPath(new NodeUrl('./studioMainBridge.js', import.meta.url)), 'utf8')
const member = '11111111-1111-4111-8111-111111111111'
const other = '22222222-2222-4222-8222-222222222222'
const key = 'app.harness.assemble'
const bridges: Array<{ dom: JSDOM; transport: ReturnType<typeof nativePreferenceTransport> }> = []
const events = new Set<() => void>()
let root: string

beforeEach(() => {
  root = mkdtempSync(join(tmpdir(), 'a3s-native-preference-proof-'))
})
afterEach(async () => {
  vi.unstubAllGlobals()
  events.clear()
  for (const { dom, transport } of bridges.splice(0)) {
    dom.window.close()
    await transport.close()
  }
  rmSync(root, { recursive: true, force: true })
})

async function bridge(subject = member, legacy?: string) {
  if (!executable) throw new Error('Native preference executable required')
  const transport = nativePreferenceTransport(executable, root)
  await transport.request('member', { member: subject })
  const dom = new JSDOM('<html><body><div id="root"></div></body></html>', {
    url: 'http://studio.local',
    runScripts: 'outside-only'
  })
  bridges.push({ dom, transport })
  if (legacy) dom.window.localStorage.setItem('a3s.studio.harness-assemble.v1', legacy)
  const callbacks = new Map<number, () => void>()
  Object.assign(dom.window, {
    __A3S_STUDIO_RUNTIME__: { token: 'test-only-host-token', apiBaseUrl: 'http://studio.local' },
    __A3S_PREFERENCE_DEFAULTS__: { [key]: 'default', 'app.language': 'zh-CN' },
    fetch: async () => new globalThis.Response('{}', { status: 200 }),
    __TAURI_INTERNALS__: {
      transformCallback: (callback: () => void) => {
        const id = callbacks.size + 1
        callbacks.set(id, callback)
        return id
      },
      unregisterCallback: (id: number) => callbacks.delete(id),
      invoke: async (command: string, args?: any) => {
        if (command === 'plugin:event|listen') {
          events.add(callbacks.get(args.handler)!)
          return args.handler
        }
        if (command === 'plugin:event|unlisten') {
          events.delete(callbacks.get(args.eventId)!)
          return
        }
        if (command === 'preference_read') return transport.request('read')
        if (command === 'preference_commit') {
          const snapshot = await transport.request('commit', { input: args.input })
          for (const callback of events) callback()
          return snapshot
        }
        throw new Error(`Unexpected command: ${command}`)
      }
    }
  })
  dom.window.eval(script)
  return { window: dom.window as unknown as globalThis.Window & typeof globalThis, transport }
}

describe.runIf(Boolean(executable))('Production bridge with actual native ACL storage', () => {
  it('invalidates the real renderer cache and refuses a new Harness capture after native corruption', async () => {
    const first = await bridge()
    vi.stubGlobal('window', first.window)
    vi.resetModules()
    const { preferenceService } = await import('@data/PreferenceService')
    const { withHarnessComponents } = await import('@renderer/services/harnessAssemble')
    await preferenceService.set(key, 'host', { optimistic: false })
    expect(await withHarnessComponents({})).toEqual({
      harnessComponents: ['system', 'tools', 'host:intent_stamp', 'budget', 'compact', 'infer']
    })
    writeFileSync(join(root, `${member}.acl`), '{broken')
    await expect(first.window.api.preference.get(key)).rejects.toMatchObject({ code: 'invalid' })
    expect(preferenceService.getCachedValue(key)).toBeUndefined()
    await expect(withHarnessComponents({})).rejects.toThrow('Harness preference is unavailable')
    preferenceService.cleanup()
  })
  it('converges two windows after concurrent different-key writes and a restarted renderer', async () => {
    const first = await bridge()
    const second = await bridge()
    await Promise.all([first.window.api.preference.getAll(), second.window.api.preference.getAll()])
    const observed: unknown[] = []
    second.window.api.preference.onChanged((name, value) => {
      if (name === key) observed.push(value)
    })
    await Promise.all([
      first.window.api.preference.set(key, 'host'),
      second.window.api.preference.set('app.language', 'en-US')
    ])
    await expect.poll(() => second.window.api.preference.get(key)).toBe('host')
    expect(observed).toContain('host')
    const restarted = await bridge()
    expect(await restarted.window.api.preference.getAll()).toEqual({
      [key]: 'host',
      'app.language': 'en-US',
      'app.onboarding.provider_setup.status': 'skipped',
      'app.privacy.policy_version': '20260820'
    })
    expect(readFileSync(join(root, `${member}.acl`), 'utf8')).toContain('preference "app.harness.assemble"')
  })

  it('keeps members separate and refuses a stale member commit', async () => {
    const first = await bridge()
    await first.window.api.preference.set(key, 'host')
    const second = await bridge(other)
    expect(await second.window.api.preference.get(key)).toBe('default')
    const old = await first.transport.request('read')
    await first.transport.request('member', { member: other })
    await expect(
      first.transport.request('commit', {
        input: { scope: old.scope, expectedRevision: old.revision, updates: { [key]: 'lean' } }
      })
    ).rejects.toMatchObject({ code: 'denied' })
    expect(await first.window.api.preference.get(key)).toBe('default')
  })

  it('clears the renderer cache and reloads the native scope when the signed-in member changes', async () => {
    const first = await bridge()
    vi.stubGlobal('window', first.window)
    await first.window.api.preference.set(key, 'host')

    vi.resetModules()
    const { publishMemberProfile } = await import('@renderer/components/account/memberProfile')
    publishMemberProfile({ id: member, displayName: 'Member A', phone: '' })
    await first.transport.request('member', { member: other })
    publishMemberProfile({ id: other, displayName: 'Member B', phone: '' })

    await expect.poll(() => first.window.api.preference.get(key)).toBe('default')
  })

  it('uses the schema default when the member snapshot has no recipe and keeps the unowned legacy choice', async () => {
    const first = await bridge(member, 'lean')
    expect(await first.window.api.preference.get(key)).toBe('default')
    expect(first.window.localStorage.getItem('a3s.studio.harness-assemble.v1')).toBe('lean')
    await expect(first.window.api.preference.set(key, 'unknown')).rejects.toMatchObject({
      code: 'invalid_harness_recipe'
    })
    expect(await first.window.api.preference.get(key)).toBe('default')
    await first.window.api.preference.set(key, 'host')
    expect(await first.window.api.preference.get(key)).toBe('host')
    expect(first.window.localStorage.getItem('a3s.studio.harness-assemble.v1')).toBe('lean')
  })

  it('refuses a corrupt ACL without activating defaults or overwriting the file', async () => {
    const first = await bridge()
    await first.window.api.preference.set(key, 'lean')
    const path = join(root, `${member}.acl`)
    writeFileSync(path, '{broken')
    const observed: unknown[] = []
    first.window.api.preference.onChanged((name, value) => {
      if (name === key) observed.push(value)
    })
    await expect(first.window.api.preference.get(key)).rejects.toMatchObject({ code: 'invalid' })
    await expect(first.window.api.preference.set(key, 'host')).rejects.toMatchObject({ code: 'invalid' })
    expect(observed).toContain(undefined)
    expect(readFileSync(path, 'utf8')).toBe('{broken')
  })
})
