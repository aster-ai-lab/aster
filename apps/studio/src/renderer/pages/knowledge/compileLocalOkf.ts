export const OKF_DIR = 'wiki'
export const OKF_INDEX = 'index.md'

interface TauriInternals {
  invoke?: (command: string, args?: Record<string, unknown>) => Promise<unknown>
}

function tauriInternals(): TauriInternals | undefined {
  return (globalThis as typeof globalThis & { __TAURI_INTERNALS__?: TauriInternals }).__TAURI_INTERNALS__
}

function tauriInvoke(): TauriInternals['invoke'] | undefined {
  return tauriInternals()?.invoke
}

export interface PersonalAcceptCounts {
  concepts: number
  skipped: number
  failed: number
}

function personalAcceptCounts(value: unknown): PersonalAcceptCounts | null {
  if (!value || typeof value !== 'object') return null
  const record = value as Record<string, unknown>
  const concepts = record.concepts
  const skipped = record.skipped
  const failed = record.failed
  if (typeof concepts !== 'number' || typeof skipped !== 'number' || typeof failed !== 'number') return null
  if (!Number.isFinite(concepts) || !Number.isFinite(skipped) || !Number.isFinite(failed)) return null
  return { concepts, skipped, failed }
}

function pendingPaths(value: unknown): string[] | null {
  if (!value || typeof value !== 'object') return null
  const pending = (value as { pending?: unknown }).pending
  if (!Array.isArray(pending) || pending.some((item) => typeof item !== 'string')) return null
  return pending
}

/**
 * Notes that still need a concept page.
 * Outside the app shell there is no plan, and the compile turn stays the whole vault.
 */
export async function planPersonalOkf(root: string): Promise<string[] | null> {
  const invoke = tauriInvoke()
  if (!invoke) return null
  const pending = pendingPaths(await invoke('plan_personal_okf', { root }))
  if (!pending) throw new Error('compile plan is incomplete')
  return pending
}

/** Copy notes and concept pages outside this plan. The copy is not inside the vault. */
export async function pinPersonalOkf(root: string, pending: readonly string[]): Promise<void> {
  const invoke = tauriInvoke()
  if (!invoke) throw new Error('pin requires the app shell')
  await invoke('pin_personal_okf', { root, pending })
}

/** Put pinned files back, then drop the copy. */
export async function releasePersonalOkf(root: string): Promise<void> {
  const invoke = tauriInvoke()
  if (!invoke) return
  await invoke('release_personal_okf', { root })
}

/** Record generation on the concept pages written for this plan. Notes stay untouched. */
export async function rememberPersonalOkf(root: string, pending: readonly string[]): Promise<void> {
  const invoke = tauriInvoke()
  if (!invoke) return
  await invoke('remember_personal_okf', { root, pending })
}

/** Ask the shell to project this vault's index. The page does not write the index itself. */
export async function acceptPersonalOkf(root: string): Promise<PersonalAcceptCounts> {
  const invoke = tauriInvoke()
  if (!invoke) throw new Error('accept requires the app shell')
  const counts = personalAcceptCounts(await invoke('accept_personal_okf', { root }))
  if (!counts) throw new Error('accept report is incomplete')
  return counts
}

/** Ask the app shell to compile the workspace. The page does not compile on its own. */
export async function compileWorkspaceOkf(root: string): Promise<void> {
  const invoke = tauriInvoke()
  if (!invoke) throw new Error('compile requires the app shell')
  await invoke('compile_workspace_okf', { root })
}

export interface WorkspaceSearch {
  phase: 'not-ready' | 'published'
  hits: Array<{ name: string; passage: string }>
}

/** Search the session WikiIndex. Outside the app shell there is no published generation. */
export async function searchWorkspaceOkf(root: string, query: string): Promise<WorkspaceSearch | null> {
  const invoke = tauriInvoke()
  if (!invoke) return null
  const result = (await invoke('search_workspace_okf', { root, query })) as WorkspaceSearch
  return result?.phase === 'not-ready' || result?.phase === 'published' ? result : null
}
