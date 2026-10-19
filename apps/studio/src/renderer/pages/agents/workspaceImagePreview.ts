import { useEffect, useState } from 'react'

import { previewKind, previewMime } from '@renderer/pages/documents/documentPreview'
import { readBudgetBytes } from '@renderer/pages/documents/fileOpenBudget'
import { readLocalDocumentBytes } from '@renderer/pages/documents/useLocalDocuments'

type Slot = { url: string; refs: number }

const cache = new Map<string, Slot>()
const inflight = new Map<string, Promise<string | null>>()
const waiting = new Map<string, number>()

function cacheKey(root: string, path: string): string {
  return `${root}\n${path}`
}

/** Workspace images paint from an authenticated read. A `file:` URL does not. */
export function retainWorkspaceImageUrl(root: string, path: string, name: string): Promise<string | null> {
  if (!root || !path || previewKind(name) !== 'image') return Promise.resolve(null)
  const id = cacheKey(root, path)
  const existing = cache.get(id)
  if (existing) {
    existing.refs += 1
    return Promise.resolve(existing.url)
  }
  waiting.set(id, (waiting.get(id) ?? 0) + 1)
  let pending = inflight.get(id)
  if (!pending) {
    pending = readLocalDocumentBytes(root, path, undefined, readBudgetBytes(name))
      .then((bytes) => {
        const mime = previewMime(name) ?? 'application/octet-stream'
        return URL.createObjectURL(new Blob([bytes], { type: mime }))
      })
      .catch(() => null)
      .then((url) => {
        inflight.delete(id)
        const count = waiting.get(id) ?? 0
        waiting.delete(id)
        if (!url || count <= 0) {
          if (url) URL.revokeObjectURL(url)
          return null
        }
        cache.set(id, { url, refs: count })
        return url
      })
    inflight.set(id, pending)
  }
  return pending
}

export function releaseWorkspaceImageUrl(root: string, path: string): void {
  const id = cacheKey(root, path)
  const slot = cache.get(id)
  if (!slot) {
    const count = waiting.get(id)
    if (count === undefined) return
    waiting.set(id, count - 1)
    return
  }
  slot.refs -= 1
  if (slot.refs > 0) return
  URL.revokeObjectURL(slot.url)
  cache.delete(id)
}

/** Object URL for one workspace image, or null while it is unread. */
export function useWorkspaceImageSrc(root: string, path: string | undefined, name: string): string | null {
  const [src, setSrc] = useState<string | null>(null)
  useEffect(() => {
    if (!root || !path || previewKind(name) !== 'image') {
      setSrc(null)
      return
    }
    let active = true
    let held = false
    setSrc(null)
    void retainWorkspaceImageUrl(root, path, name).then((url) => {
      if (!active) {
        if (url) releaseWorkspaceImageUrl(root, path)
        return
      }
      if (!url) {
        setSrc(null)
        return
      }
      held = true
      setSrc(url)
    })
    return () => {
      active = false
      if (held) releaseWorkspaceImageUrl(root, path)
    }
  }, [name, path, root])
  return src
}
