import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'

import { beforeEach, describe, expect, it, vi } from 'vitest'

const bridgePath = resolve(__dirname, 'studioMainBridge.js')

interface Captured {
  url: string
  method: string
  body: Uint8Array | string | null
}

function installBridge(fetchImpl: (url: string, init?: RequestInit) => Promise<Response>) {
  delete (globalThis as { __A3S_CHERRY_MAIN__?: unknown }).__A3S_CHERRY_MAIN__
  delete (globalThis as { api?: unknown }).api
  ;(globalThis as { __A3S_STUDIO_RUNTIME__?: unknown }).__A3S_STUDIO_RUNTIME__ = {
    apiBaseUrl: 'http://studio.local',
    token: 'token',
    homePath: '/Users/roylin'
  }
  vi.stubGlobal(
    'fetch',
    vi.fn((input: RequestInfo | URL, init?: RequestInit) => fetchImpl(String(input), init))
  )
  const run = new Function(readFileSync(bridgePath, 'utf8'))
  run()
}

function jsonResponse(status: number, body: unknown) {
  return Promise.resolve(
    new Response(JSON.stringify(body), {
      status,
      headers: { 'content-type': 'application/json' }
    })
  )
}

describe('studio file bridge', () => {
  const captured: Captured[] = []

  it.each([null, {}, { entries: null }, { entries: {} }])(
    'rejects malformed workspace search results instead of reporting an empty directory: %j',
    async (body) => {
      installBridge(async () => jsonResponse(200, body))
      await expect(window.api.file.listDirectoryEntries('/work/alpha')).rejects.toThrow(
        'workspace search response invalid'
      )
    }
  )

  it('keeps the file input on document.body until the picker settles', async () => {
    installBridge(async () => jsonResponse(200, {}))
    const pending = window.api.file.select({ properties: ['openFile'] })
    const input = document.body.querySelector('input[type="file"]')
    expect(input).toBeInstanceOf(HTMLInputElement)
    input?.dispatchEvent(new Event('cancel'))
    await expect(pending).resolves.toBeNull()
    expect(document.body.querySelector('input[type="file"]')).toBeNull()
  })

  it('accepts an explicitly empty workspace search result', async () => {
    installBridge(async () => jsonResponse(200, { entries: [] }))
    await expect(window.api.file.listDirectoryEntries('/work/alpha')).resolves.toEqual([])
  })

  beforeEach(() => {
    captured.length = 0
    document.head.innerHTML = '<meta name="logger-window-source" content="mainWindow" />'
    document.body.innerHTML = '<div id="root"></div>'
    installBridge(async (url, init) => {
      const method = init?.method || 'GET'
      let body: Captured['body'] = null
      const raw = init?.body
      if (raw instanceof Uint8Array) body = raw
      else if (ArrayBuffer.isView(raw)) body = new Uint8Array(raw.buffer, raw.byteOffset, raw.byteLength)
      else if (raw instanceof ArrayBuffer) body = new Uint8Array(raw)
      else if (typeof raw === 'string') body = raw
      else if (raw instanceof Blob) body = new Uint8Array(await raw.arrayBuffer())
      else if (raw) body = new TextEncoder().encode(`${raw.constructor?.name || typeof raw}`)
      captured.push({ url, method, body })
      if (url.includes('/api/desktop-settings')) return jsonResponse(200, { ok: true })
      if (url.includes('/api/workspace-folder')) {
        const request = JSON.parse(String(init?.body || '{}')) as { title?: string }
        if (request.title?.startsWith('cancel')) return jsonResponse(200, { path: '' })
        return jsonResponse(200, { path: '/tmp/out' })
      }
      if (url.includes('/api/local-documents/entries') && method === 'GET') {
        const root = new URL(url).searchParams.get('root')
        if (root === '/tmp/out') {
          return jsonResponse(200, { entries: [{ name: 'taken.md', kind: 'file' }] })
        }
        if (root === '/Users/roylin/Documents') return jsonResponse(200, { entries: [] })
        if (root === '/Users/roylin/Documents/ToWork') return jsonResponse(404, { error: 'not_found' })
        if (root === '/Users/roylin/vault') {
          return jsonResponse(200, {
            entries: [
              { name: 'notes', kind: 'directory' },
              { name: 'today.md', kind: 'file' }
            ]
          })
        }
        if (root === '/Users/roylin/vault/notes') return jsonResponse(200, { entries: [] })
        return jsonResponse(200, { entries: [] })
      }
      if (url.includes('/api/local-documents/entries') && method === 'POST') {
        const request = JSON.parse(String(init?.body || '{}')) as { name?: string; root?: string }
        return jsonResponse(200, {
          name: request.name,
          path: `${request.root}/${request.name}`,
          kind: 'file'
        })
      }
      if (url.includes('/api/workspace-file/import') && method === 'POST') {
        const params = new URL(url).searchParams
        return jsonResponse(200, { path: `${params.get('directory')}/${params.get('name')}` })
      }
      if (url.includes('/api/workspace-file/raw') && method === 'PUT') return jsonResponse(200, { ok: true })
      if (url.includes('/api/workspace-file/text')) {
        return jsonResponse(200, {
          text: JSON.stringify({
            vaults: { one: { path: '/Users/roylin/vault', name: 'Vault' } }
          })
        })
      }
      if (url.includes('/api/workspace-entries')) return jsonResponse(400, { error: 'bad_request' })
      if (url.includes('/api/open-external') && method === 'POST') return jsonResponse(200, { ok: true })
      return jsonResponse(404, { error: 'not_found' })
    })
  })

  it('saves a named file in the chosen folder and skips a cancel', async () => {
    const saved = await globalThis.api.file.save('202601010000.md', 'hello')

    expect(saved).toBe('/tmp/out/202601010000.md')
    const written = captured.find((call) => call.url.includes('/api/workspace-file/import'))
    expect(new URL(written!.url).searchParams.get('name')).toBe('202601010000.md')
    expect(new URL(written!.url).searchParams.get('replace')).toBe('false')
    expect(new TextDecoder().decode(written?.body as Uint8Array)).toBe('hello')
    await expect(globalThis.api.file.save('cancel.md', 'nope')).resolves.toBeNull()
  })

  it('adds a suffix when the chosen folder already has that file name', async () => {
    const saved = await globalThis.api.file.save('taken.md', 'next')

    expect(saved).toBe('/tmp/out/taken (2).md')
  })

  it('preserves a file created after the save dialog listing instead of overwriting it', async () => {
    let winner = new Uint8Array([1, 2, 255])
    installBridge(async (url, init) => {
      if (url.includes('/api/workspace-folder')) return jsonResponse(200, { path: '/tmp/out' })
      if (url.includes('/api/local-documents/entries') && init?.method !== 'POST')
        return jsonResponse(200, { entries: [] })
      if (url.includes('/api/local-documents/entries') || url.includes('/api/workspace-file/import'))
        return jsonResponse(409, { error: 'already_exists' })
      if (init?.method === 'PUT') {
        winner = new Uint8Array(init.body as Uint8Array)
        return jsonResponse(200, { ok: true })
      }
      return jsonResponse(404, { error: 'not_found' })
    })
    await expect(globalThis.api.file.save('race.bin', new Uint8Array([8, 9]))).rejects.toThrow()
    expect(winner).toEqual(new Uint8Array([1, 2, 255]))
  })

  it('rejects an unavailable directory and treats an empty path as cancel', async () => {
    installBridge(async (url) => {
      if (url.includes('/api/workspace-folder')) {
        return jsonResponse(200, { path: null, reason: 'desktop-capability-unavailable' })
      }
      return jsonResponse(404, { error: 'not_found' })
    })
    await expect(globalThis.api.file.selectFolder()).rejects.toThrow('desktop-capability-unavailable')

    installBridge(async (url) => {
      if (url.includes('/api/workspace-folder')) return jsonResponse(200, { path: '' })
      return jsonResponse(404, { error: 'not_found' })
    })
    await expect(globalThis.api.file.selectFolder()).resolves.toBeNull()
  })

  it('writes a png for saveImage and returns false when the panel is dismissed', async () => {
    const saved = await globalThis.api.file.saveImage('Shot', 'data:image/png;base64,aGk=')

    expect(saved).toBe(true)
    const written = captured.find(
      (call) => call.url.includes('/api/workspace-file/import') && call.url.includes('Shot.png')
    )
    expect(new TextDecoder().decode(written?.body as Uint8Array)).toBe('hi')
    await expect(globalThis.api.file.saveImage('cancel.md', 'data:image/png;base64,aGk=')).resolves.toBe(false)
  })

  it('writes notes outside the session tree when the workspace rejects the path', async () => {
    await globalThis.api.file.write('/Users/roylin/Documents/ToWork/note.md', '正文')

    const written = captured.find((call) => call.method === 'PUT')
    expect(new TextDecoder().decode(written?.body as Uint8Array)).toBe('正文')
    await expect(globalThis.api.file.checkFileName('/Users/roylin/Documents/ToWork', '会议', true)).resolves.toEqual({
      safeName: '会议',
      exists: false
    })
  })

  it('saves a Word document and lists Obsidian vault files', async () => {
    const saved = await globalThis.api.ipcApi.request('export.word.from_markdown', {
      markdown: '你好',
      fileName: 'Report/Q1'
    })

    expect(saved).toEqual({ ok: true, data: true })
    const written = captured.find(
      (call) => call.url.includes('/api/workspace-file/import') && call.url.includes('.docx')
    )
    const bytes = written?.body as Uint8Array
    expect(bytes[0]).toBe(0x50)
    expect(bytes[1]).toBe(0x4b)
    expect(new TextDecoder().decode(bytes)).toContain('你好')

    await expect(globalThis.api.ipcApi.request('export.obsidian.get_vaults')).resolves.toEqual({
      ok: true,
      data: [{ path: '/Users/roylin/vault', name: 'Vault' }]
    })
    await expect(globalThis.api.ipcApi.request('export.obsidian.get_files', { vaultName: 'Vault' })).resolves.toEqual({
      ok: true,
      data: [
        { path: 'notes', type: 'folder', name: 'notes' },
        { path: 'today.md', type: 'markdown', name: 'today.md' }
      ]
    })
  })

  it('asks the shell to open a published notice and refuses another scheme', async () => {
    const opened = await globalThis.api.ipcApi.request(
      'system.shell.open_external_website',
      'http://127.0.0.1:3000/v1/api/legal/terms'
    )

    expect(opened).toEqual({ ok: true, data: undefined })
    const call = captured.find((item) => item.url.includes('/api/open-external'))
    expect(call?.method).toBe('POST')
    expect(JSON.parse(String(call?.body))).toEqual({ url: 'http://127.0.0.1:3000/v1/api/legal/terms' })

    const before = captured.length
    const refused = await globalThis.api.ipcApi.request('system.shell.open_external_website', 'file:///etc/passwd')
    expect(refused).toEqual({ ok: false, error: { code: 'INTERNAL', message: 'unsupported url' } })
    expect(captured).toHaveLength(before)

    await expect(
      globalThis.api.shell.openExternal('https://docs.example.com/v1/api/legal/privacy')
    ).resolves.toBeUndefined()
    const secure = captured.filter((item) => item.url.includes('/api/open-external'))
    expect(JSON.parse(String(secure.at(-1)?.body))).toEqual({
      url: 'https://docs.example.com/v1/api/legal/privacy'
    })
  })

  it('drives the focused window through the Tauri window plugin', async () => {
    const invoke = vi.fn(async (command: string) => (command.endsWith('is_fullscreen') ? false : null))
    ;(globalThis as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = { invoke }

    await expect(globalThis.api.ipcApi.request('window.close')).resolves.toEqual({ ok: true, data: null })
    expect(invoke).toHaveBeenCalledWith('plugin:window|close')

    await expect(globalThis.api.ipcApi.request('window.is_full_screen')).resolves.toEqual({ ok: true, data: false })
    await expect(globalThis.api.ipcApi.request('window.set_full_screen', false)).resolves.toEqual({
      ok: true,
      data: null
    })
    expect(invoke).toHaveBeenCalledWith('plugin:window|set_fullscreen', { value: false })

    await expect(globalThis.api.ipcApi.request('window.set_zoom', { factor: 1.24 })).resolves.toEqual({
      ok: true,
      data: null
    })
    expect(invoke).toHaveBeenCalledWith('plugin:webview|set_webview_zoom', { value: 1.2 })

    delete (globalThis as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__
    await expect(globalThis.api.ipcApi.request('window.minimize')).resolves.toEqual({
      ok: false,
      error: { code: 'INTERNAL', message: 'window host missing' }
    })
  })

  it('opens the screen recording pane through the shell command', async () => {
    const invoke = vi.fn(async () => null)
    ;(globalThis as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = { invoke }

    await expect(globalThis.api.ipcApi.request('system.mac.open_screen_capture_settings')).resolves.toEqual({
      ok: true,
      data: null
    })
    expect(invoke).toHaveBeenCalledWith('open_screen_capture_settings')

    delete (globalThis as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__
    await expect(globalThis.api.ipcApi.request('system.mac.open_screen_capture_settings')).resolves.toEqual({
      ok: false,
      error: { code: 'INTERNAL', message: 'screen capture settings host missing' }
    })
  })

  it('sends saved install settings to the shell command', async () => {
    const values = {
      'feature.binary.install_settings': {
        githubMirror: 'https://ghfast.top',
        githubToken: 'ghp_exampletoken',
        npmRegistry: '',
        pipIndexUrl: '',
        verifySignatures: true
      }
    }
    const snapshot = { scope: '11111111-1111-4111-8111-111111111111', revision: 0, values }
    const invoke = vi.fn(async (command: string, args?: { input?: { updates: typeof values } }) => {
      if (command === 'preference_read') return snapshot
      if (command === 'preference_commit') {
        Object.assign(values, args?.input?.updates)
        snapshot.revision++
        return snapshot
      }
      return null
    })
    ;(globalThis as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {
      invoke,
      transformCallback: () => 1
    }
    ;(globalThis as { __A3S_PREFERENCE_DEFAULTS__?: unknown }).__A3S_PREFERENCE_DEFAULTS__ = {
      'feature.binary.install_settings': {}
    }
    try {
      installBridge(async () => jsonResponse(200, { ok: true }))
      await globalThis.api.preference.get('feature.binary.install_settings')
      expect(invoke).toHaveBeenCalledWith('binary_ipc', {
        route: 'binary.apply_install_settings',
        input: {
          githubMirror: 'https://ghfast.top',
          githubToken: 'ghp_exampletoken',
          npmRegistry: '',
          pipIndexUrl: '',
          verifySignatures: true
        }
      })
      invoke.mockClear()
      await globalThis.api.preference.set('feature.binary.install_settings', {
        githubMirror: 'https://ghproxy.net',
        githubToken: '',
        npmRegistry: 'https://registry.npmmirror.com',
        pipIndexUrl: 'https://pypi.tuna.tsinghua.edu.cn/simple',
        verifySignatures: true
      })
      expect(invoke).toHaveBeenCalledWith('binary_ipc', {
        route: 'binary.apply_install_settings',
        input: {
          githubMirror: 'https://ghproxy.net',
          githubToken: '',
          npmRegistry: 'https://registry.npmmirror.com',
          pipIndexUrl: 'https://pypi.tuna.tsinghua.edu.cn/simple',
          verifySignatures: true
        }
      })
    } finally {
      delete (globalThis as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__
      delete (globalThis as { __A3S_PREFERENCE_DEFAULTS__?: unknown }).__A3S_PREFERENCE_DEFAULTS__
    }
  })

  it('sends a binary install to the shell command', async () => {
    const invoke = vi.fn(async () => null)
    ;(globalThis as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = { invoke }
    await expect(globalThis.api.ipcApi.request('binary.install_tool', { name: 'uv' })).resolves.toEqual({
      ok: true,
      data: null
    })
    expect(invoke).toHaveBeenCalledWith('binary_ipc', { route: 'binary.install_tool', input: { name: 'uv' } })
    delete (globalThis as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__
  })

  it('asks the shell to check, install, and read release notes', async () => {
    const invoke = vi.fn(async (command: string) =>
      command === 'app_updater_notes' ? [{ version: '2.1.4', releaseNotes: 'n' }] : null
    )
    ;(globalThis as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = { invoke }

    await expect(globalThis.api.ipcApi.request('app.updater.check_for_update')).resolves.toEqual({
      ok: true,
      data: null
    })
    expect(invoke).toHaveBeenCalledWith('app_updater_check')

    await expect(globalThis.api.ipcApi.request('app.updater.quit_and_install')).resolves.toEqual({
      ok: true,
      data: null
    })
    expect(invoke).toHaveBeenCalledWith('app_updater_install')

    await expect(globalThis.api.ipcApi.request('app.updater.release_notes.get')).resolves.toEqual({
      ok: true,
      data: [{ version: '2.1.4', releaseNotes: 'n' }]
    })

    const heard: unknown[] = []
    const stop = globalThis.api.ipcApi.on('app.updater.download_progress', (payload) => {
      heard.push(payload)
    })
    ;(globalThis as { __A3S_UPDATER_EMIT__?: (event: string, payload: unknown) => void }).__A3S_UPDATER_EMIT__(
      'app.updater.download_progress',
      { percent: 40 }
    )
    stop()
    ;(globalThis as { __A3S_UPDATER_EMIT__?: (event: string, payload: unknown) => void }).__A3S_UPDATER_EMIT__(
      'app.updater.download_progress',
      { percent: 90 }
    )
    expect(heard).toEqual([{ percent: 40 }])

    delete (globalThis as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__
    await expect(globalThis.api.ipcApi.request('app.updater.check_for_update')).resolves.toEqual({
      ok: false,
      error: { code: 'INTERNAL', message: 'updater host missing' }
    })
  })

  it('sends worktree list, restore, remove, create, and clear to the shell command', async () => {
    const listed = {
      items: [
        { id: '/repo', path: '/repo', dirty: false, primary: true, mtimeMs: 1 },
        { id: '/repo-feature', path: '/repo-feature', dirty: false, primary: false, mtimeMs: 2 }
      ]
    }
    const invoke = vi.fn(async () => listed)
    ;(globalThis as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = { invoke }
    const restore = { cwd: '/repo-feature', action: 'restore', paths: ['src/a.ts'] }
    await expect(globalThis.api.ipcApi.request('app.git.worktrees', { cwd: '/repo', action: 'list' })).resolves.toEqual({
      ok: true,
      data: listed
    })
    await expect(globalThis.api.ipcApi.request('app.git.worktrees', restore)).resolves.toEqual({
      ok: true,
      data: listed
    })
    await expect(
      globalThis.api.ipcApi.request('app.git.worktrees', { cwd: '/repo', action: 'remove', path: '/repo-feature' })
    ).resolves.toEqual({ ok: true, data: listed })
    expect(invoke).toHaveBeenNthCalledWith(1, 'git_worktrees', { input: { cwd: '/repo', action: 'list' } })
    expect(invoke).toHaveBeenNthCalledWith(2, 'git_worktrees', { input: restore })
    expect(invoke).toHaveBeenNthCalledWith(3, 'git_worktrees', {
      input: { cwd: '/repo', action: 'remove', path: '/repo-feature' }
    })
    const create = { cwd: '/repo', action: 'create', branch: 'feature' }
    const clear = { cwd: '/repo', action: 'clear', sinceMs: 20 }
    await expect(globalThis.api.ipcApi.request('app.git.worktrees', create)).resolves.toEqual({
      ok: true,
      data: listed
    })
    await expect(globalThis.api.ipcApi.request('app.git.worktrees', clear)).resolves.toEqual({
      ok: true,
      data: listed
    })
    expect(invoke).toHaveBeenNthCalledWith(4, 'git_worktrees', { input: create })
    expect(invoke).toHaveBeenNthCalledWith(5, 'git_worktrees', { input: clear })
    delete (globalThis as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__
    await expect(globalThis.api.ipcApi.request('app.git.worktrees', { cwd: '/repo', action: 'list' })).resolves.toEqual({
      ok: false,
      error: { code: 'INTERNAL', message: 'worktree host missing' }
    })
  })

  it('returns a country code and a directory snapshot for the task pane', async () => {
    await expect(globalThis.api.ipcApi.request('system.get_ip_country')).resolves.toEqual({ ok: true, data: 'CN' })

    const created = await globalThis.api.ipcApi.request('file.tree.create', { rootPath: '/Users/roylin/vault' })
    expect(created.ok).toBe(true)
    expect(created.data.revision).toBe(0)
    expect(created.data.treeId).toBe('studio-tree-1')
    expect(created.data.snapshot).toEqual({
      kind: 'directory',
      path: '/Users/roylin/vault',
      basename: 'vault',
      children: {
        notes: {
          kind: 'directory',
          path: '/Users/roylin/vault/notes',
          basename: 'notes',
          children: {}
        },
        'today.md': { kind: 'file', path: '/Users/roylin/vault/today.md', basename: 'today.md' }
      }
    })

    const missing = await globalThis.api.ipcApi.request('file.tree.create', {
      rootPath: '/Users/roylin/Documents/ToWork'
    })
    expect(missing.data.snapshot.kind).toBe('directory')
    expect(missing.data.snapshot.children).toEqual({})
    expect(missing.data.treeId).toBe('studio-tree-2')

    await expect(
      globalThis.api.ipcApi.request('file.tree.activate', { treeId: created.data.treeId, revision: 0 })
    ).resolves.toEqual({
      ok: true,
      data: true
    })
    await expect(globalThis.api.ipcApi.request('file.tree.dispose', { treeId: created.data.treeId })).resolves.toEqual({
      ok: true,
      data: null
    })
  })

  it('reports a notes directory from app info', async () => {
    const info = await globalThis.api.ipcApi.request('app.get_info')

    expect(info.ok).toBe(true)
    expect(info.data.notesPath).toBe('/Users/roylin/Documents/ToWork')
  })

  it('stores an installed assistant and hides it after archive', async () => {
    localStorage.removeItem('a3s.workbench.assistants')
    const created = await globalThis.api.dataApi.request({
      id: 'create-assistant',
      method: 'POST',
      path: '/assistants',
      body: { name: 'ACP 云端智能体', prompt: '记住 sessionId', description: '云端案例', emoji: '云' }
    })
    expect(created.status).toBe(201)
    expect(created.data.name).toBe('ACP 云端智能体')
    expect(created.data.id).toEqual(expect.any(String))

    const listed = await globalThis.api.dataApi.request({ method: 'GET', path: '/assistants' })
    expect(listed.data.items.map((row: { name: string }) => row.name)).toEqual(['ACP 云端智能体'])

    const archived = await globalThis.api.ipcApi.request('trash.assistant.archive', {
      assistantId: created.data.id,
      deleteTopics: false
    })
    expect(archived).toEqual({ ok: true, data: { deleted: true } })
    const after = await globalThis.api.dataApi.request({ method: 'GET', path: '/assistants' })
    expect(after.data.items).toEqual([])
    localStorage.removeItem('a3s.workbench.assistants')
  })

  it('creates a workspace from a path when the name is omitted', async () => {
    localStorage.removeItem('a3s.workbench.projects')
    const created = await globalThis.api.dataApi.request({
      id: 'create-workspace',
      method: 'POST',
      path: '/agent-workspaces',
      body: { path: '/Users/roylin/Library/Application Support/dev.a3s.studio/toyou-fork2/compile-fixture' }
    })
    expect(created.status).toBe(201)
    expect(created.data).toMatchObject({
      name: 'compile-fixture',
      path: '/Users/roylin/Library/Application Support/dev.a3s.studio/toyou-fork2/compile-fixture',
      type: 'user'
    })

    const named = await globalThis.api.dataApi.request({
      id: 'create-named-workspace',
      method: 'POST',
      path: '/agent-workspaces',
      body: { path: '/Users/roylin/notes', name: '笔记库' }
    })
    expect(named.status).toBe(201)
    expect(named.data.name).toBe('笔记库')

    const again = await globalThis.api.dataApi.request({
      id: 'create-workspace-again',
      method: 'POST',
      path: '/agent-workspaces',
      body: { path: created.data.path }
    })
    expect(again.status).toBe(200)
    expect(again.data.id).toBe(created.data.id)

    const missing = await globalThis.api.dataApi.request({
      id: 'create-workspace-empty',
      method: 'POST',
      path: '/agent-workspaces',
      body: { path: '   ' }
    })
    expect(missing.status).toBe(400)
    expect(missing.error.message).toBe('project path is required')
    localStorage.removeItem('a3s.workbench.projects')
  })

  it('stores a channel on ToBot and does not sync a short runtime token', async () => {
    localStorage.removeItem('a3s.agent.channels')
    const created = await globalThis.api.ipcApi.request('channel.create', {
      type: 'telegram',
      name: 'Telegram',
      config: { bot_token: '', allowed_chat_ids: [] },
      isActive: false
    })

    expect(created.ok).toBe(true)
    expect(created.data.agentId).toBe('tobot')
    expect(created.data.workspace).toEqual({ type: 'system' })
    const listed = await globalThis.api.dataApi.request({
      method: 'GET',
      path: '/agent-channels',
      params: { type: 'telegram' }
    })
    expect(listed.data).toEqual([created.data])
    expect(captured.some((call) => call.url.includes('/api/agent-channels/sync'))).toBe(false)

    await globalThis.api.ipcApi.request('channel.delete', { channelId: created.data.id })
    const after = await globalThis.api.dataApi.request({ method: 'GET', path: '/agent-channels' })
    expect(after.data).toEqual([])
    localStorage.removeItem('a3s.agent.channels')
  })

  it('reads a session workspace that is not in the local project list', async () => {
    installBridge(async (url) => {
      const parsed = new URL(url)
      if (parsed.pathname.endsWith('/api/workspace-file')) {
        expect(parsed.searchParams.get('root')).toBe('/tmp/quarter')
        expect(parsed.searchParams.get('path')).toBe('/tmp/quarter')
        return jsonResponse(200, { kind: 'directory', size: 0, createdAt: 1, modifiedAt: 2 })
      }
      return jsonResponse(404, { error: 'not_found' })
    })

    const metadata = await globalThis.api.ipcApi.request('file.get_metadata', {
      kind: 'path',
      path: '/tmp/quarter'
    })

    expect(metadata).toEqual({
      ok: true,
      data: { kind: 'directory', size: 0, createdAt: 1, modifiedAt: 2 }
    })
  })

  it('does not turn an outside file into metadata when the host rejects it', async () => {
    installBridge(async (url) => {
      if (url.includes('/api/workspace-file')) return jsonResponse(400, { error: 'not_a_directory' })
      return jsonResponse(404, { error: 'not_found' })
    })

    const metadata = await globalThis.api.ipcApi.request('file.get_metadata', {
      kind: 'path',
      path: '/tmp/quarter/ops.csv'
    })

    expect(metadata).toEqual({ ok: true, data: null })
  })

  it('keeps an uploaded contract path so send can attach it', async () => {
    const path = '/tmp/a3s-studio-composer/be60411f48e54ecb_软件开发服务合同(含漏洞示例).docx'
    const entry = await globalThis.api.file.createInternalEntry({
      source: 'path',
      path,
      cleanupPolicy: 'delete_when_unreferenced'
    })
    expect(entry.id).toBe(path)
    await expect(globalThis.api.file.getPhysicalPath({ id: entry.id })).resolves.toBe(path)
    const metadata = await globalThis.api.ipcApi.request('file.get_metadata', { kind: 'path', path })
    expect(metadata.data).toEqual(
      expect.objectContaining({
        kind: 'file',
        mime: 'application/vnd.openxmlformats-officedocument.wordprocessingml.document'
      })
    )
    const batch = await globalThis.api.ipcApi.request('file.batch_get_metadata', {
      items: [{ key: path, handle: { kind: 'path', path } }]
    })
    expect(batch.data[path].kind).toBe('file')
    expect(captured.some((call) => call.url.includes('/api/workspace-file'))).toBe(false)
  })
})
