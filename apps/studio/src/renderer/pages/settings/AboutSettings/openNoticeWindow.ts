import type { NoticeSlug } from './publicationLink'

interface TauriInternals {
  invoke?: (command: string, args?: Record<string, unknown>) => Promise<unknown>
}

function tauriInvoke(): TauriInternals['invoke'] | undefined {
  return (globalThis as typeof globalThis & { __TAURI_INTERNALS__?: TauriInternals }).__TAURI_INTERNALS__?.invoke
}

/** Open one published notice in a native window. Returns false outside the Tauri shell. */
export async function openNoticeWindow(input: { slug: NoticeSlug; title: string; url: string }): Promise<boolean> {
  const invoke = tauriInvoke()
  if (!invoke) return false
  try {
    await invoke('open_notice_window', { slug: input.slug, title: input.title, url: input.url })
    return true
  } catch {
    return false
  }
}
