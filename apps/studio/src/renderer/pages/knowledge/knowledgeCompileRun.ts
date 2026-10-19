import { useEffect, useRef, useSyncExternalStore } from 'react'
import { useTranslation } from 'react-i18next'

import { cacheService } from '@data/CacheService'
import { buildAgentSessionTopicId } from '@renderer/utils/agentSession'

import {
  acceptPersonalOkf,
  pinPersonalOkf,
  planPersonalOkf,
  releasePersonalOkf,
  rememberPersonalOkf,
  type PersonalAcceptCounts
} from './compileLocalOkf'
import { type KnowledgeCompileSession, startKnowledgeCompileSession } from './knowledgeCompileAgent'

export type KnowledgeCompilePhase = 'idle' | 'running' | 'accepting' | 'done'

interface CompileStore {
  phase: KnowledgeCompilePhase
  session: KnowledgeCompileSession | null
  errorCode: '' | 'failed'
  accepted: PersonalAcceptCounts | null
  vaultRoot: string
  startedAt: number
  planned: boolean
  pending: string[] | null
}

let state: CompileStore = {
  phase: 'idle',
  session: null,
  errorCode: '',
  accepted: null,
  vaultRoot: '',
  startedAt: 0,
  planned: false,
  pending: null
}

const listeners = new Set<() => void>()
let stopWatch = () => {}
let acceptStarted = false
let generation = 0
let acceptListener: (() => void) | null = null

function emit() {
  for (const listener of listeners) listener()
}

function getSnapshot() {
  return state
}

function subscribe(listener: () => void) {
  listeners.add(listener)
  return () => listeners.delete(listener)
}

function patch(next: Partial<CompileStore>) {
  state = { ...state, ...next }
  emit()
}

function streamStatus(sessionId: string): string | undefined {
  const entry = cacheService.getShared(
    `topic.stream.statuses.${buildAgentSessionTopicId(sessionId)}` as 'topic.stream.statuses.${topicId}'
  )
  const status = entry?.status
  return typeof status === 'string' ? status : undefined
}

/** The dialog can unmount. This subscription keeps the turn's accept alive. */
function watch(sessionId: string, token: number) {
  stopWatch()
  const key = `topic.stream.statuses.${buildAgentSessionTopicId(sessionId)}` as 'topic.stream.statuses.${topicId}'
  const consider = () => {
    if (token !== generation || state.phase !== 'running' || state.session?.sessionId !== sessionId) {
      stopWatch()
      return
    }
    const status = streamStatus(sessionId)
    if (status === 'error' || status === 'aborted') {
      stopWatch()
      fail()
      return
    }
    // awaiting-approval stays open. Accept runs only after the turn is done.
    if (status !== 'done') return
    stopWatch()
    void settle()
  }
  stopWatch = cacheService.subscribe(key, consider)
  consider()
}

/** Test isolation. A mounted page must not call this when the user leaves. */
export function resetKnowledgeCompileStore() {
  generation += 1
  stopWatch()
  stopWatch = () => {}
  acceptStarted = false
  state = {
    phase: 'idle',
    session: null,
    errorCode: '',
    accepted: null,
    vaultRoot: '',
    startedAt: 0,
    planned: false,
    pending: null
  }
  emit()
}

function reset() {
  if (state.phase === 'running' || state.phase === 'accepting') return
  resetKnowledgeCompileStore()
}

async function begin(vaultRoot: string, showCompile: boolean, workspaceRoot: string) {
  if (state.phase === 'running' || state.phase === 'accepting' || !showCompile || !workspaceRoot) return
  const token = ++generation
  acceptStarted = false
  stopWatch()
  stopWatch = () => {}
  patch({
    phase: 'running',
    errorCode: '',
    session: null,
    accepted: null,
    vaultRoot,
    startedAt: Date.now(),
    planned: false,
    pending: null
  })
  let pending: string[] | null
  try {
    pending = await planPersonalOkf(vaultRoot)
  } catch {
    if (token !== generation) return
    patch({ phase: 'idle', errorCode: 'failed' })
    return
  }
  if (token !== generation || state.phase !== 'running' || state.vaultRoot !== vaultRoot) return
  patch({ planned: true, pending })
  if (pending && pending.length === 0) {
    void settle()
    return
  }
  if (pending && pending.length > 0) {
    try {
      await pinPersonalOkf(vaultRoot, pending)
    } catch {
      if (token !== generation) return
      patch({ phase: 'idle', errorCode: 'failed' })
      return
    }
    if (token !== generation || state.phase !== 'running' || state.vaultRoot !== vaultRoot) return
  }
  try {
    const session = await startKnowledgeCompileSession(vaultRoot)
    if (token !== generation || state.phase !== 'running' || state.vaultRoot !== vaultRoot) return
    patch({ session })
    watch(session.sessionId, token)
  } catch {
    if (token !== generation) return
    try {
      if (pending && pending.length > 0) await releasePersonalOkf(vaultRoot)
    } catch {
      // The run still fails. The next pin replaces this snapshot.
    }
    if (token !== generation) return
    patch({ phase: 'idle', errorCode: 'failed' })
  }
}

async function settle() {
  if (acceptStarted || state.phase !== 'running') return
  acceptStarted = true
  stopWatch()
  stopWatch = () => {}
  const vaultRoot = state.vaultRoot
  const token = generation
  const notify = acceptListener
  patch({ phase: 'accepting' })
  try {
    if (state.pending && state.pending.length > 0) {
      await releasePersonalOkf(vaultRoot)
      await rememberPersonalOkf(vaultRoot, state.pending)
    }
    const counts = await acceptPersonalOkf(vaultRoot)
    if (token !== generation) return
    patch({ accepted: counts, phase: 'done' })
    notify?.()
  } catch {
    if (token !== generation) return
    acceptStarted = false
    patch({ phase: 'idle', errorCode: 'failed' })
  }
}

function fail() {
  if (state.phase !== 'running') return
  stopWatch()
  stopWatch = () => {}
  const token = generation
  const vaultRoot = state.vaultRoot
  const pending = state.pending
  void (async () => {
    try {
      if (pending && pending.length > 0) await releasePersonalOkf(vaultRoot)
    } catch {
      // The run still fails. The next pin replaces this snapshot.
    }
    if (token !== generation || state.phase !== 'running') return
    acceptStarted = false
    patch({ phase: 'idle', errorCode: 'failed' })
  })()
}

/** Dialog phases for one personal-vault compile. Closing the page does not cancel it. */
export function useKnowledgeCompileRun(input: {
  showCompile: boolean
  workspaceRoot: string
  vaultRoot: string
  onAccepted: () => void
}) {
  const { t } = useTranslation()
  const snap = useSyncExternalStore(subscribe, getSnapshot, getSnapshot)
  const onAcceptedRef = useRef(input.onAccepted)
  onAcceptedRef.current = input.onAccepted
  useEffect(() => {
    acceptListener = () => onAcceptedRef.current()
    return () => {
      acceptListener = null
    }
  }, [])

  return {
    phase: snap.phase,
    session: snap.session,
    error: snap.errorCode ? t('knowledge.notes.compile_error') : '',
    accepted: snap.accepted,
    startedAt: snap.startedAt,
    planned: snap.planned,
    pending: snap.pending,
    begin: () => begin(input.vaultRoot, input.showCompile, input.workspaceRoot),
    settle,
    fail,
    reset
  }
}
