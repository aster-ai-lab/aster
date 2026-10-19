import { useSyncExternalStore } from 'react'

export type WikiRetrievalPhase = 'idle' | 'preparing' | 'failed'

let phase: WikiRetrievalPhase = 'idle'
const listeners = new Set<() => void>()

function emit() {
  for (const listener of listeners) listener()
}

export function publishWikiRetrieval(next: WikiRetrievalPhase) {
  if (phase === next) return
  phase = next
  emit()
}

export function resetWikiRetrievalForTests() {
  phase = 'idle'
  emit()
}

export function useWikiRetrievalPhase(): WikiRetrievalPhase {
  return useSyncExternalStore(
    (listener) => {
      listeners.add(listener)
      return () => listeners.delete(listener)
    },
    () => phase,
    () => 'idle'
  )
}
