import { useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { ToWorkWordmark } from '@renderer/components/ToWorkWordmark'

/** One left-to-right write. The CSS draws once in the same 500ms, then hands over to the ring. */
const HOLD_MS = 500
const FADE_MS = 180

const seenEntries = new Set<string>()
const inflightEntries = new Map<string, number>()
let entrySeq = 0

/** Drop first-entry memory. Tests start from a fresh session. */
export function clearPageEntrySession() {
  seenEntries.clear()
  inflightEntries.clear()
}

function beginPageEntry(id: string): number | null {
  if (seenEntries.has(id) || inflightEntries.has(id)) return null
  const token = ++entrySeq
  inflightEntries.set(id, token)
  return token
}

/**
 * One contain loader for a view entry.
 * The loader stays until `ready`, and at least HOLD_MS from mount, then fades.
 * Each `id` plays once per session.
 */
export function PageEntry({ ready = true, id }: { ready?: boolean; id: string }) {
  const { t } = useTranslation()
  const token = useRef<number | null | undefined>(undefined)
  if (token.current === undefined) token.current = beginPageEntry(id)
  const [phase, setPhase] = useState<'drawing' | 'leaving' | 'hidden'>('drawing')
  const started = useRef<number | null>(null)
  const leaveAt = useRef<number | null>(null)
  if (started.current === null) started.current = performance.now()

  useEffect(() => {
    const claim = token.current
    if (claim == null) return
    inflightEntries.set(id, claim)
    const commit = window.setTimeout(() => {
      if (inflightEntries.get(id) !== claim) return
      inflightEntries.delete(id)
      seenEntries.add(id)
    }, 0)
    return () => {
      window.clearTimeout(commit)
      if (inflightEntries.get(id) === claim) inflightEntries.delete(id)
    }
  }, [id])

  useEffect(() => {
    if (token.current == null) return
    if (leaveAt.current === null) {
      if (!ready) return
      leaveAt.current = performance.now() + Math.max(0, HOLD_MS - (performance.now() - started.current!))
    }
    const wait = Math.max(0, leaveAt.current - performance.now())
    const leave = window.setTimeout(() => setPhase('leaving'), wait)
    const hide = window.setTimeout(() => setPhase('hidden'), wait + FADE_MS)
    return () => {
      window.clearTimeout(leave)
      window.clearTimeout(hide)
    }
  }, [ready])

  if (token.current == null || phase === 'hidden') return null
  return <ToWorkPageLoading label={t('common.loading')} leaving={phase === 'leaving'} />
}

/** Same wordmark loader for a region that is not a route entry. */
export function ToWorkPageLoading({ label, leaving = false }: { label: string; leaving?: boolean }) {
  return (
    <div
      className={leaving ? 'page-loader page-loader--contain is-leaving' : 'page-loader page-loader--contain'}
      style={{ zIndex: 40 }}
      role="status"
      aria-live="polite"
      aria-label={label}
      aria-busy={leaving ? 'false' : 'true'}>
      <div className="page-loader-spinner-wrap page-loader-handoff">
        <ToWorkWordmark drawn />
        <span className="page-loader-ring">
          <PageLoaderRing />
        </span>
      </div>
    </div>
  )
}

/** The ink ring for a wait whose end is unknown. */
export function PageLoaderRing({ size = 56 }: { size?: number }) {
  return (
    <svg
      data-ui="page.loader.ring"
      className="page-loader-spinner"
      viewBox="0 0 48 48"
      width={size}
      height={size}
      aria-hidden>
      <circle
        cx="24"
        cy="24"
        r="18"
        fill="none"
        stroke="currentColor"
        strokeWidth="3"
        strokeLinecap="round"
        strokeDasharray="28 85"
      />
    </svg>
  )
}

/** Pages that paint their own entry loader. A shell loader on top would stack two marks. */
const PAGE_OWNED_ENTRY = /^\/app\/(agents|capabilities|org|platform|plugin)(?:\/|$)/

export function appShellPaintsEntry(pathname: string): boolean {
  return !PAGE_OWNED_ENTRY.test(pathname)
}

/** The work page stays on its entry loader until agents, the open session, and the task list have settled. */
export function workbenchEntryReady(flags: {
  agentsLoading: boolean
  sessionLoading: boolean
  sessionsLoading: boolean
  pinsLoading: boolean
}): boolean {
  return !flags.agentsLoading && !flags.sessionLoading && !flags.sessionsLoading && !flags.pinsLoading
}
