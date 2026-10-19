const queued: File[] = []
const listeners = new Set<(files: File[]) => void>()

/** Hold a homepage drop until the composer that can attach it is mounted. */
export function queueWorkbenchDroppedFiles(files: readonly File[]): void {
  const list = [...files]
  if (list.length === 0) return
  if (listeners.size === 0) {
    queued.push(...list)
    return
  }
  const snapshot = [...list]
  for (const listener of listeners) listener(snapshot)
}

export function subscribeWorkbenchDroppedFiles(listener: (files: File[]) => void): () => void {
  listeners.add(listener)
  if (queued.length > 0) {
    const pending = queued.splice(0, queued.length)
    listener(pending)
  }
  return () => {
    listeners.delete(listener)
  }
}
