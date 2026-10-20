import { useEffect, useRef, useState } from 'react'

import { useSharedCacheValue } from '@renderer/data/hooks/useCache'
import { loadStudioGoalSnapshot } from '@renderer/data/studioAgentPlane'
import { readStudioRuntime } from '@shared/studioRuntime'
import { parseStudioGoalSnapshot, type StudioGoalSnapshot, unavailableStudioGoal } from '@shared/types/studioGoal'

/** The current task owns both HTTP and SSE snapshots; late requests cannot replace another task. */
export function useStudioGoal(sessionId: string | null, enabled: boolean): StudioGoalSnapshot | undefined {
  const entry = useSharedCacheValue(`topic.stream.statuses.agent-session:${sessionId ?? ''}` as const)
  const latestEntry = useRef(entry)
  latestEntry.current = entry
  const native = Boolean(readStudioRuntime())
  const active = enabled && native && sessionId !== null
  const observation = useRef({ entry, sessionId, active, epoch: 0 })
  if (
    observation.current.entry !== entry ||
    observation.current.sessionId !== sessionId ||
    observation.current.active !== active
  ) {
    observation.current = { entry, sessionId, active, epoch: observation.current.epoch + 1 }
  }
  const [loaded, setLoaded] = useState<{ snapshot: StudioGoalSnapshot; epoch: number }>()
  const terminal = entry?.status === 'done' || entry?.status === 'aborted' || entry?.status === 'error'
  const terminalIdentity = terminal ? (entry?.turnId ?? entry?.lastCompletedAt) : undefined
  useEffect(() => {
    if (!enabled || !native || !sessionId) return
    const controller = new AbortController()
    const before = latestEntry.current
    const epoch = observation.current.epoch
    void loadStudioGoalSnapshot(sessionId, controller.signal)
      .then((snapshot) => {
        if (!controller.signal.aborted && latestEntry.current === before && observation.current.epoch === epoch)
          setLoaded({ snapshot, epoch })
      })
      .catch(() => {
        if (!controller.signal.aborted && latestEntry.current === before && observation.current.epoch === epoch)
          setLoaded({ snapshot: unavailableStudioGoal(sessionId, 'request_failed'), epoch })
      })
    return () => controller.abort()
  }, [enabled, native, sessionId, terminal, terminalIdentity])

  if (!enabled || !native || !sessionId) return undefined
  if (loaded?.epoch === observation.current.epoch && loaded.snapshot.sessionId === sessionId) return loaded.snapshot
  if (entry?.goalSnapshot !== undefined) return parseStudioGoalSnapshot(entry.goalSnapshot, sessionId)
  return undefined
}
