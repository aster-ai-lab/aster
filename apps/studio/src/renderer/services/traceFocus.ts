/** Session id the traces page shows until the reader clears it. */

export const TRACE_FOCUS_KEY = 'a3s.studio.traces.focus'

export function readTraceFocus(storage: Pick<Storage, 'getItem'> | undefined = globalThis.sessionStorage): string {
  try {
    return storage?.getItem(TRACE_FOCUS_KEY)?.trim() ?? ''
  } catch {
    return ''
  }
}

export function writeTraceFocus(
  sessionId: string,
  storage: Pick<Storage, 'setItem'> | undefined = globalThis.sessionStorage
): void {
  storage?.setItem(TRACE_FOCUS_KEY, sessionId)
}

export function clearTraceFocus(storage: Pick<Storage, 'removeItem'> | undefined = globalThis.sessionStorage): void {
  storage?.removeItem(TRACE_FOCUS_KEY)
}
