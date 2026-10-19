import { quarkSearchQuery } from '@renderer/utils/normalizeWebviewAddress'

interface TauriInvoke {
  (command: string, args?: Record<string, unknown>): Promise<unknown>
}

const QUARK_SUGGESTION_LIMIT = 8

export interface WorkspaceBrowserPlacement {
  url: string
  x: number
  y: number
  width: number
  height: number
  visible: boolean
  reloadKey: number
}

function tauriInvoke(): TauriInvoke | undefined {
  const internals = (globalThis as { __TAURI_INTERNALS__?: { invoke?: TauriInvoke } }).__TAURI_INTERNALS__
  if (typeof internals?.invoke === 'function') return internals.invoke
  const tauri = (globalThis as { __TAURI__?: { core?: { invoke?: TauriInvoke } } }).__TAURI__
  if (typeof tauri?.core?.invoke === 'function') return tauri.core.invoke
  return undefined
}

/** The signed-in ToWork window is Tauri. It embeds the page in the workspace pane. */
export function hostUsesA3sWebview(): boolean {
  return tauriInvoke() !== undefined
}

/** http(s) and file URLs only. Credentialed and blank pages stay in the address bar. */
export function workspaceBrowserUrl(value: string): string | undefined {
  try {
    const url = new URL(value)
    if (url.username || url.password) return undefined
    if (url.protocol !== 'http:' && url.protocol !== 'https:' && url.protocol !== 'file:') return undefined
    return url.toString()
  } catch {
    return undefined
  }
}

/** Words from Quark. A web address, a missing host, or a failed request returns no words. */
export async function quarkSearchSuggestions(query: string): Promise<string[]> {
  const trimmed = quarkSearchQuery(query)
  const invoke = tauriInvoke()
  if (!trimmed || !invoke) return []
  try {
    const value = await invoke('quark_search_suggestions', { query: trimmed })
    if (!Array.isArray(value)) return []
    const words: string[] = []
    for (const item of value) {
      if (typeof item !== 'string') continue
      const word = item.trim()
      if (!word || word.length > 200 || words.includes(word)) continue
      words.push(word)
      if (words.length === QUARK_SUGGESTION_LIMIT) break
    }
    return words
  } catch {
    return []
  }
}

export interface WorkspaceBrowserNav {
  url: string
  canGoBack: boolean
  canGoForward: boolean
  loading: boolean
  title: string | null
}

/** Back, forward, stop, and the live address of the embedded page. A missing page returns null. */
export async function workspaceBrowserNavigation(
  action: 'state' | 'back' | 'forward' | 'stop'
): Promise<WorkspaceBrowserNav | null> {
  const invoke = tauriInvoke()
  if (!invoke) return null
  try {
    const value = await invoke('workspace_browser_navigate', { action })
    if (!value || typeof value !== 'object') return null
    const record = value as Record<string, unknown>
    if (typeof record.url !== 'string') return null
    if (typeof record.canGoBack !== 'boolean' || typeof record.canGoForward !== 'boolean') return null
    const title = typeof record.title === 'string' ? record.title.trim().slice(0, 300) : null
    return {
      url: record.url,
      canGoBack: record.canGoBack,
      canGoForward: record.canGoForward,
      loading: record.loading === true,
      title
    }
  } catch {
    return null
  }
}

export function launchWorkspaceBrowser(sessionId: string, placement: WorkspaceBrowserPlacement): Promise<void> {
  const invoke = tauriInvoke()
  if (!invoke) return Promise.reject(new Error('workspace browser host is unavailable'))
  return Promise.resolve(invoke('launch_workspace_browser', { sessionId, ...placement })).then(() => undefined)
}

let noticeCovered = false
const noticeCoverListeners = new Set<() => void>()

/** True while a notice dialog owns the foreground page. The workspace pane stays hidden. */
export function readNoticeWebviewCover(): boolean {
  return noticeCovered
}

export function subscribeNoticeWebviewCover(listener: () => void): () => void {
  noticeCoverListeners.add(listener)
  return () => noticeCoverListeners.delete(listener)
}

export function setNoticeWebviewCover(covered: boolean): void {
  if (noticeCovered === covered) return
  noticeCovered = covered
  for (const listener of noticeCoverListeners) listener()
}

/** Place the notice child over a dialog. This does not move the workspace pane. */
export function launchNoticeBrowser(placement: WorkspaceBrowserPlacement): Promise<void> {
  const invoke = tauriInvoke()
  if (!invoke) return Promise.reject(new Error('workspace browser host is unavailable'))
  const { url, x, y, width, height, visible, reloadKey } = placement
  return Promise.resolve(invoke('launch_notice_browser', { url, x, y, width, height, visible, reloadKey })).then(
    () => undefined
  )
}
