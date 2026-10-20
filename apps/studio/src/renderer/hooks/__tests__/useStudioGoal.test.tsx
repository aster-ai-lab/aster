import { act, renderHook } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import type { TopicStatusSnapshotEntry } from '@shared/ai/transport'
import { parseStudioGoalSnapshot, type StudioGoalSnapshot } from '@shared/types/studioGoal'

const fixture = vi.hoisted(() => ({ entry: undefined as TopicStatusSnapshotEntry | undefined, load: vi.fn() }))
vi.mock('@renderer/data/hooks/useCache', () => ({ useSharedCacheValue: () => fixture.entry }))
vi.mock('@renderer/data/studioAgentPlane', () => ({
  loadStudioGoalSnapshot: (...args: unknown[]) => fixture.load(...args)
}))
vi.mock('@shared/studioRuntime', () => ({ readStudioRuntime: () => ({ shell: 'tauri' }) }))

import { useStudioGoal } from '../useStudioGoal'

function snapshot(sessionId = 's1', state: 'active' | 'completed' = 'active'): StudioGoalSnapshot {
  return {
    status: 'known',
    sessionId,
    acpSessionId: 'acp-1',
    executionId: 'run-1',
    goal: {
      goalId: 'g',
      goalRevision: 2,
      planRevision: 1,
      goalSequence: 4,
      objective: 'Read inputs and verify outputs',
      state,
      tick: state === 'completed' ? 'complete' : 'advance'
    }
  }
}

function deferred() {
  let resolve!: (value: StudioGoalSnapshot) => void
  const promise = new Promise<StudioGoalSnapshot>((done) => {
    resolve = done
  })
  return { promise, resolve }
}

beforeEach(() => {
  fixture.entry = undefined
  fixture.load.mockReset()
})

describe('current Studio goal snapshots', () => {
  it('distinguishes authoritative no-goal from unavailable and rejects foreign/unknown/unsafe snapshots', () => {
    const none = { ...snapshot(), goal: null }
    expect(parseStudioGoalSnapshot(none, 's1')).toEqual(none)
    expect(parseStudioGoalSnapshot(snapshot('foreign'), 's1')).toMatchObject({ status: 'unavailable', goal: null })
    const value = snapshot()
    for (const goal of [
      { ...value.goal, state: 'invented' },
      { ...value.goal, tick: null },
      { ...value.goal, goalRevision: Number.MAX_SAFE_INTEGER + 1 }
    ]) {
      expect(parseStudioGoalSnapshot({ ...value, goal }, 's1')).toMatchObject({ status: 'unavailable', goal: null })
    }
  })

  it('loads the current native goal without historical commands and clears terminal UI state', async () => {
    fixture.load.mockResolvedValue(snapshot())
    const { result, rerender } = renderHook(() => useStudioGoal('s1', true))
    await act(async () => {})
    expect(result.current).toEqual(snapshot())
    fixture.entry = {
      status: 'done',
      activeExecutions: [],
      awaitingApprovalAnchors: [],
      goalSnapshot: snapshot('s1', 'completed')
    }
    fixture.load.mockResolvedValue(snapshot('s1', 'completed'))
    await act(async () => rerender())
    expect(result.current?.goal?.state).toBe('completed')
  })

  it('does not revive a prior HTTP-only goal after leaving and re-entering the task controls', async () => {
    fixture.load.mockResolvedValueOnce(snapshot())
    const next = deferred()
    fixture.load.mockReturnValueOnce(next.promise)
    const { result, rerender } = renderHook(({ enabled }) => useStudioGoal('s1', enabled), {
      initialProps: { enabled: true }
    })
    await act(async () => {})
    expect(result.current?.goal?.state).toBe('active')
    rerender({ enabled: false })
    expect(result.current).toBeUndefined()
    rerender({ enabled: true })
    expect(result.current).toBeUndefined()
    await act(async () => next.resolve({ ...snapshot(), goal: null }))
    expect(result.current?.goal).toBeNull()
  })

  it('rejects an old task response after switching sessions', async () => {
    const old = deferred(),
      next = deferred()
    fixture.load.mockReturnValueOnce(old.promise).mockReturnValueOnce(next.promise)
    const { result, rerender } = renderHook(({ id }) => useStudioGoal(id, true), { initialProps: { id: 's1' } })
    rerender({ id: 's2' })
    await act(async () => old.resolve(snapshot()))
    expect(result.current).toBeUndefined()
    await act(async () => next.resolve(snapshot('s2')))
    expect(result.current?.sessionId).toBe('s2')
  })

  it('lets a newer SSE snapshot win and never revives an old GET across cache ABA', async () => {
    const pending = deferred()
    fixture.load.mockReturnValue(pending.promise)
    const { result, rerender } = renderHook(() => useStudioGoal('s1', true))
    fixture.entry = {
      status: 'streaming',
      activeExecutions: [],
      awaitingApprovalAnchors: [],
      goalSnapshot: { ...snapshot(), goal: null }
    }
    rerender()
    expect(result.current?.goal).toBeNull()
    fixture.entry = undefined
    rerender()
    await act(async () => pending.resolve(snapshot()))
    expect(result.current).toBeUndefined()
  })
})
