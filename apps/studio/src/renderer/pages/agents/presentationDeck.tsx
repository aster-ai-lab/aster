import i18next from 'i18next'
import { useEffect, useRef, useSyncExternalStore } from 'react'

import { writeWorkbenchSidebarCollapsed } from '@renderer/layout/pageSidebar'

import type { HomeComposerDeck } from './homeComposerMode'

export type PresentationDeck = HomeComposerDeck & {
  sessionId: string
  outline: readonly string[] | null
}

type Listener = () => void

const listeners = new Set<Listener>()

// shortcut: the deck lives in this renderer process; a reload must rebuild it from the task transcript.
let deck: PresentationDeck | null = null
let viewingSessionId: string | null = null

function emit() {
  for (const listener of listeners) listener()
}

function subscribe(listener: Listener) {
  listeners.add(listener)
  return () => listeners.delete(listener)
}

export function readPresentationDeck(): PresentationDeck | null {
  return deck
}

export function clearPresentationDeck() {
  deck = null
  viewingSessionId = null
  emit()
}

export function resetPresentationDeckForTests() {
  clearPresentationDeck()
}

export function usePresentationDeck(): PresentationDeck | null {
  return useSyncExternalStore(subscribe, readPresentationDeck, readPresentationDeck)
}

/** The task that should show the presentation work mode. */
export function armPresentationDeck(sessionId: string, next: HomeComposerDeck) {
  viewingSessionId = sessionId
  deck = { ...next, sessionId, outline: null }
  emit()
}

function setPresentationView(sessionId: string | null) {
  viewingSessionId = sessionId
}

function outlineFromJson(source: string): readonly string[] | null {
  const start = source.indexOf('{')
  const end = source.lastIndexOf('}')
  if (start < 0 || end <= start) return null
  let parsed: unknown
  try {
    parsed = JSON.parse(source.slice(start, end + 1))
  } catch {
    return null
  }
  if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) return null
  const record = parsed as { step?: unknown; outline?: unknown }
  const outline = record.outline
  if (record.step !== 'outline' || !Array.isArray(outline) || outline.length === 0) return null
  if (!outline.every((item) => typeof item === 'string' && item.trim().length > 0)) return null
  return outline
}

/** The presenton skill's outline step: one JSON object, optionally inside a fence. */
export function outlineFromReply(text: string): readonly string[] | null {
  const fence = /```(?:json)?\s*([\s\S]*?)```/.exec(text)
  return outlineFromJson(fence?.[1] ?? text)
}

export function notePresentationAgentProse(text: string): 'applied' | 'idle' {
  if (!deck || viewingSessionId !== deck.sessionId) return 'idle'
  const outline = outlineFromReply(text)
  if (!outline) return 'idle'
  if (
    deck.outline &&
    deck.outline.length === outline.length &&
    deck.outline.every((line, index) => line === outline[index])
  ) {
    return 'idle'
  }
  deck = { ...deck, outline }
  emit()
  return 'applied'
}

function slideHeading(markdown: string): string {
  const line = markdown.split('\n')[0]?.trim() ?? ''
  const heading = line.replace(/^#{1,6}\s+/, '').trim()
  return heading || line
}

function slideBody(markdown: string): string {
  const lines = markdown.split('\n')
  const first = lines[0]?.trim() ?? ''
  if (!/^#{1,6}\s+/.test(first)) return markdown.trim()
  return lines.slice(1).join('\n').trim()
}

/** Outline work mode for the task that was started from the presentation composer. */
export function PresentationSessionDeck({ sessionId }: { sessionId: string }) {
  const current = usePresentationDeck()
  const visible = current?.sessionId === sessionId
  useEffect(() => {
    if (!visible) return undefined
    setPresentationView(sessionId)
    return () => setPresentationView(null)
  }, [sessionId, visible])
  if (!current || !visible) return null
  const ready = current.outline !== null
  const status = i18next.t(ready ? 'workbench.presentation.ready' : 'workbench.presentation.preparing')
  return (
    <section
      aria-label={current.title}
      data-ui="workbench.presentation"
      className="flex h-full min-h-0 w-full flex-col gap-3 overflow-y-auto px-4 py-4">
      <div className="flex flex-col gap-1">
        <p className="text-[13px] leading-5 text-[#8a8a8a]">{current.title}</p>
        <h2 className="text-[15px] leading-5 font-medium text-[#171717]">{current.topic}</h2>
      </div>
      {current.lines.length > 0 ? (
        <ul className="m-0 flex list-none flex-wrap gap-2 p-0">
          {current.lines.map((line) => (
            <li key={line} className="rounded-lg bg-[#f5f5f5] px-2.5 py-1 text-[13px] leading-5 text-[#171717]">
              {line}
            </li>
          ))}
        </ul>
      ) : null}
      <p data-presentation-status={ready ? 'ready' : 'outline'} className="m-0 text-[13px] leading-5 text-[#8a8a8a]">
        {status}
      </p>
      {current.outline ? (
        <ol className="m-0 flex list-none flex-col gap-2 p-0">
          {current.outline.map((slide, index) => {
            const body = slideBody(slide)
            return (
              <li
                key={`${index}:${slideHeading(slide)}`}
                data-presentation-slide={index + 1}
                className="flex gap-3 rounded-lg bg-[#f5f5f5] px-3 py-3">
                <span className="grid size-[22px] shrink-0 place-items-center rounded-md bg-white text-[12px] font-medium text-[#171717]">
                  {index + 1}
                </span>
                <div className="min-w-0">
                  <p className="m-0 text-[13px] leading-5 font-medium text-[#171717]">{slideHeading(slide)}</p>
                  {body ? (
                    <p className="m-0 mt-1 text-[13px] leading-5 whitespace-pre-wrap text-[#6f6f6f]">{body}</p>
                  ) : null}
                </div>
              </li>
            )
          })}
        </ol>
      ) : null}
    </section>
  )
}

/** Collapse the work-page sidebar once when this task is the presentation session. */
export function collapsePresentationSidebar(
  sessionId: string | undefined,
  deckSessionId: string | undefined,
  collapsedFor: string | null
): string | null {
  if (!sessionId || sessionId !== deckSessionId || collapsedFor === sessionId) return null
  return sessionId
}

/** Write the collapsed preference once, then leave a later expand in place. */
export function syncPresentationSidebar(
  sessionId: string | undefined,
  deckSessionId: string | undefined,
  collapsedFor: string | null
): string | null {
  const next = collapsePresentationSidebar(sessionId, deckSessionId, collapsedFor)
  if (!next) return collapsedFor
  writeWorkbenchSidebarCollapsed(true)
  return next
}

/** Opens the task's right workspace while this session is in the presentation work mode. */
export function PresentationWorkspaceOpener({
  sessionId,
  ready = true,
  openWorkspace
}: {
  sessionId?: string
  ready?: boolean
  openWorkspace: () => void
}) {
  const current = usePresentationDeck()
  const open = current?.sessionId === sessionId
  const openRef = useRef(openWorkspace)
  openRef.current = openWorkspace
  useEffect(() => {
    if (open && ready) openRef.current()
  }, [open, ready])
  return null
}
