import { useRef, useState } from 'react'

import type { MessageQuote } from '@renderer/im/quote'

const STORAGE_KEY = 'messages.composer.drafts.v1'
const MAX_DRAFT_CHARS = 20_000

export type ComposerShot = { id: string; file: File; preview?: string }

type StorageLike = Pick<Storage, 'getItem' | 'setItem' | 'removeItem'>

export function readComposerDrafts(storage: Pick<Storage, 'getItem'> = localStorage): Record<string, string> {
  try {
    const raw = storage.getItem(STORAGE_KEY)
    if (!raw) return {}
    const parsed: unknown = JSON.parse(raw)
    if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) return {}
    const drafts: Record<string, string> = {}
    for (const [id, value] of Object.entries(parsed)) {
      if (typeof value === 'string' && value.length > 0 && value.length <= MAX_DRAFT_CHARS) drafts[id] = value
    }
    return drafts
  } catch {
    return {}
  }
}

export function writeComposerDrafts(drafts: Record<string, string>, storage: StorageLike = localStorage): void {
  const kept: Record<string, string> = {}
  for (const [id, value] of Object.entries(drafts)) {
    if (value.length > 0 && value.length <= MAX_DRAFT_CHARS) kept[id] = value
  }
  if (Object.keys(kept).length === 0) storage.removeItem(STORAGE_KEY)
  else storage.setItem(STORAGE_KEY, JSON.stringify(kept))
}

/**
 * Text survives a reload. Files and quotes stay in memory, one map per conversation,
 * so switching chats does not carry them across.
 */
export function useConversationComposer(storage: StorageLike = localStorage) {
  const drafts = useRef(readComposerDrafts(storage))
  const shots = useRef<Record<string, ComposerShot[]>>({})
  const quotes = useRef<Record<string, MessageQuote | null>>({})
  const active = useRef<string | null>(null)
  const [text, setText] = useState('')
  const textRef = useRef('')
  const [files, setFiles] = useState<ComposerShot[]>([])
  const filesRef = useRef<ComposerShot[]>([])
  const [quote, setQuote] = useState<MessageQuote | null>(null)
  const quoteRef = useRef<MessageQuote | null>(null)

  const remember = () => {
    const id = active.current
    if (!id) return
    if (textRef.current) drafts.current[id] = textRef.current
    else delete drafts.current[id]
    writeComposerDrafts(drafts.current, storage)
    if (filesRef.current.length > 0) shots.current[id] = filesRef.current
    else delete shots.current[id]
    if (quoteRef.current) quotes.current[id] = quoteRef.current
    else delete quotes.current[id]
  }

  const show = (id: string | null) => {
    const nextText = id ? (drafts.current[id] ?? '') : ''
    const nextFiles = id ? (shots.current[id] ?? []) : []
    const nextQuote = id ? (quotes.current[id] ?? null) : null
    textRef.current = nextText
    filesRef.current = nextFiles
    quoteRef.current = nextQuote
    setText(nextText)
    setFiles(nextFiles)
    setQuote(nextQuote)
  }

  const focus = (id: string | null) => {
    if (active.current === id) return
    remember()
    active.current = id
    show(id)
  }

  const writeText = (id: string, value: string | ((current: string) => string)) => {
    const current = active.current === id ? textRef.current : (drafts.current[id] ?? '')
    const next = typeof value === 'function' ? value(current) : value
    if (next) drafts.current[id] = next
    else delete drafts.current[id]
    writeComposerDrafts(drafts.current, storage)
    if (active.current === id) {
      textRef.current = next
      setText(next)
    }
  }

  const writeFiles = (id: string, value: ComposerShot[] | ((current: ComposerShot[]) => ComposerShot[])) => {
    const current = active.current === id ? filesRef.current : (shots.current[id] ?? [])
    const next = typeof value === 'function' ? value(current) : value
    if (next.length > 0) shots.current[id] = next
    else delete shots.current[id]
    if (active.current === id) {
      filesRef.current = next
      setFiles(next)
    }
  }

  const writeQuote = (id: string, next: MessageQuote | null) => {
    if (next) quotes.current[id] = next
    else delete quotes.current[id]
    if (active.current === id) {
      quoteRef.current = next
      setQuote(next)
    }
  }

  const dropFiles = (id: string) => {
    const current = active.current === id ? filesRef.current : (shots.current[id] ?? [])
    for (const shot of current) {
      if (shot.preview) URL.revokeObjectURL(shot.preview)
    }
    delete shots.current[id]
    if (active.current === id) {
      filesRef.current = []
      setFiles([])
    }
  }

  /** Keep what the member is typing when a placeholder conversation id is replaced. */
  const handoff = (from: string, to: string) => {
    if (from === to) return
    const visible = active.current === from
    const carry = visible && (textRef.current.length > 0 || filesRef.current.length > 0 || quoteRef.current !== null)
    if (!carry) {
      focus(to)
      return
    }
    if (textRef.current) drafts.current[to] = textRef.current
    else delete drafts.current[to]
    delete drafts.current[from]
    writeComposerDrafts(drafts.current, storage)
    if (filesRef.current.length > 0) shots.current[to] = filesRef.current
    delete shots.current[from]
    if (quoteRef.current) quotes.current[to] = quoteRef.current
    delete quotes.current[from]
    active.current = to
  }

  return {
    text,
    files,
    quote,
    focus,
    writeText,
    writeFiles,
    writeQuote,
    dropFiles,
    handoff,
    active: () => active.current
  }
}
