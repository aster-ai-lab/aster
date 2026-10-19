// Runs before the Cherry renderer modules. The Tauri window has no Electron
// preload, so this installs the slice of window.api the main window reads
// on its first frame. Onboarding is already finished: login replaced it.
if (globalThis.top === globalThis && !globalThis.__A3S_CHERRY_MAIN__) {
  globalThis.__A3S_CHERRY_MAIN__ = true
  const desktop = globalThis.__A3S_STUDIO_DESKTOP__
  const overlay = Boolean(desktop && typeof desktop === 'object' && desktop.chrome === 'overlay')
  if (overlay && globalThis.document.documentElement) {
    const style = globalThis.document.documentElement.style
    if (!style.getPropertyValue('--a3s-traffic-light-inset')) {
      style.setProperty('--a3s-traffic-light-inset', '60px')
    }
    if (!style.getPropertyValue('--a3s-titlebar-height')) {
      style.setProperty('--a3s-titlebar-height', '44px')
    }
  }
  const seeded = globalThis.__A3S_PREFERENCE_DEFAULTS__
  const prefs = seeded && typeof seeded === 'object' ? { ...seeded } : {}
  const nativeInternals = globalThis.__TAURI_INTERNALS__
  const hasNativePreferences = typeof nativeInternals?.invoke === 'function'
  const workspacePreference = 'app.workspace.path'
  const workspaceStorage = 'a3s.workspace.path'
  try {
    const savedWorkspace = globalThis.localStorage.getItem(workspaceStorage)
    if (!hasNativePreferences && typeof savedWorkspace === 'string') prefs[workspacePreference] = savedWorkspace
  } catch {
    // The page can still show the default directory for this session.
  }
  const rememberWorkspace = (key, value) => {
    if (key !== workspacePreference) return
    try {
      globalThis.localStorage.setItem(workspaceStorage, String(value ?? ''))
    } catch {
      // The in-memory value still applies until the page reloads.
    }
  }
  const settingsStorage = 'a3s.settings.preferences'
  const persistedKeys = [
    'app.launch_on_boot',
    'app.tray.enabled',
    'app.tray.on_close',
    'app.tray.on_launch',
    'feature.quick_assistant.click_tray_to_show',
    'app.power.prevent_sleep_when_busy',
    'chat.context_settings.enabled',
    'chat.context_settings.max_messages',
    'chat.context_settings.truncate_threshold',
    'chat.context_settings.compress.enabled',
    'chat.context_settings.compress.model_id',
    'chat.context_settings.compress.threshold_percent',
    'chat.default_model_id',
    'feature.quick_assistant.model_id',
    'feature.translate.model_id',
    'ui.theme_mode',
    'app.language',
    'app.harness.assemble',
    'feature.binary.install_settings'
  ]
  const installSettingsKey = 'feature.binary.install_settings'
  const desktopKeys = [
    'app.launch_on_boot',
    'app.tray.enabled',
    'app.tray.on_close',
    'app.tray.on_launch',
    'app.power.prevent_sleep_when_busy',
    'app.language'
  ]
  const savedSettings = {}
  let settingsReadable = true
  try {
    const raw = globalThis.localStorage.getItem(settingsStorage)
    const parsed = raw ? JSON.parse(raw) : null
    if (parsed !== null && (typeof parsed !== 'object' || Array.isArray(parsed))) {
      throw new Error('Invalid preference store')
    }
    if (parsed) {
      for (const key of persistedKeys) {
        if (Object.prototype.hasOwnProperty.call(parsed, key)) savedSettings[key] = parsed[key]
      }
    }
  } catch {
    settingsReadable = false
    // Defaults stay in place for this session.
  }
  for (const key of persistedKeys) {
    if (!hasNativePreferences && Object.prototype.hasOwnProperty.call(savedSettings, key))
      prefs[key] = savedSettings[key]
  }
  const harnessKey = 'app.harness.assemble'
  const legacyHarnessKey = 'a3s.studio.harness-assemble.v1'
  const harnessIds = ['default', 'lean', 'host']
  try {
    if (hasNativePreferences) throw new Error('Native preferences require an owned snapshot')
    if (!settingsReadable) throw new Error('Harness preference store is unavailable')
    const legacy = globalThis.localStorage.getItem(legacyHarnessKey)
    if (!Object.prototype.hasOwnProperty.call(savedSettings, harnessKey) && harnessIds.includes(legacy)) {
      const migrated = { ...savedSettings, [harnessKey]: legacy }
      globalThis.localStorage.setItem(settingsStorage, JSON.stringify(migrated))
      Object.assign(savedSettings, migrated)
      prefs[harnessKey] = legacy
    }
  } catch {
    // A failed migration keeps the old value for retry and refuses new runs.
    prefs[harnessKey] = undefined
  }
  if (!hasNativePreferences && prefs[harnessKey] !== undefined) {
    try {
      globalThis.localStorage.removeItem(legacyHarnessKey)
    } catch {
      // The persisted unified choice remains authoritative; cleanup can retry next boot.
    }
  }
  const rememberSettings = (updates) => {
    const entries = Object.entries(updates)
    for (const [key, value] of entries) rememberWorkspace(key, value)
    const persistent = entries.filter(([key]) => persistedKeys.includes(key))
    if (!persistent.length) return
    const raw = globalThis.localStorage.getItem(settingsStorage)
    const latest = raw ? JSON.parse(raw) : {}
    if (!latest || typeof latest !== 'object' || Array.isArray(latest)) throw new Error('Invalid preference store')
    const merged = Object.fromEntries(
      persistedKeys.filter((key) => Object.hasOwn(latest, key)).map((key) => [key, latest[key]])
    )
    for (const [key, value] of persistent) merged[key] = value
    globalThis.localStorage.setItem(settingsStorage, JSON.stringify(merged))
    Object.assign(savedSettings, merged)
  }
  globalThis.window.addEventListener('storage', (event) => {
    if (hasNativePreferences) return
    if (event.key !== settingsStorage && event.key !== null) return
    try {
      const raw = globalThis.localStorage.getItem(settingsStorage)
      const latest = raw ? JSON.parse(raw) : {}
      if (!latest || typeof latest !== 'object' || Array.isArray(latest)) return
      for (const key of persistedKeys) {
        const value = Object.hasOwn(latest, key) ? latest[key] : seeded?.[key]
        if (Object.is(prefs[key], value)) continue
        prefs[key] = value
        if (Object.hasOwn(latest, key)) savedSettings[key] = value
        else delete savedSettings[key]
        emit(key, value)
      }
    } catch {
      // Invalid external writes cannot change the active preference snapshot.
    }
  })
  const postInstallSettings = (value) => {
    const internals = globalThis.__TAURI_INTERNALS__
    if (!internals || typeof internals.invoke !== 'function') return
    const settings = value && typeof value === 'object' ? value : null
    if (!settings) return
    internals
      .invoke('binary_ipc', {
        route: 'binary.apply_install_settings',
        input: {
          githubMirror: typeof settings.githubMirror === 'string' ? settings.githubMirror : '',
          githubToken: typeof settings.githubToken === 'string' ? settings.githubToken : '',
          npmRegistry: typeof settings.npmRegistry === 'string' ? settings.npmRegistry : '',
          pipIndexUrl: typeof settings.pipIndexUrl === 'string' ? settings.pipIndexUrl : '',
          verifySignatures: settings.verifySignatures !== false
        }
      })
      .catch(() => {})
  }
  const postDesktopSettings = () => {
    if (
      !desktopKeys.some(
        (key) =>
          Object.prototype.hasOwnProperty.call(savedSettings, key) || Object.prototype.hasOwnProperty.call(prefs, key)
      )
    ) {
      return
    }
    const runtime = globalThis.__A3S_STUDIO_RUNTIME__
    if (!runtime || !runtime.apiBaseUrl || !runtime.token) return
    const language = prefs['app.language'] === 'en-US' ? 'en-US' : 'zh-CN'
    globalThis
      .fetch(`${runtime.apiBaseUrl}/api/desktop-settings`, {
        method: 'POST',
        headers: {
          authorization: `Bearer ${runtime.token}`,
          'content-type': 'application/json'
        },
        body: JSON.stringify({
          launchOnBoot: Boolean(prefs['app.launch_on_boot']),
          tray: Boolean(prefs['app.tray.enabled']),
          trayOnClose: Boolean(prefs['app.tray.on_close']),
          trayOnLaunch: Boolean(prefs['app.tray.on_launch']),
          preventSleepWhenBusy: Boolean(prefs['app.power.prevent_sleep_when_busy']),
          language
        })
      })
      .catch(() => {})
  }
  prefs['app.onboarding.provider_setup.status'] = 'skipped'
  prefs['app.privacy.policy_version'] = '20260820'
  if (prefs['app.language'] !== 'zh-CN' && prefs['app.language'] !== 'en-US') {
    prefs['app.language'] = 'zh-CN'
  }
  const listeners = new Set()
  const emit = (key, value) => {
    for (const listener of listeners) listener(key, value)
  }
  const nativeKeys = Object.keys(seeded || {}).filter((key) => !key.startsWith('BootConfig.'))
  let nativeSnapshot = null
  let nativeQueue = Promise.resolve()
  let nativeEventReady = null
  const enqueueNative = (action) => {
    const request = nativeQueue.then(action)
    nativeQueue = request.catch(() => {})
    return request
  }
  const invalidateNative = () => {
    nativeSnapshot = null
    for (const key of nativeKeys) {
      prefs[key] = undefined
      emit(key, undefined)
    }
  }
  const acceptNative = (snapshot) => {
    if (
      !snapshot ||
      typeof snapshot.scope !== 'string' ||
      !Number.isSafeInteger(snapshot.revision) ||
      snapshot.revision < 0 ||
      !snapshot.values ||
      typeof snapshot.values !== 'object' ||
      Array.isArray(snapshot.values)
    ) {
      throw new Error('Invalid native preference snapshot')
    }
    if (nativeSnapshot?.scope === snapshot.scope && nativeSnapshot.revision > snapshot.revision) return
    nativeSnapshot = snapshot
    let desktopChanged = false
    let installChanged = false
    for (const key of nativeKeys) {
      // A readable member snapshot that never stored this key uses the schema
      // default. An unscoped local choice stays in local storage until a new
      // choice is saved. A failed snapshot read still clears the key.
      const value = Object.hasOwn(snapshot.values, key) ? snapshot.values[key] : seeded[key]
      if (JSON.stringify(prefs[key]) === JSON.stringify(value)) continue
      prefs[key] = value
      emit(key, value)
      if (desktopKeys.includes(key)) desktopChanged = true
      if (key === installSettingsKey) installChanged = true
    }
    if (desktopChanged) postDesktopSettings()
    if (installChanged) postInstallSettings(prefs[installSettingsKey])
  }
  const readNative = async () => {
    try {
      acceptNative(
        await nativeInternals.invoke('preference_read', { hostToken: globalThis.__A3S_STUDIO_RUNTIME__?.token || '' })
      )
      return nativeSnapshot
    } catch (error) {
      invalidateNative()
      throw error
    }
  }
  globalThis.window.addEventListener('a3s:member-scope-changed', () => {
    if (!hasNativePreferences) return
    invalidateNative()
    void enqueueNative(readNative).catch(() => {})
  })
  const listenNative = () => {
    if (!nativeEventReady) {
      if (typeof nativeInternals.transformCallback !== 'function')
        return Promise.reject(new Error('Native preference events unavailable'))
      const callback = nativeInternals.transformCallback(() => {
        void enqueueNative(readNative).catch(() => {})
      })
      nativeEventReady = nativeInternals
        .invoke('plugin:event|listen', {
          event: 'studio-preferences-committed',
          target: { kind: 'WebviewWindow', label: 'main' },
          handler: callback
        })
        .catch((error) => {
          nativeInternals.unregisterCallback?.(callback)
          nativeEventReady = null
          throw error
        })
      nativeEventReady.then(
        (eventId) => {
          globalThis.window.addEventListener(
            'pagehide',
            () => {
              void nativeInternals
                .invoke('plugin:event|unlisten', { event: 'studio-preferences-committed', eventId })
                .catch(() => {})
              nativeInternals.unregisterCallback?.(callback)
            },
            { once: true }
          )
        },
        () => {}
      )
    }
    return nativeEventReady
  }
  const writeNative = (updates) =>
    enqueueNative(async () => {
      await listenNative()
      let snapshot = await readNative()
      const scope = snapshot.scope
      for (let attempt = 0; attempt < 3; attempt++) {
        try {
          snapshot = await nativeInternals.invoke('preference_commit', {
            hostToken: globalThis.__A3S_STUDIO_RUNTIME__?.token || '',
            input: {
              scope,
              expectedRevision: snapshot.revision,
              updates
            }
          })
          acceptNative(snapshot)
          if (desktopKeys.some((key) => Object.hasOwn(updates, key))) postDesktopSettings()
          if (Object.hasOwn(updates, installSettingsKey)) postInstallSettings(updates[installSettingsKey])
          return
        } catch (error) {
          if (error?.code !== 'conflict') throw error
          snapshot = error.snapshot
          if (snapshot?.scope !== scope) throw new Error('Preference member changed')
        }
      }
      throw new Error('Preference write conflict')
    })
  const nativePreferenceApi = hasNativePreferences
    ? {
        get: (key) =>
          enqueueNative(async () => {
            await listenNative()
            await readNative()
            return prefs[key]
          }),
        getMultipleRaw: (keys) =>
          enqueueNative(async () => {
            await listenNative()
            await readNative()
            return Object.fromEntries(keys.map((key) => [key, prefs[key]]))
          }),
        getAll: () =>
          enqueueNative(async () => {
            await listenNative()
            await readNative()
            return { ...prefs }
          }),
        set: (key, value) => writeNative({ [key]: value }),
        setMultiple: writeNative,
        subscribe: () => listenNative(),
        onChanged: (callback) => {
          listeners.add(callback)
          return () => listeners.delete(callback)
        }
      }
    : null
  const emptyCollection = () => {
    const rows = []
    rows.items = []
    rows.total = 0
    return rows
  }
  const bindRoot = () => {
    globalThis.root = globalThis.document.getElementById('root') || globalThis.document.documentElement
  }
  bindRoot()
  globalThis.document.addEventListener('DOMContentLoaded', bindRoot)
  const projectStorage = 'a3s.workbench.projects'
  const readProjects = () => {
    try {
      const raw = globalThis.localStorage.getItem(projectStorage)
      const parsed = raw ? JSON.parse(raw) : []
      if (!Array.isArray(parsed)) return []
      return parsed.filter(
        (row) => row && row.type === 'user' && typeof row.id === 'string' && typeof row.path === 'string'
      )
    } catch {
      return []
    }
  }
  const writeProjects = (rows) => {
    try {
      globalThis.localStorage.setItem(projectStorage, JSON.stringify(rows))
    } catch {
      // The folder still exists for this session.
    }
  }
  const channelStorage = 'a3s.agent.channels'
  const tobotAgentId = 'tobot'
  const boundAgent = (value) => (typeof value === 'string' && value.trim() ? value.trim() : tobotAgentId)
  const readChannels = () => {
    try {
      const raw = globalThis.localStorage.getItem(channelStorage)
      const parsed = raw ? JSON.parse(raw) : []
      return Array.isArray(parsed) ? parsed.filter((row) => row && typeof row.id === 'string') : []
    } catch {
      return []
    }
  }
  const syncChannels = (rows) => {
    const runtime = globalThis.__A3S_STUDIO_RUNTIME__
    const token = runtime && typeof runtime.token === 'string' ? runtime.token : ''
    if (!/^[a-f0-9]{64}$/.test(token) || !runtime.apiBaseUrl) return
    globalThis
      .fetch(`${runtime.apiBaseUrl}/api/agent-channels/sync`, {
        method: 'POST',
        headers: {
          authorization: `Bearer ${token}`,
          'content-type': 'application/json'
        },
        body: JSON.stringify({ channels: rows })
      })
      .catch(() => {})
  }
  const writeChannels = (rows) => {
    try {
      globalThis.localStorage.setItem(channelStorage, JSON.stringify(rows))
    } catch {
      // The list still applies until the page reloads.
    }
    syncChannels(rows)
  }
  const normalizeChannel = (row, nowIso) => ({
    id: String(row.id),
    type: typeof row.type === 'string' ? row.type : '',
    name: typeof row.name === 'string' && row.name.trim() ? row.name.trim() : 'Channel',
    agentId: boundAgent(row.agentId),
    workspace: row.workspace && typeof row.workspace === 'object' ? row.workspace : { type: 'system' },
    config: row.config && typeof row.config === 'object' && !Array.isArray(row.config) ? row.config : {},
    isActive: row.isActive === true,
    permissionMode: Object.prototype.hasOwnProperty.call(row, 'permissionMode') ? row.permissionMode : null,
    createdAt: typeof row.createdAt === 'string' ? row.createdAt : nowIso,
    updatedAt: nowIso
  })
  const answerChannels = (request, method, path) => {
    if (path !== '/agent-channels' || method !== 'GET') return null
    const params = request?.params && typeof request.params === 'object' ? request.params : {}
    const query = request?.query && typeof request.query === 'object' ? request.query : {}
    const type = typeof params.type === 'string' ? params.type : typeof query.type === 'string' ? query.type : ''
    let rows = readChannels()
    if (type) rows = rows.filter((row) => row.type === type)
    return { id: request?.id, status: 200, data: rows, metadata: { timestamp: Date.now() } }
  }
  const answerProjects = (request, method, path) => {
    const now = Date.now()
    const metadata = { timestamp: now }
    if (path === '/agent-workspaces' && method === 'GET') {
      return { id: request?.id, status: 200, data: readProjects(), metadata }
    }
    if (path === '/agent-workspaces' && method === 'POST') {
      const body = request?.body && typeof request.body === 'object' ? request.body : {}
      const projectPath = typeof body.path === 'string' ? body.path.trim() : ''
      const explicitName = typeof body.name === 'string' ? body.name.trim() : ''
      const derivedName = projectPath.split(/[\\/]/).filter(Boolean).pop() || ''
      const name = explicitName || derivedName
      if (!projectPath || !name) {
        return {
          id: request?.id,
          status: 400,
          error: { code: 'BAD_REQUEST', message: 'project path is required', status: 400 },
          metadata
        }
      }
      const rows = readProjects()
      const existing = rows.find((row) => row.path === projectPath)
      if (existing) return { id: request?.id, status: 200, data: existing, metadata }
      const stamp = new Date(now).toISOString()
      const created = {
        id: globalThis.crypto?.randomUUID ? globalThis.crypto.randomUUID() : `project-${now}`,
        name,
        path: projectPath,
        type: 'user',
        orderKey: 'a0',
        createdAt: stamp,
        updatedAt: stamp
      }
      writeProjects([created, ...rows])
      return { id: request?.id, status: 201, data: created, metadata }
    }
    const match = /^\/agent-workspaces\/([^/]+)$/.exec(path)
    if (match && method === 'GET') {
      const row = readProjects().find((item) => item.id === decodeURIComponent(match[1]))
      if (!row) {
        return {
          id: request?.id,
          status: 404,
          error: { code: 'NOT_FOUND', message: 'not found', status: 404 },
          metadata
        }
      }
      return { id: request?.id, status: 200, data: row, metadata }
    }
    return null
  }
  const assistantStorage = 'a3s.workbench.assistants'
  const readAssistants = () => {
    try {
      const raw = globalThis.localStorage.getItem(assistantStorage)
      const parsed = raw ? JSON.parse(raw) : []
      if (!Array.isArray(parsed)) return []
      return parsed.filter(
        (row) => row && typeof row.id === 'string' && typeof row.name === 'string' && row.name.trim()
      )
    } catch {
      return []
    }
  }
  const writeAssistants = (rows) => {
    try {
      globalThis.localStorage.setItem(assistantStorage, JSON.stringify(rows))
    } catch {
      // The list still applies until the page reloads.
    }
  }
  const answerAssistants = (request, method, path) => {
    const metadata = { timestamp: Date.now() }
    if (path === '/assistants' && method === 'GET') {
      const rows = readAssistants().filter((row) => !row.deletedAt)
      return { id: request?.id, status: 200, data: { items: rows, total: rows.length }, metadata }
    }
    if (path === '/assistants' && method === 'POST') {
      const body = request?.body && typeof request.body === 'object' ? request.body : {}
      const name = typeof body.name === 'string' ? body.name.trim() : ''
      if (!name) {
        return {
          id: request?.id,
          status: 400,
          error: { code: 'BAD_REQUEST', message: 'name is required', status: 400 },
          metadata
        }
      }
      const stamp = new Date().toISOString()
      const created = {
        id: globalThis.crypto?.randomUUID ? globalThis.crypto.randomUUID() : `assistant-${Date.now()}`,
        name,
        prompt: typeof body.prompt === 'string' ? body.prompt : '',
        emoji: typeof body.emoji === 'string' && body.emoji.trim() ? body.emoji.trim() : '✨',
        description: typeof body.description === 'string' ? body.description : '',
        settings:
          body.settings && typeof body.settings === 'object' && !Array.isArray(body.settings) ? body.settings : {},
        modelId: typeof body.modelId === 'string' ? body.modelId : null,
        groupId: null,
        orderKey: 'a0',
        mcpServerIds: [],
        knowledgeBaseIds: [],
        createdAt: stamp,
        updatedAt: stamp,
        modelName: null
      }
      writeAssistants([created, ...readAssistants()])
      return { id: request?.id, status: 201, data: created, metadata }
    }
    const match = /^\/assistants\/([^/]+)$/.exec(path)
    if (!match) return null
    const assistantId = decodeURIComponent(match[1])
    const rows = readAssistants()
    const index = rows.findIndex((row) => row.id === assistantId && !row.deletedAt)
    if (method === 'GET') {
      if (index < 0) {
        return {
          id: request?.id,
          status: 404,
          error: { code: 'NOT_FOUND', message: 'not found', status: 404 },
          metadata
        }
      }
      return { id: request?.id, status: 200, data: rows[index], metadata }
    }
    if (method === 'DELETE') {
      writeAssistants(rows.filter((row) => row.id !== assistantId))
      return { id: request?.id, status: 200, data: { deleted: true }, metadata }
    }
    return null
  }
  const removeAssistantRecord = (assistantId, archive) => {
    const rows = readAssistants()
    if (!archive) {
      writeAssistants(rows.filter((row) => row.id !== assistantId))
      return
    }
    const stamp = new Date().toISOString()
    writeAssistants(rows.map((row) => (row.id === assistantId ? { ...row, deletedAt: stamp, updatedAt: stamp } : row)))
  }
  const answer = (request) => {
    const method = String(request?.method || 'GET').toUpperCase()
    const path = String(request?.path || '')
    const projects = answerProjects(request, method, path)
    if (projects) return projects
    const channels = answerChannels(request, method, path)
    if (channels) return channels
    const assistants = answerAssistants(request, method, path)
    if (assistants) return assistants
    const segments = path.split('/').filter(Boolean)
    const now = Date.now()
    if (method === 'GET' && segments.length <= 1) {
      return {
        id: request?.id,
        status: 200,
        data: emptyCollection(),
        metadata: { timestamp: now }
      }
    }
    if (method === 'GET' && ['latest', 'search', 'count', 'summary'].includes(segments[segments.length - 1])) {
      return {
        id: request?.id,
        status: 200,
        data: { session: null, topic: null, items: [], total: 0 },
        metadata: { timestamp: now }
      }
    }
    if (method === 'GET') {
      return {
        id: request?.id,
        status: 404,
        error: { code: 'NOT_FOUND', message: 'not found', status: 404 },
        metadata: { timestamp: now }
      }
    }
    return { id: request?.id, status: 200, data: {}, metadata: { timestamp: now } }
  }

  const workspaceFiles = createWorkspaceFileAccess()
  const fileApi = createStudioFileApi()

  function openHttpPage(url) {
    if (typeof url !== 'string' || !/^https?:\/\/\S+$/.test(url)) {
      return Promise.resolve({
        ok: false,
        error: { code: 'INTERNAL', message: 'unsupported url' }
      })
    }
    const runtime = globalThis.__A3S_STUDIO_RUNTIME__
    if (!runtime || !runtime.apiBaseUrl || !runtime.token) {
      return Promise.resolve({
        ok: false,
        error: { code: 'INTERNAL', message: 'studio runtime missing' }
      })
    }
    return globalThis
      .fetch(`${runtime.apiBaseUrl}/api/open-external`, {
        method: 'POST',
        headers: {
          authorization: `Bearer ${runtime.token}`,
          'content-type': 'application/json'
        },
        body: JSON.stringify({ url })
      })
      .then((response) => {
        if (response.ok) return { ok: true, data: undefined }
        return { ok: false, error: { code: 'INTERNAL', message: `open ${response.status}` } }
      })
      .catch((error) => ({
        ok: false,
        error: { code: 'INTERNAL', message: error instanceof Error ? error.message : String(error) }
      }))
  }

  const windowHostCommand = (route, input) => {
    if (route === 'window.close') return ['plugin:window|close']
    if (route === 'window.minimize') return ['plugin:window|minimize']
    if (route === 'window.maximize') return ['plugin:window|maximize']
    if (route === 'window.unmaximize') return ['plugin:window|unmaximize']
    if (route === 'window.is_maximized') return ['plugin:window|is_maximized']
    if (route === 'window.is_full_screen') return ['plugin:window|is_fullscreen']
    if (route === 'window.set_full_screen') return ['plugin:window|set_fullscreen', { value: input === true }]
    if (route === 'window.set_zoom') {
      const factor = input && typeof input.factor === 'number' && Number.isFinite(input.factor) ? input.factor : 1
      const value = Math.min(10, Math.max(0.2, Math.round(factor * 10) / 10))
      return ['plugin:webview|set_webview_zoom', { value }]
    }
    return null
  }

  const invokeWindowHost = (route, input) => {
    const command = windowHostCommand(route, input)
    if (!command) return null
    const internals = globalThis.__TAURI_INTERNALS__
    if (!internals || typeof internals.invoke !== 'function') {
      return Promise.resolve({
        ok: false,
        error: { code: 'INTERNAL', message: 'window host missing' }
      })
    }
    const pending = command[1] === undefined ? internals.invoke(command[0]) : internals.invoke(command[0], command[1])
    return pending
      .then((data) => ({ ok: true, data: data ?? null }))
      .catch((error) => ({
        ok: false,
        error: { code: 'INTERNAL', message: error instanceof Error ? error.message : String(error) }
      }))
  }

  const updaterListeners = new Map()
  globalThis.__A3S_UPDATER_EMIT__ = (event, payload) => {
    const callbacks = updaterListeners.get(event)
    if (!callbacks) return
    for (const callback of callbacks) callback(payload)
  }

  globalThis.api = {
    preference: nativePreferenceApi || {
      get: (key) => Promise.resolve(Object.hasOwn(prefs, key) ? prefs[key] : undefined),
      set: async (key, value) => {
        rememberSettings({ [key]: value })
        prefs[key] = value
        if (desktopKeys.includes(key)) postDesktopSettings()
        if (key === installSettingsKey) postInstallSettings(value)
        emit(key, value)
        return Promise.resolve()
      },
      getMultipleRaw: (keys) => Promise.resolve(Object.fromEntries(keys.map((key) => [key, prefs[key]]))),
      setMultiple: async (updates) => {
        rememberSettings(updates)
        Object.assign(prefs, updates)
        let desktop = false
        for (const [key, value] of Object.entries(updates)) {
          if (desktopKeys.includes(key)) desktop = true
          emit(key, value)
        }
        if (desktop) postDesktopSettings()
        if (Object.prototype.hasOwnProperty.call(updates, installSettingsKey)) {
          postInstallSettings(updates[installSettingsKey])
        }
        return Promise.resolve()
      },
      getAll: () => Promise.resolve(prefs),
      subscribe: () => Promise.resolve(),
      onChanged: (callback) => {
        listeners.add(callback)
        return () => listeners.delete(callback)
      }
    },
    cache: {
      getAllShared: () => Promise.resolve({}),
      broadcastSync: () => {},
      onSync: () => () => {}
    },
    dataApi: {
      request: (request) => Promise.resolve(answer(request)),
      onDataChanged: () => () => {}
    },
    ipcApi: {
      request: (route, input) => {
        const handlePath = input && input.kind === 'path' && typeof input.path === 'string' ? input.path : ''
        if (handlePath && route === 'file.get_metadata') {
          return workspaceFiles
            .metadata(handlePath)
            .then((data) => ({ ok: true, data }))
            .catch(() => ({ ok: true, data: null }))
        }
        if (handlePath && (route === 'file.open' || route === 'file.show_in_folder')) {
          const action = route === 'file.open' ? workspaceFiles.open : workspaceFiles.reveal
          return action(handlePath)
            .then((response) => {
              if (response.ok) return { ok: true, data: undefined }
              if (route === 'file.open' && response.status === 403) {
                return {
                  ok: false,
                  error: { code: 'FILE_OPEN_BLOCKED_UNSAFE_TYPE', message: 'blocked unsafe file type' }
                }
              }
              return { ok: false, error: { code: 'INTERNAL', message: `workspace file ${response.status}` } }
            })
            .catch((error) => ({ ok: false, error: { code: 'INTERNAL', message: String(error) } }))
        }
        if (route === 'system.get_native_theme') {
          const dark =
            typeof globalThis.window.matchMedia === 'function' &&
            globalThis.window.matchMedia('(prefers-color-scheme: dark)').matches
          return Promise.resolve({ ok: true, data: dark ? 'dark' : 'light' })
        }
        if (route === 'app.get_info') {
          const version = typeof globalThis.__A3S_APP_VERSION__ === 'string' ? globalThis.__A3S_APP_VERSION__ : ''
          const runtime = globalThis.__A3S_STUDIO_RUNTIME__
          const homePath = runtime && typeof runtime.homePath === 'string' ? runtime.homePath : ''
          const info = (notesPath) => ({
            ok: true,
            data: {
              version,
              isPortable: false,
              isPackaged: false,
              appPath: homePath,
              homePath,
              notesPath,
              configPath: homePath,
              appDataPath: homePath,
              resourcesPath: '',
              logsPath: '',
              arch: '',
              installPath: '',
              pendingNotifications: []
            }
          })
          return fileApi
            .notesDirectory()
            .then(info)
            .catch(() => info(''))
        }
        if (route === 'export.word.from_markdown') {
          const markdown = input && typeof input.markdown === 'string' ? input.markdown : ''
          const fileName = input && typeof input.fileName === 'string' ? input.fileName : 'export'
          return fileApi
            .saveWord(fileName, markdown)
            .then((saved) => ({ ok: true, data: saved }))
            .catch((error) => ({
              ok: false,
              error: { code: 'INTERNAL', message: error instanceof Error ? error.message : String(error) }
            }))
        }
        if (route === 'export.obsidian.get_vaults') {
          return fileApi
            .obsidianVaults()
            .then((data) => ({ ok: true, data }))
            .catch(() => ({ ok: true, data: [] }))
        }
        if (route === 'export.obsidian.get_files') {
          const vaultName = input && typeof input.vaultName === 'string' ? input.vaultName : ''
          return fileApi
            .obsidianFiles(vaultName)
            .then((data) => ({ ok: true, data }))
            .catch(() => ({ ok: true, data: [] }))
        }
        if (
          (route === 'system.shell.open_external_website' || route === 'system.shell.open_website') &&
          typeof input === 'string'
        ) {
          return openHttpPage(input)
        }
        if (route === 'system.mac.open_screen_capture_settings') {
          const internals = globalThis.__TAURI_INTERNALS__
          if (!internals || typeof internals.invoke !== 'function') {
            return Promise.resolve({
              ok: false,
              error: { code: 'INTERNAL', message: 'screen capture settings host missing' }
            })
          }
          return internals
            .invoke('open_screen_capture_settings')
            .then(() => ({ ok: true, data: null }))
            .catch((error) => ({
              ok: false,
              error: { code: 'INTERNAL', message: error instanceof Error ? error.message : String(error) }
            }))
        }
        if (route === 'channel.create') {
          const body = input && typeof input === 'object' ? input : {}
          const now = new Date().toISOString()
          const id = globalThis.crypto?.randomUUID ? globalThis.crypto.randomUUID() : `channel-${Date.now()}`
          const row = normalizeChannel({ ...body, id, createdAt: now }, now)
          writeChannels([row, ...readChannels()])
          return Promise.resolve({ ok: true, data: row })
        }
        if (route === 'channel.update') {
          const channelId = input && typeof input.channelId === 'string' ? input.channelId : ''
          const updates = input && input.updates && typeof input.updates === 'object' ? input.updates : {}
          const rows = readChannels()
          const index = rows.findIndex((row) => row.id === channelId)
          if (index < 0) {
            return Promise.resolve({ ok: false, error: { code: 'NOT_FOUND', message: 'not found' } })
          }
          const now = new Date().toISOString()
          const next = normalizeChannel(
            { ...rows[index], ...updates, id: rows[index].id, createdAt: rows[index].createdAt },
            now
          )
          rows[index] = next
          writeChannels(rows)
          return Promise.resolve({ ok: true, data: next })
        }
        if (route === 'channel.delete') {
          const channelId = input && typeof input.channelId === 'string' ? input.channelId : ''
          writeChannels(readChannels().filter((row) => row.id !== channelId))
          return Promise.resolve({ ok: true, data: null })
        }
        if (route === 'channel.get_logs') {
          return Promise.resolve({ ok: true, data: [] })
        }
        if (route === 'trash.assistant.archive' || route === 'trash.assistant.delete_permanently') {
          const assistantId = input && typeof input.assistantId === 'string' ? input.assistantId : ''
          if (!assistantId) {
            return Promise.resolve({ ok: false, error: { code: 'BAD_REQUEST', message: 'assistant id is required' } })
          }
          removeAssistantRecord(assistantId, route === 'trash.assistant.archive')
          return Promise.resolve({ ok: true, data: { deleted: true } })
        }
        if (
          route === 'app.updater.check_for_update' ||
          route === 'app.updater.quit_and_install' ||
          route === 'app.updater.release_notes.get'
        ) {
          const internals = globalThis.__TAURI_INTERNALS__
          if (!internals || typeof internals.invoke !== 'function') {
            return Promise.resolve({
              ok: false,
              error: { code: 'INTERNAL', message: 'updater host missing' }
            })
          }
          const command =
            route === 'app.updater.check_for_update'
              ? 'app_updater_check'
              : route === 'app.updater.quit_and_install'
                ? 'app_updater_install'
                : 'app_updater_notes'
          return internals
            .invoke(command)
            .then((data) => ({ ok: true, data: data ?? null }))
            .catch((error) => ({
              ok: false,
              error: { code: 'INTERNAL', message: error instanceof Error ? error.message : String(error) }
            }))
        }
        if (typeof route === 'string' && route.startsWith('binary.')) {
          const internals = globalThis.__TAURI_INTERNALS__
          if (!internals || typeof internals.invoke !== 'function') {
            return Promise.resolve({
              ok: false,
              error: { code: 'INTERNAL', message: 'binary host missing' }
            })
          }
          return internals
            .invoke('binary_ipc', { route, input: input ?? null })
            .then((data) => ({ ok: true, data }))
            .catch((error) => ({
              ok: false,
              error: { code: 'INTERNAL', message: error instanceof Error ? error.message : String(error) }
            }))
        }
        if (route === 'system.get_ip_country') {
          return Promise.resolve({ ok: true, data: 'CN' })
        }
        if (route === 'file.tree.create') {
          const rootPath = input && typeof input.rootPath === 'string' ? input.rootPath : ''
          return fileApi.directorySnapshot(rootPath).then((data) => ({ ok: true, data }))
        }
        if (route === 'file.tree.activate') {
          return Promise.resolve({ ok: true, data: true })
        }
        if (route === 'file.tree.dispose') {
          return Promise.resolve({ ok: true, data: null })
        }
        if (route === 'file.batch_get_metadata') {
          const items = input && Array.isArray(input.items) ? input.items : []
          return Promise.all(
            items.map(async (item) => {
              const handle = item && item.handle
              const key = item && typeof item.key === 'string' ? item.key : ''
              const path = handle && handle.kind === 'path' && typeof handle.path === 'string' ? handle.path : ''
              if (!key || !path) return null
              const data = await workspaceFiles.metadata(path).catch(() => null)
              return data ? [key, data] : null
            })
          ).then((rows) => ({ ok: true, data: Object.fromEntries(rows.filter(Boolean)) }))
        }
        if (route === 'app.git.worktrees') {
          const internals = globalThis.__TAURI_INTERNALS__
          if (!internals || typeof internals.invoke !== 'function') {
            return Promise.resolve({
              ok: false,
              error: { code: 'INTERNAL', message: 'worktree host missing' }
            })
          }
          return internals
            .invoke('git_worktrees', { input: input ?? null })
            .then((data) => ({ ok: true, data }))
            .catch((error) => ({
              ok: false,
              error: { code: 'INTERNAL', message: error instanceof Error ? error.message : String(error) }
            }))
        }
        const windowResult = invokeWindowHost(route, input)
        if (windowResult) return windowResult
        return Promise.resolve({
          ok: true,
          data: { homePath: '', resourcesPath: '', pendingNotifications: [] }
        })
      },
      on: (event, callback) => {
        let callbacks = updaterListeners.get(event)
        if (!callbacks) {
          callbacks = new Set()
          updaterListeners.set(event, callbacks)
        }
        callbacks.add(callback)
        return () => callbacks.delete(callback)
      }
    },
    shell: {
      openExternal: (url) =>
        openHttpPage(url).then((result) => {
          if (result.ok) return
          throw new Error(result.error.message)
        })
    },
    fs: {
      read: (filePath) => workspaceFiles.read(String(filePath)),
      readText: (filePath) => workspaceFiles.readText(String(filePath))
    },
    file: fileApi
  }

  function defaultWorkspaceRoot() {
    const custom = typeof prefs[workspacePreference] === 'string' ? prefs[workspacePreference].trim() : ''
    if (custom.startsWith('/') || /^[A-Za-z]:[\\/]/.test(custom)) return custom.replace(/[\\/]+$/, '')
    const runtime = globalThis.__A3S_STUDIO_RUNTIME__
    const home = runtime && typeof runtime.homePath === 'string' ? runtime.homePath.replace(/[\\/]+$/, '') : ''
    if (!home || home === '/' || home === '\\') throw new Error('workspace root missing')
    const sep = home.includes('\\') && !home.includes('/') ? '\\' : '/'
    return `${home}${sep}.a3s`
  }

  function createWorkspaceFileAccess() {
    const isAbsolute = (value) => typeof value === 'string' && (value.startsWith('/') || /^[A-Za-z]:[\\/]/.test(value))
    const trimRoot = (value) => value.replace(/[\\/]+$/, '')
    const rootFor = (filePath) => {
      const candidates = readProjects().map((row) => row.path)
      try {
        candidates.push(defaultWorkspaceRoot())
      } catch {
        // No default workspace yet; project roots still apply.
      }
      let best = ''
      for (const candidate of candidates) {
        if (!isAbsolute(candidate)) continue
        const root = trimRoot(candidate)
        const inside = filePath === root || filePath.startsWith(`${root}/`) || filePath.startsWith(`${root}\\`)
        if (inside && root.length > best.length) best = root
      }
      return best
    }
    const runtime = () => {
      const value = globalThis.__A3S_STUDIO_RUNTIME__
      if (!value || !value.apiBaseUrl || !value.token) throw new Error('studio runtime missing')
      return value
    }
    const query = (filePath) => {
      const root = rootFor(filePath)
      if (!root) return null
      return `root=${encodeURIComponent(root)}&path=${encodeURIComponent(filePath)}`
    }
    const get = async (route, filePath) => {
      const params = query(filePath)
      if (!params) return { status: 403 }
      const { apiBaseUrl, token } = runtime()
      return globalThis.fetch(`${apiBaseUrl}/api/workspace-file${route}?${params}`, {
        headers: { authorization: `Bearer ${token}` }
      })
    }
    const post = async (route, filePath) => {
      const root = rootFor(filePath)
      if (!root) return { ok: false, status: 403 }
      const { apiBaseUrl, token } = runtime()
      return globalThis.fetch(`${apiBaseUrl}/api/workspace-file${route}`, {
        method: 'POST',
        headers: { authorization: `Bearer ${token}`, 'content-type': 'application/json' },
        body: JSON.stringify({ root, path: filePath })
      })
    }
    const extensionOf = (filePath) => {
      const name = filePath.split(/[/\\]/).pop() || ''
      return name.includes('.') ? name.slice(name.lastIndexOf('.') + 1).toLowerCase() : ''
    }
    const fileType = (ext, isText) => {
      if (/^(png|jpe?g|gif|webp|bmp|svg|ico|heic|avif)$/.test(ext)) return 'image'
      if (isText) return 'text'
      if (/^(pdf|docx?|pptx?|xlsx?|numbers|pages|key|odt|ods|odp|rtf|epub)$/.test(ext)) return 'document'
      if (/^(mp3|wav|m4a|aac|flac|ogg)$/.test(ext)) return 'audio'
      if (/^(mp4|mov|m4v|webm|mkv|avi)$/.test(ext)) return 'video'
      return 'other'
    }
    const MIME = {
      pdf: 'application/pdf',
      png: 'image/png',
      jpg: 'image/jpeg',
      jpeg: 'image/jpeg',
      gif: 'image/gif',
      webp: 'image/webp',
      svg: 'image/svg+xml',
      md: 'text/markdown',
      html: 'text/html',
      htm: 'text/html',
      json: 'application/json',
      docx: 'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
      xlsx: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
      pptx: 'application/vnd.openxmlformats-officedocument.presentationml.presentation'
    }
    const composerUpload = (filePath) => /[/\\]a3s-studio-composer[/\\]/.test(filePath)
    const metadataResponse = async (filePath) => {
      if (rootFor(filePath)) return get('', filePath)
      // A session workspace is its own root. It is not always in the local project list.
      const { apiBaseUrl, token } = runtime()
      return globalThis.fetch(
        `${apiBaseUrl}/api/workspace-file?root=${encodeURIComponent(filePath)}&path=${encodeURIComponent(filePath)}`,
        { headers: { authorization: `Bearer ${token}` } }
      )
    }
    return {
      metadata: async (filePath) => {
        if (composerUpload(filePath)) {
          const ext = extensionOf(filePath)
          const type = fileType(ext, false)
          const mime = MIME[ext] || (type === 'text' ? 'text/plain' : 'application/octet-stream')
          return { size: 0, createdAt: 0, modifiedAt: 0, kind: 'file', type, mime }
        }
        const response = await metadataResponse(filePath)
        if (!response.ok) return null
        const body = await response.json()
        const base = { size: body.size || 0, createdAt: body.createdAt || 0, modifiedAt: body.modifiedAt || 0 }
        if (body.kind === 'directory') return { ...base, kind: 'directory' }
        const ext = extensionOf(filePath)
        const type = fileType(ext, body.isText === true)
        const mime = MIME[ext] || (type === 'text' ? 'text/plain' : 'application/octet-stream')
        return { ...base, kind: 'file', type, mime }
      },
      readText: async (filePath) => {
        const response = await get('/text', filePath)
        if (!response.ok) throw new Error(`workspace file ${response.status}`)
        const body = await response.json()
        return typeof body.text === 'string' ? body.text : ''
      },
      read: async (filePath) => {
        const response = await get('/raw', filePath)
        if (!response.ok) throw new Error(`workspace file ${response.status}`)
        return new Uint8Array(await response.arrayBuffer())
      },
      open: (filePath) => post('/open', filePath),
      reveal: (filePath) => post('/reveal', filePath)
    }
  }

  function createStudioFileApi() {
    const saved = new Map()
    const basename = (value) => {
      const name =
        String(value || 'file.bin')
          .split(/[/\\]/)
          .pop() || 'file.bin'
      return name.replace(/[^\w.\- ()\u0080-\uffff]+/g, '_') || 'file.bin'
    }
    const metadata = (record) => {
      const ext = record.name.includes('.') ? record.name.slice(record.name.lastIndexOf('.')) : ''
      const image = /^\.(png|jpe?g|gif|webp|bmp|svg)$/i.test(ext)
      return {
        id: record.path,
        name: record.name,
        origin_name: record.name.replace(/\.[^.]+$/, '') || record.name,
        path: record.path,
        size: record.size,
        ext,
        type: image ? 'image' : 'document',
        created_at: new Date().toISOString(),
        count: 1
      }
    }
    const workspaceRoot = defaultWorkspaceRoot
    const postEntry = async (body) => {
      const runtime = globalThis.__A3S_STUDIO_RUNTIME__
      if (!runtime || !runtime.apiBaseUrl || !runtime.token) throw new Error('studio runtime missing')
      const response = await globalThis.fetch(`${runtime.apiBaseUrl}/api/workspace-entries`, {
        method: 'POST',
        headers: {
          authorization: `Bearer ${runtime.token}`,
          'content-type': 'application/json'
        },
        body: JSON.stringify({ root: workspaceRoot(), ...body })
      })
      if (response.ok) return
      if (response.status === 404) throw new Error('does not exist')
      throw new Error(`workspace entry ${response.status}`)
    }
    const postFile = async (name, bytes) => {
      const runtime = globalThis.__A3S_STUDIO_RUNTIME__
      if (!runtime || !runtime.apiBaseUrl || !runtime.token) {
        throw new Error('studio runtime missing')
      }
      const response = await globalThis.fetch(
        `${runtime.apiBaseUrl}/api/composer-files/${encodeURIComponent(basename(name))}`,
        {
          method: 'POST',
          headers: {
            authorization: `Bearer ${runtime.token}`,
            'content-type': 'application/octet-stream'
          },
          body: bytes
        }
      )
      if (!response.ok) throw new Error(`composer file ${response.status}`)
      return response.json()
    }
    const requireRuntime = () => {
      const runtime = globalThis.__A3S_STUDIO_RUNTIME__
      if (!runtime || !runtime.apiBaseUrl || !runtime.token) throw new Error('studio runtime missing')
      return runtime
    }
    const asBytes = (data) => {
      if (typeof data === 'string') return new globalThis.TextEncoder().encode(data)
      if (data instanceof Uint8Array) return data
      if (ArrayBuffer.isView(data)) return new Uint8Array(data.buffer, data.byteOffset, data.byteLength)
      throw new Error('save data missing')
    }
    const parentOf = (filePath) => {
      const index = Math.max(filePath.lastIndexOf('/'), filePath.lastIndexOf('\\'))
      if (index <= 0) throw new Error('save path missing')
      return filePath.slice(0, index)
    }
    const nameOf = (filePath) => filePath.slice(Math.max(filePath.lastIndexOf('/'), filePath.lastIndexOf('\\')) + 1)
    const joinPath = (directory, name) => {
      const sep = directory.includes('\\') && !directory.includes('/') ? '\\' : '/'
      return `${directory.replace(/[\\/]+$/, '')}${sep}${name}`
    }
    const postJson = async (path, body) => {
      const runtime = requireRuntime()
      const response = await globalThis.fetch(`${runtime.apiBaseUrl}${path}`, {
        method: 'POST',
        headers: {
          authorization: `Bearer ${runtime.token}`,
          'content-type': 'application/json'
        },
        body: JSON.stringify(body)
      })
      const payload = response.status === 204 ? null : await response.json().catch(() => null)
      return { status: response.status, body: payload }
    }
    let directoryTreeSerial = 0
    const listEntries = async (directory) => {
      const runtime = requireRuntime()
      const response = await globalThis.fetch(
        `${runtime.apiBaseUrl}/api/local-documents/entries?root=${encodeURIComponent(directory)}`,
        { headers: { authorization: `Bearer ${runtime.token}` } }
      )
      if (!response.ok) throw new Error(`local documents ${response.status}`)
      const body = await response.json()
      const entries = Array.isArray(body.entries) ? body.entries : []
      return entries.flatMap((entry) => {
        if (!entry || typeof entry.name !== 'string') return []
        const kind = entry.kind === 'directory' || entry.kind === 'folder' ? 'directory' : 'file'
        return [{ name: entry.name, kind }]
      })
    }
    const readTextFile = async (root, filePath) => {
      const runtime = requireRuntime()
      const params = `root=${encodeURIComponent(root)}&path=${encodeURIComponent(filePath)}`
      const response = await globalThis.fetch(`${runtime.apiBaseUrl}/api/workspace-file/text?${params}`, {
        headers: { authorization: `Bearer ${runtime.token}` }
      })
      if (!response.ok) throw new Error(`workspace file ${response.status}`)
      const body = await response.json()
      return typeof body.text === 'string' ? body.text : ''
    }
    const writeContainedBytes = async (filePath, bytes) => {
      const parent = parentOf(filePath)
      const name = nameOf(filePath)
      const created = await postJson('/api/local-documents/entries', { root: parent, parent: '', name, kind: 'file' })
      if (created.status !== 200 && created.status !== 409) throw new Error(`save file ${created.status}`)
      const target =
        created.status === 200 && created.body && typeof created.body.path === 'string' ? created.body.path : filePath
      const runtime = requireRuntime()
      const params = `root=${encodeURIComponent(parent)}&path=${encodeURIComponent(target)}`
      const response = await globalThis.fetch(`${runtime.apiBaseUrl}/api/workspace-file/raw?${params}`, {
        method: 'PUT',
        headers: {
          authorization: `Bearer ${runtime.token}`,
          'content-type': 'application/octet-stream'
        },
        body: bytes
      })
      if (!response.ok) throw new Error(`save file ${response.status}`)
      return target
    }
    const chooseDirectory = async (options) => {
      const runtime = requireRuntime()
      const response = await globalThis.fetch(`${runtime.apiBaseUrl}/api/workspace-folder`, {
        method: 'POST',
        headers: {
          authorization: `Bearer ${runtime.token}`,
          'content-type': 'application/json'
        },
        body: JSON.stringify({
          defaultPath: options && options.defaultPath ? options.defaultPath : '',
          title: options && options.title ? options.title : ''
        })
      })
      if (response.status === 404) return ensureNotesDirectory()
      if (!response.ok) throw new Error(`workspace folder ${response.status}`)
      const body = await response.json()
      if (body && body.reason === 'desktop-capability-unavailable') {
        throw new Error('desktop-capability-unavailable')
      }
      return typeof body.path === 'string' && body.path ? body.path : null
    }
    const suggestedName = (fileName, extension) => {
      const flattened = String(fileName || 'export').replace(/[/\\]/g, '_')
      let name = basename(flattened)
      if (name.startsWith('.')) name = `_${name.slice(1)}`
      if (extension) {
        const suffix = `.${String(extension).replace(/^\./, '').toLowerCase()}`
        if (suffix !== '.' && !name.toLowerCase().endsWith(suffix)) name = `${name}${suffix}`
      }
      if (name.length > 255) {
        const dot = name.lastIndexOf('.')
        const ext = dot > 0 ? name.slice(dot) : ''
        name = `${name.slice(0, Math.max(1, 255 - ext.length))}${ext}`.slice(0, 255)
      }
      return name || 'export'
    }
    const extensionFrom = (options) => {
      const extensions = options && options.filters && options.filters[0] && options.filters[0].extensions
      if (!Array.isArray(extensions) || extensions.length === 0) return ''
      const ext = String(extensions[0] || '').replace(/^\./, '')
      return ext && ext !== '*' ? ext : ''
    }
    const unusedName = async (directory, name) => {
      let taken = new Set()
      try {
        taken = new Set((await listEntries(directory)).map((entry) => entry.name))
      } catch {
        taken = new Set()
      }
      if (!taken.has(name)) return name
      const dot = name.lastIndexOf('.')
      const stem = dot > 0 ? name.slice(0, dot) : name
      const ext = dot > 0 ? name.slice(dot) : ''
      for (let index = 2; index < 100; index += 1) {
        const candidate = `${stem} (${index})${ext}`
        if (!taken.has(candidate)) return candidate
      }
      return `${stem}-${Date.now()}${ext}`
    }
    const dataUrlBytes = (dataUrl) => {
      const text = String(dataUrl || '')
      const comma = text.indexOf(',')
      if (!text.startsWith('data:') || comma < 0) return null
      const meta = text.slice(0, comma)
      const payload = text.slice(comma + 1)
      if (!/base64/i.test(meta)) return new globalThis.TextEncoder().encode(decodeURIComponent(payload))
      const binary = globalThis.atob(payload)
      const bytes = new Uint8Array(binary.length)
      for (let index = 0; index < binary.length; index += 1) bytes[index] = binary.charCodeAt(index)
      return bytes
    }
    let notesDirectoryPromise = null
    const ensureNotesDirectory = async () => {
      const runtime = requireRuntime()
      const home = typeof runtime.homePath === 'string' ? runtime.homePath.replace(/[\\/]+$/, '') : ''
      if (!home || home === '/' || home === '\\') throw new Error('notes path missing')
      let parent = joinPath(home, 'Documents')
      try {
        await listEntries(parent)
      } catch {
        parent = home
      }
      const notes = joinPath(parent, 'ToWork')
      try {
        await listEntries(notes)
        return notes
      } catch {
        // The notes folder is created on first use.
      }
      const created = await postJson('/api/local-documents/entries', {
        root: parent,
        parent: '',
        name: 'ToWork',
        kind: 'folder'
      })
      if (created.status !== 200 && created.status !== 409) throw new Error(`notes folder ${created.status}`)
      return notes
    }
    const crc32 = (bytes) => {
      let crc = 0xffffffff
      for (let index = 0; index < bytes.length; index += 1) {
        crc ^= bytes[index]
        for (let bit = 0; bit < 8; bit += 1) crc = (crc >>> 1) ^ (crc & 1 ? 0xedb88320 : 0)
      }
      return (crc ^ 0xffffffff) >>> 0
    }
    const concatBytes = (parts) => {
      const size = parts.reduce((sum, part) => sum + part.length, 0)
      const out = new Uint8Array(size)
      let offset = 0
      for (const part of parts) {
        out.set(part, offset)
        offset += part.length
      }
      return out
    }
    const zipStore = (files) => {
      const locals = []
      const centrals = []
      let offset = 0
      for (const file of files) {
        const name = new globalThis.TextEncoder().encode(file.name)
        const data = file.data
        const crc = crc32(data)
        const local = new Uint8Array(30 + name.length)
        const localView = new DataView(local.buffer)
        localView.setUint32(0, 0x04034b50, true)
        localView.setUint16(4, 20, true)
        localView.setUint16(6, 0x0800, true)
        localView.setUint32(14, crc, true)
        localView.setUint32(18, data.length, true)
        localView.setUint32(22, data.length, true)
        localView.setUint16(26, name.length, true)
        local.set(name, 30)
        locals.push(local, data)
        const central = new Uint8Array(46 + name.length)
        const centralView = new DataView(central.buffer)
        centralView.setUint32(0, 0x02014b50, true)
        centralView.setUint16(4, 20, true)
        centralView.setUint16(6, 20, true)
        centralView.setUint16(8, 0x0800, true)
        centralView.setUint32(16, crc, true)
        centralView.setUint32(20, data.length, true)
        centralView.setUint32(24, data.length, true)
        centralView.setUint16(28, name.length, true)
        centralView.setUint32(42, offset, true)
        central.set(name, 46)
        centrals.push(central)
        offset += local.length + data.length
      }
      const centralBytes = concatBytes(centrals)
      const end = new Uint8Array(22)
      const endView = new DataView(end.buffer)
      endView.setUint32(0, 0x06054b50, true)
      endView.setUint16(8, files.length, true)
      endView.setUint16(10, files.length, true)
      endView.setUint32(12, centralBytes.length, true)
      endView.setUint32(16, offset, true)
      return concatBytes([...locals, centralBytes, end])
    }
    const wordDocument = (markdown) => {
      const lines = String(markdown || '').split(/\r?\n/)
      const paragraphs = (lines.length > 0 ? lines : ['']).map((line) => {
        const text = line.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
        return `<w:p><w:r><w:t xml:space="preserve">${text}</w:t></w:r></w:p>`
      })
      return `<?xml version="1.0" encoding="UTF-8" standalone="yes"?>\n<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>${paragraphs.join('')}</w:body></w:document>`
    }
    const docxBytes = (markdown) => {
      const utf8 = (text) => new globalThis.TextEncoder().encode(text)
      return zipStore([
        {
          name: '[Content_Types].xml',
          data: utf8(
            '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>\n<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>'
          )
        },
        {
          name: '_rels/.rels',
          data: utf8(
            '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>\n<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>'
          )
        },
        { name: 'word/document.xml', data: utf8(wordDocument(markdown)) },
        {
          name: 'word/_rels/document.xml.rels',
          data: utf8(
            '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>\n<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"></Relationships>'
          )
        }
      ])
    }
    const saveNamedBytes = async (name, bytes) => {
      // The host exposes a directory panel. The chosen folder receives this file name.
      const directory = await chooseDirectory({ title: name })
      if (!directory) return null
      const unique = await unusedName(directory, name)
      const runtime = requireRuntime()
      const params = new globalThis.URLSearchParams({ root: directory, directory, name: unique, replace: 'false' })
      const response = await globalThis.fetch(`${runtime.apiBaseUrl}/api/workspace-file/import?${params}`, {
        method: 'POST',
        headers: { authorization: `Bearer ${runtime.token}`, 'content-type': 'application/octet-stream' },
        body: bytes
      })
      if (!response.ok) throw new Error(`save file ${response.status}`)
      const receipt = await response.json()
      if (!receipt || typeof receipt.path !== 'string' || !receipt.path) throw new Error('save path missing')
      return receipt.path
    }
    const obsidianConfig = async () => {
      const runtime = requireRuntime()
      const home = typeof runtime.homePath === 'string' ? runtime.homePath.replace(/[\\/]+$/, '') : ''
      if (!home) return []
      const dir = `${home}/Library/Application Support/obsidian`
      let text = ''
      try {
        text = await readTextFile(dir, `${dir}/obsidian.json`)
      } catch {
        return []
      }
      let parsed = null
      try {
        parsed = JSON.parse(text)
      } catch {
        return []
      }
      const vaults = parsed && parsed.vaults && typeof parsed.vaults === 'object' ? parsed.vaults : {}
      return Object.values(vaults).flatMap((vault) => {
        if (!vault || typeof vault.path !== 'string' || !vault.path) return []
        const folder = vault.path.split('/').filter(Boolean).pop() || vault.path
        const name = typeof vault.name === 'string' && vault.name ? vault.name : folder
        return [{ path: vault.path, name }]
      })
    }
    return {
      createInternalEntry: async (params) => {
        const path = params && params.source === 'path' && typeof params.path === 'string' ? params.path : ''
        if (!path) throw new Error('attachment path missing')
        const name = path.split(/[/\\]/).pop() || 'file'
        const ext = name.includes('.') ? name.slice(name.lastIndexOf('.')) : ''
        return { id: path, ext }
      },
      getPhysicalPath: async (params) => {
        const id = params && typeof params.id === 'string' ? params.id : ''
        if (!id) throw new Error('file entry missing')
        return id
      },
      createTempFile: async (fileName) => `pending:${Date.now().toString(36)}:${basename(fileName)}`,
      mkdir: (dirPath) => postEntry({ op: 'mkdir', path: dirPath }),
      moveDir: (dirPath, newDirPath) => postEntry({ op: 'move', path: dirPath, to: newDirPath }),
      write: async (filePath, data) => {
        if (typeof filePath === 'string' && (filePath.startsWith('/') || /^[A-Za-z]:[\\/]/.test(filePath))) {
          if (typeof data === 'string') {
            try {
              await postEntry({ op: 'write', path: filePath, data })
              return
            } catch (error) {
              const message = String(error && error.message)
              if (message !== 'does not exist' && !/workspace entry (400|404|413)/.test(message)) throw error
            }
          }
          await writeContainedBytes(filePath, asBytes(data))
          return
        }
        const pending = String(filePath).startsWith('pending:')
        const name = pending ? String(filePath).split(':').slice(2).join(':') : basename(filePath)
        const bytes = typeof data === 'string' ? new globalThis.TextEncoder().encode(data) : data
        const record = await postFile(name, bytes)
        const meta = metadata({ path: record.path, name: record.name || name, size: record.size })
        saved.set(record.path, meta)
        saved.set(filePath, meta)
      },
      get: async (filePath) => saved.get(filePath) || null,
      getPathForFile: () => '',
      listDirectoryEntries: async (dirPath, options) => {
        const runtime = globalThis.__A3S_STUDIO_RUNTIME__
        if (!runtime || !runtime.apiBaseUrl || !runtime.token) throw new Error('studio runtime missing')
        const pattern = options && typeof options.searchPattern === 'string' ? options.searchPattern.trim() : ''
        const params = [`root=${encodeURIComponent(String(dirPath))}`]
        if (pattern && pattern !== '.') params.push(`q=${encodeURIComponent(pattern)}`)
        if (options && Number.isInteger(options.maxEntries) && options.maxEntries > 0) {
          params.push(`limit=${options.maxEntries}`)
        }
        const response = await globalThis.fetch(`${runtime.apiBaseUrl}/api/workspace-search?${params.join('&')}`, {
          headers: { authorization: `Bearer ${runtime.token}` }
        })
        if (!response.ok) throw new Error(`workspace search ${response.status}`)
        const body = await response.json()
        if (!body || !Array.isArray(body.entries)) throw new Error('workspace search response invalid')
        const entries = body.entries
        return entries.flatMap((entry) => {
          if (!entry || typeof entry.path !== 'string' || !entry.path) return []
          const isDirectory = entry.isDirectory === true
          if (
            (isDirectory && options?.includeDirectories === false) ||
            (!isDirectory && options?.includeFiles === false)
          ) {
            return []
          }
          return [{ path: entry.path, isDirectory }]
        })
      },
      selectFolder: (options) => chooseDirectory(options),
      save: async (fileName, data, options) => {
        const extension = extensionFrom(options)
        const saved = await saveNamedBytes(suggestedName(fileName, extension), asBytes(data))
        return saved
      },
      saveImage: async (fileName, dataUrl) => {
        const bytes = dataUrlBytes(dataUrl)
        if (!bytes) return false
        const saved = await saveNamedBytes(suggestedName(fileName, 'png'), bytes)
        return Boolean(saved)
      },
      saveWord: async (fileName, markdown) =>
        Boolean(await saveNamedBytes(suggestedName(fileName, 'docx'), docxBytes(markdown))),
      checkFileName: async (dirPath, name, isFile) => {
        let safeName = basename(String(name || 'note')).replace(/\.md$/i, '')
        if (safeName.startsWith('.')) safeName = `_${safeName.slice(1)}`
        if (!safeName || safeName === 'file.bin') safeName = 'note'
        safeName = safeName.slice(0, 250)
        const target = isFile ? `${safeName}.md` : safeName
        let exists = false
        try {
          exists = (await listEntries(dirPath)).some((entry) => entry.name === target)
        } catch {
          exists = false
        }
        return { safeName, exists }
      },
      directorySnapshot: async (rootPath) => {
        const root = typeof rootPath === 'string' ? rootPath : ''
        const basename = root.split(/[\\/]/).filter(Boolean).pop() || root
        const children = {}
        if (root) {
          try {
            for (const entry of await listEntries(root)) {
              const path = joinPath(root, entry.name)
              children[entry.name] =
                entry.kind === 'directory'
                  ? { kind: 'directory', path, basename: entry.name, children: {} }
                  : { kind: 'file', path, basename: entry.name }
            }
          } catch {
            // A missing listing stays an empty directory so the task pane does not log a broken snapshot.
          }
        }
        directoryTreeSerial += 1
        return {
          treeId: `studio-tree-${directoryTreeSerial}`,
          revision: 0,
          snapshot: { kind: 'directory', path: root, basename, children }
        }
      },
      validateNotesDirectory: async (dirPath) => {
        try {
          await listEntries(dirPath)
          return true
        } catch {
          return false
        }
      },
      notesDirectory: () => {
        if (!notesDirectoryPromise) {
          notesDirectoryPromise = ensureNotesDirectory().catch((error) => {
            notesDirectoryPromise = null
            throw error
          })
        }
        return notesDirectoryPromise
      },
      obsidianVaults: () => obsidianConfig(),
      obsidianFiles: async (vaultName) => {
        const vaults = await obsidianConfig()
        const vault = vaults.find((item) => item.name === vaultName)
        if (!vault) return []
        const found = []
        const walk = async (relative, depth) => {
          if (depth > 6 || found.length > 400) return
          const directory = relative ? joinPath(vault.path, relative) : vault.path
          let entries = []
          try {
            entries = await listEntries(directory)
          } catch {
            return
          }
          for (const entry of entries) {
            if (found.length > 400) return
            const rel = relative ? `${relative}/${entry.name}` : entry.name
            if (entry.kind === 'directory') {
              found.push({ path: rel, type: 'folder', name: entry.name })
              await walk(rel, depth + 1)
            } else if (entry.name.toLowerCase().endsWith('.md')) {
              found.push({ path: rel, type: 'markdown', name: entry.name })
            }
          }
        }
        await walk('', 0)
        return found
      },
      select: (options) =>
        new Promise((resolve, reject) => {
          const input = globalThis.document.createElement('input')
          input.type = 'file'
          input.hidden = true
          const settle = (complete, value) => {
            input.remove()
            complete(value)
          }
          const multiple = Array.isArray(options?.properties) && options.properties.includes('multiSelections')
          input.multiple = multiple
          const extensions = options?.filters?.[0]?.extensions
          if (Array.isArray(extensions) && extensions.length > 0 && !extensions.includes('*')) {
            input.accept = extensions.map((ext) => (String(ext).startsWith('.') ? ext : `.${ext}`)).join(',')
          }
          input.addEventListener('cancel', () => {
            settle(resolve, null)
          })
          input.addEventListener('change', async () => {
            const chosen = [...(input.files || [])]
            if (chosen.length === 0) {
              settle(resolve, null)
              return
            }
            try {
              const api = globalThis.api.file
              const metas = []
              for (const file of chosen) {
                const key = await api.createTempFile(file.name)
                await api.write(key, new Uint8Array(await file.arrayBuffer()))
                const meta = await api.get(key)
                if (meta) metas.push(meta)
              }
              settle(resolve, metas.length > 0 ? metas : null)
            } catch (error) {
              settle(reject, error)
            }
          })
          try {
            // Keep the input reachable while WebKit's native picker is open.
            globalThis.document.body.appendChild(input)
            input.click()
          } catch (error) {
            settle(reject, error)
          }
        })
    }
  }
  if (!hasNativePreferences) postDesktopSettings()
  if (!hasNativePreferences && Object.prototype.hasOwnProperty.call(prefs, installSettingsKey)) {
    postInstallSettings(prefs[installSettingsKey])
  }
  syncChannels(readChannels())
}
