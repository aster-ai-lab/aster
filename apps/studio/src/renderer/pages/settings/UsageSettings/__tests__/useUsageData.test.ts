import { renderHook, waitFor } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'

const harness = vi.hoisted(() => {
  let orgId = 'org-1'
  const listeners = new Set<() => void>()
  return {
    memberRequest: vi.fn(),
    fetchOrgModels: vi.fn(),
    readCenterId: () => orgId,
    setOrgId(next: string) {
      orgId = next
      for (const listener of listeners) listener()
    },
    subscribe(listener: () => void) {
      listeners.add(listener)
      return () => {
        listeners.delete(listener)
      }
    }
  }
})

vi.mock('@renderer/data/memberOrgs', () => ({
  PERSONAL_CENTER: 'personal',
  readCenterId: () => harness.readCenterId(),
  subscribeCenter: (listener: () => void) => harness.subscribe(listener),
  memberRequest: (...args: unknown[]) => harness.memberRequest(...args),
  MemberOrgError: class MemberOrgError extends Error {
    constructor(message: string) {
      super(message)
      this.name = 'MemberOrgError'
    }
  }
}))

vi.mock('@renderer/data/orgModels', () => ({
  fetchOrgModels: () => harness.fetchOrgModels()
}))

import { MemberOrgError } from '@renderer/data/memberOrgs'

import { shiftUtcDay, usageSpan, type OrgUsageScope } from '../orgUsage'
import { useUsageData } from '../useUsageData'

const payload = {
  from_day: '2026-08-08',
  to_day: '2026-10-06',
  totals: { tokens: 10, requests: 2, succeeded: 2, failed: 0 },
  by_day: [],
  by_model: [],
  lines: [
    {
      day: '2026-10-06',
      model_id: 'model-a',
      endpoint: 'chat',
      tokens: 7,
      requests: 1,
      succeeded: 1,
      failed: 0
    },
    {
      day: '2026-08-08',
      model_id: 'model-a',
      endpoint: 'chat',
      tokens: 3,
      requests: 1,
      succeeded: 1,
      failed: 0
    }
  ]
}

