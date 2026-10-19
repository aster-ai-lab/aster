import { afterEach, describe, expect, it, vi } from 'vitest'

import { appletPackageCacheKey, loadAppletPackage, sha256Hex } from '../loadAppletPackage'

const runtime = {
  apiBaseUrl: 'http://127.0.0.1:9',
  appletTicket: 'ab'.repeat(32),
  appId: 'leave',
  tenant: 'toyou'
}

const packageBody = JSON.stringify({
  appId: 'leave',
  version: '1.0.0',
  entry: 'pages/index/index',
  pages: ['pages/index/index'],
  window: { navigationBarTitleText: '请假' }
})

const cacheKey = appletPackageCacheKey('toyou')

afterEach(() => {
  localStorage.removeItem(cacheKey)
  Reflect.deleteProperty(globalThis, '__A3S_APPLET_RUNTIME__')
  Reflect.deleteProperty(globalThis, '__A3S_STUDIO_RUNTIME__')
  vi.unstubAllGlobals()
  vi.useRealTimers()
})

function installRuntime() {
  Object.defineProperty(globalThis, '__A3S_APPLET_RUNTIME__', {
    value: runtime,
    configurable: true
  })
}

async function digestOf(body: string): Promise<string> {
  return sha256Hex(new TextEncoder().encode(body))
}

describe('loadAppletPackage', () => {
  it('downloads the release, checks the digest, and caches the bytes', async () => {
    installRuntime()
    const digest = await digestOf(packageBody)
    const fetchImpl = vi.fn(async (url: string, init?: RequestInit) => {
      expect(init?.headers).toMatchObject({ authorization: `Bearer ${runtime.appletTicket}` })
      if (String(url).endsWith('/api/applet-runtime/leave')) {
        return new Response(JSON.stringify({ release: { version: '1.0.0', digest }, trial: null }), { status: 200 })
      }
      if (String(url).endsWith('/package/release')) {
        return new Response(packageBody, { status: 200 })
      }
      return new Response('{}', { status: 404 })
    })

    const pinned = await loadAppletPackage('leave', fetchImpl)
    expect(pinned).toMatchObject({
      appId: 'leave',
      version: '1.0.0',
      digest,
      source: 'network',
      body: packageBody,
      channel: 'release'
    })
    await expect(pinned?.updateReady).resolves.toBe(false)
    const cached = JSON.parse(localStorage.getItem(cacheKey) || '{}') as {
      leave: { release: { digest: string; body: string } }
    }
    expect(cached.leave.release.digest).toBe(digest)
    expect(cached.leave.release.body).toBe(packageBody)
    expect(packageBody).not.toContain('token')
  })

  it('opens the cached release and keeps that pin when a newer build arrives', async () => {
    installRuntime()
    const digest = await digestOf(packageBody)
    localStorage.setItem(
      cacheKey,
      JSON.stringify({ leave: { release: { version: '1.0.0', digest, body: packageBody } } })
    )
    const nextBody = packageBody.replace('1.0.0', '1.1.0')
    const nextDigest = await digestOf(nextBody)
    let releaseMeta: () => void = () => {}
    const gate = new Promise<void>((resolve) => {
      releaseMeta = resolve
    })
    const fetchImpl = vi.fn((url: string) => {
      if (String(url).endsWith('/package/release')) {
        return Promise.resolve(new Response(nextBody, { status: 200 }))
      }
      return gate.then(
        () =>
          new Response(JSON.stringify({ release: { version: '1.1.0', digest: nextDigest }, trial: null }), {
            status: 200
          })
      )
    })

    const pinned = await loadAppletPackage('leave', fetchImpl)
    expect(pinned).toMatchObject({ version: '1.0.0', digest, source: 'cache' })
    expect(fetchImpl).not.toHaveBeenCalledWith(expect.stringContaining('/package/release'), expect.anything())

    releaseMeta()
    await vi.waitFor(() => {
      const cached = JSON.parse(localStorage.getItem(cacheKey) || '{}') as {
        leave: { release: { version: string } }
      }
      expect(cached.leave.release.version).toBe('1.1.0')
    })
    expect(pinned?.version).toBe('1.0.0')
    await expect(pinned?.updateReady).resolves.toBe(true)
  })

  it('returns no package when the release does not arrive within one second', async () => {
    installRuntime()
    vi.useFakeTimers()
    const fetchImpl = vi.fn(() => new Promise<Response>(() => {}))
    const pending = loadAppletPackage('leave', fetchImpl)
    await vi.advanceTimersByTimeAsync(999)
    let settled = false
    void pending.then(() => {
      settled = true
    })
    await Promise.resolve()
    expect(settled).toBe(false)
    await vi.advanceTimersByTimeAsync(1)
    await expect(pending).resolves.toBeNull()
  })

  it('returns no package when the trial channel is not published', async () => {
    installRuntime()
    const digest = await digestOf(packageBody)
    const fetchImpl = vi.fn(async (url: string) => {
      if (String(url).endsWith('/package/trial')) return new Response(packageBody, { status: 200 })
      return new Response(JSON.stringify({ release: { version: '1.0.0', digest }, trial: null }), { status: 200 })
    })
    await expect(loadAppletPackage('leave', fetchImpl, 1000, localStorage, 'trial')).resolves.toBeNull()
    expect(fetchImpl).not.toHaveBeenCalledWith(expect.stringContaining('/package/trial'), expect.anything())
  })

  it('does not call the shell when the runtime descriptor is missing', async () => {
    const fetchImpl = vi.fn()
    await expect(loadAppletPackage('leave', fetchImpl)).resolves.toBeNull()
    expect(fetchImpl).not.toHaveBeenCalled()
  })

  it('does not use the shell bearer', async () => {
    Object.defineProperty(globalThis, '__A3S_STUDIO_RUNTIME__', {
      value: { apiBaseUrl: 'http://127.0.0.1:9', token: 'cd'.repeat(32) },
      configurable: true
    })
    const fetchImpl = vi.fn()
    await expect(loadAppletPackage('leave', fetchImpl)).resolves.toBeNull()
    expect(fetchImpl).not.toHaveBeenCalled()
  })
})
