import { emptyTraceBook, RL_TRAJECTORY_SCHEMA, type TraceBook } from './extractExecutionTrace'

export const TRACE_BOOK_KEY = 'a3s.studio.rl-trajectory.v1'

export function readTraceBook(storage: Pick<Storage, 'getItem'> | undefined = globalThis.localStorage): TraceBook {
  try {
    const raw = storage?.getItem(TRACE_BOOK_KEY)
    if (!raw) return emptyTraceBook()
    const parsed: unknown = JSON.parse(raw)
    if (!isTraceBook(parsed)) return emptyTraceBook()
    return parsed
  } catch {
    return emptyTraceBook()
  }
}

export function writeTraceBook(
  book: TraceBook,
  storage: Pick<Storage, 'setItem'> | undefined = globalThis.localStorage
): void {
  storage?.setItem(TRACE_BOOK_KEY, JSON.stringify(book))
}

function isTraceBook(value: unknown): value is TraceBook {
  if (!value || typeof value !== 'object') return false
  const book = value as Partial<TraceBook>
  if (book.version !== 2 || book.schema !== RL_TRAJECTORY_SCHEMA) return false
  if (!book.seen || typeof book.seen !== 'object' || Array.isArray(book.seen)) return false
  if (!Array.isArray(book.sessions)) return false
  return book.sessions.every(
    (session) => !!session && typeof session.sessionId === 'string' && Array.isArray(session.events)
  )
}