describe('useUsageData', () => {
  beforeEach(() => {
    harness.setOrgId('org-1')
    harness.memberRequest.mockReset()
    harness.memberRequest.mockResolvedValue(payload)
    harness.fetchOrgModels.mockReset()
    harness.fetchOrgModels.mockResolvedValue({ items: [{ id: 'model-a', name: 'Alpha' }] })
  })

  it('reads the current window from one request and names models from the catalog', async () => {
    const { result, rerender } = renderHook(
      ({ groupBy }: { groupBy: 'model' | 'endpoint' }) => useUsageData({ windowKey: '30d', groupBy }),
      { initialProps: { groupBy: 'model' as const } }
    )

    await waitFor(() => expect(result.current.timelineLoading).toBe(false))
    expect(harness.memberRequest).toHaveBeenCalledTimes(1)
    expect(harness.memberRequest.mock.calls[0]?.[0]).toBe('/api/member/orgs/org-1/usage?days=60')
    expect(result.current.overviewTotals.totalTokens).toBe(7)
    expect(result.current.previousOverviewTotals.totalTokens).toBe(3)
    expect(result.current.hasPrevious).toBe(true)
    expect(result.current.overviewTotals.succeeded).toBe(1)
    expect(result.current.entries).toEqual([
      expect.objectContaining({ modelName: 'Alpha', endpoint: 'chat', tokens: 7 })
    ])

    rerender({ groupBy: 'endpoint' })
    expect(harness.memberRequest).toHaveBeenCalledTimes(1)
    expect(result.current.exploreTimelineRows[0]).toEqual(expect.objectContaining({ sourceId: 'chat', totalTokens: 7 }))
  })

  it('asks for 14 days when the window changes to 7 days', async () => {
    const { result, rerender } = renderHook(
      ({ windowKey }: { windowKey: '30d' | '7d' }) => useUsageData({ windowKey, groupBy: 'model' }),
      { initialProps: { windowKey: '30d' as const } }
    )

    await waitFor(() => expect(result.current.overviewTotals.totalTokens).toBe(7))
    rerender({ windowKey: '7d' })
    await waitFor(() => expect(harness.memberRequest).toHaveBeenCalledTimes(2))
    expect(harness.memberRequest.mock.calls[1]?.[0]).toBe('/api/member/orgs/org-1/usage?days=14')
  })

  it('asks for one department, then one member, when the scope changes', async () => {
    const { result, rerender } = renderHook(
      ({ scope }: { scope?: OrgUsageScope }) => useUsageData({ windowKey: '30d', groupBy: 'model', scope }),
      { initialProps: { scope: undefined as OrgUsageScope | undefined } }
    )

    await waitFor(() => expect(result.current.timelineLoading).toBe(false))
    rerender({ scope: { departmentId: 'dept-1' } })
    await waitFor(() => expect(harness.memberRequest).toHaveBeenCalledTimes(2))
    expect(harness.memberRequest.mock.calls[1]?.[0]).toBe('/api/member/orgs/org-1/usage?days=60&department_id=dept-1')
    rerender({ scope: { userId: 'user-1' } })
    await waitFor(() => expect(harness.memberRequest).toHaveBeenCalledTimes(3))
    expect(harness.memberRequest.mock.calls[2]?.[0]).toBe('/api/member/orgs/org-1/usage?days=60&user_id=user-1')
  })

  it('keeps the ledger when the model catalog fails', async () => {
    harness.fetchOrgModels.mockRejectedValue(new Error('catalog down'))
    const { result } = renderHook(() => useUsageData({ windowKey: '30d', groupBy: 'model' }))

    await waitFor(() => expect(result.current.timelineLoading).toBe(false))
    expect(result.current.error).toBeNull()
    expect(result.current.entries[0]?.modelName).toBe('model-a')
    expect(result.current.overviewTotals.totalTokens).toBe(7)
  })

  it('reads 14 days once on a fresh 7-day window and keeps the previous half', async () => {
    const span = usageSpan('7d')
    expect(span).toEqual({ days: 14, currentDays: 7 })
    const toDay = '2026-10-06'
    const previousDay = shiftUtcDay(toDay, -span.currentDays)
    const fromDay = shiftUtcDay(toDay, -(span.days - 1))
    harness.memberRequest.mockResolvedValue({
      from_day: fromDay,
      to_day: toDay,
      totals: { tokens: 10, requests: 2, succeeded: 2, failed: 0 },
      by_day: [],
      by_model: [],
      lines: [
        {
          day: toDay,
          model_id: 'model-a',
          endpoint: 'chat',
          tokens: 7,
          requests: 1,
          succeeded: 1,
          failed: 0
        },
        {
          day: previousDay,
          model_id: 'model-a',
          endpoint: 'chat',
          tokens: 3,
          requests: 1,
          succeeded: 1,
          failed: 0
        }
      ]
    })

    const { result } = renderHook(() => useUsageData({ windowKey: '7d', groupBy: 'model' }))

    await waitFor(() => expect(result.current.timelineLoading).toBe(false))
    expect(harness.memberRequest).toHaveBeenCalledTimes(1)
    expect(harness.memberRequest.mock.calls[0]?.[0]).toBe('/api/member/orgs/org-1/usage?days=14')
    expect(result.current.overviewTotals.totalTokens).toBe(7)
    expect(result.current.previousOverviewTotals.totalTokens).toBe(3)
    expect(result.current.hasPrevious).toBe(true)
    expect(result.current.error).toBeNull()
  })

  it('reads 90 days once and leaves the previous comparison empty', async () => {
    const span = usageSpan('90d')
    expect(span).toEqual({ days: 90, currentDays: 90 })
    const toDay = '2026-10-06'
    const fromDay = shiftUtcDay(toDay, -(span.days - 1))
    harness.memberRequest.mockResolvedValue({
      from_day: fromDay,
      to_day: toDay,
      totals: { tokens: 7, requests: 1, succeeded: 1, failed: 0 },
      by_day: [],
      by_model: [],
      lines: [
        {
          day: toDay,
          model_id: 'model-a',
          endpoint: 'chat',
          tokens: 7,
          requests: 1,
          succeeded: 1,
          failed: 0
        }
      ]
    })

    const { result } = renderHook(() => useUsageData({ windowKey: '90d', groupBy: 'endpoint' }))

    await waitFor(() => expect(result.current.timelineLoading).toBe(false))
    expect(harness.memberRequest).toHaveBeenCalledTimes(1)
    expect(harness.memberRequest.mock.calls[0]?.[0]).toBe('/api/member/orgs/org-1/usage?days=90')
    expect(result.current.hasPrevious).toBe(false)
    expect(result.current.overviewTotals.totalTokens).toBe(7)
    expect(result.current.previousOverviewTotals.totalTokens).toBe(0)
    expect(result.current.exploreTimelineRows[0]).toEqual(expect.objectContaining({ sourceId: 'chat', totalTokens: 7 }))
  })

  it('keeps the server message when the usage read fails', async () => {
    harness.memberRequest.mockRejectedValue(new MemberOrgError('A3S Cloud usage is not configured'))

    const { result } = renderHook(() => useUsageData({ windowKey: '30d', groupBy: 'model' }))

    await waitFor(() => expect(result.current.timelineLoading).toBe(false))
    expect(harness.memberRequest).toHaveBeenCalledTimes(1)
    expect(result.current.error).toBe('A3S Cloud usage is not configured')
    expect(result.current.needsOrg).toBe(false)
    expect(result.current.entries).toEqual([])
    expect(result.current.overviewTotals.requestCount).toBe(0)
  })

  it('does not call the member usage path from the personal center', async () => {
    harness.setOrgId('personal')
    const { result } = renderHook(() => useUsageData({ windowKey: '30d', groupBy: 'model' }))

    await waitFor(() => expect(result.current.needsOrg).toBe(true))
    expect(result.current.error).toBeNull()
    expect(harness.memberRequest).not.toHaveBeenCalled()
    expect(harness.fetchOrgModels).not.toHaveBeenCalled()
  })
})
