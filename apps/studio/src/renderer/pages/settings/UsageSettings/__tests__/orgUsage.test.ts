import { beforeEach, describe, expect, it, vi } from 'vitest'

const memberRequest = vi.hoisted(() => vi.fn())

vi.mock('@renderer/data/memberOrgs', () => ({
  PERSONAL_CENTER: 'personal',
  memberRequest,
  MemberOrgError: class MemberOrgError extends Error {
    readonly status?: number

    constructor(message: string, status?: number) {
      super(message)
      this.name = 'MemberOrgError'
      this.status = status
    }
  }
}))

import { MemberOrgError } from '@renderer/data/memberOrgs'

import { fetchOrgUsage, OrgUsageError, parseOrgUsage, sliceUsage, usagePath, usageSpan } from '../orgUsage'

const wide = {
  from_day: '2026-08-08',
  to_day: '2026-10-06',
  totals: { tokens: 10, requests: 2, succeeded: 9, failed: 1 },
  by_day: [
    { day: '2026-09-06', tokens: 4, requests: 1 },
    { day: '2026-09-07', tokens: 6, requests: 1 }
  ],
  by_model: [{ model_id: 'model-a', tokens: 10, requests: 2 }],
  lines: [
    {
      day: '2026-09-06',
      model_id: 'model-a',
      endpoint: 'chat',
      tokens: 4,
      requests: 1,
      succeeded: 1,
      failed: 0
    },
    {
      day: '2026-09-07',
      model_id: 'model-a',
      endpoint: 'chat',
      tokens: 6,
      requests: 1,
      succeeded: 1,
      failed: 0
    }
  ]
}

describe('org usage', () => {
  beforeEach(() => {
    memberRequest.mockReset()
  })

  it('asks for the previous window in the same call when the span stays within 93 days', () => {
    expect(usageSpan('7d')).toEqual({ currentDays: 7, days: 14 })
    expect(usageSpan('30d')).toEqual({ currentDays: 30, days: 60 })
    expect(usageSpan('90d')).toEqual({ currentDays: 90, days: 90 })
  })

  it('rejects a count that is not a safe integer', () => {
    expect(() => parseOrgUsage({ ...wide, totals: { ...wide.totals, tokens: 1.5 } })).toThrow(OrgUsageError)
    expect(() =>
      parseOrgUsage({
        ...wide,
        lines: [{ ...wide.lines[0], succeeded: -1 }]
      })
    ).toThrow(OrgUsageError)
  })

  it('splits a 60-day payload on the current window boundary', () => {
    const slice = sliceUsage(parseOrgUsage(wide), 30)

    expect(slice.current.map((line) => line.day)).toEqual(['2026-09-07'])
    expect(slice.previous?.map((line) => line.day)).toEqual(['2026-09-06'])
    expect(slice.outcomes).toEqual({ tokens: 6, requests: 1, succeeded: 1, failed: 0 })
    expect(slice.previousOutcomes).toEqual({ tokens: 4, requests: 1, succeeded: 1, failed: 0 })
  })

  it('does not assign the whole span to the current window when detail lines are absent', () => {
    const { lines: _lines, ...withoutLines } = wide
    const slice = sliceUsage(parseOrgUsage(withoutLines), 30)

    expect(slice.current.map((line) => line.day)).toEqual(['2026-09-07'])
    expect(slice.outcomes).toBeNull()
    expect(slice.previousOutcomes).toBeNull()
    expect(slice.modelLines).toEqual([])
  })

  it('counts a quiet doubled window as zero outcomes', () => {
    const slice = sliceUsage(
      parseOrgUsage({
        ...wide,
        totals: { tokens: 0, requests: 0, succeeded: 0, failed: 0 },
        by_day: [],
        by_model: [],
        lines: []
      }),
      30
    )

    expect(slice.outcomes).toEqual({ succeeded: 0, failed: 0 })
    expect(slice.previousOutcomes).toBeNull()
    expect(slice.modelLines).toEqual([])
  })

  it('keeps outcome totals when the response covers only the current window', () => {
    const slice = sliceUsage(
      parseOrgUsage({
        ...wide,
        from_day: '2026-09-07',
        lines: [],
        by_day: [{ day: '2026-09-07', tokens: 6, requests: 1 }]
      }),
      30
    )

    expect(slice.previous).toBeNull()
    expect(slice.outcomes).toEqual({ succeeded: 9, failed: 1 })
    expect(slice.modelLines.map((line) => line.modelId)).toEqual(['model-a'])
  })

  it('loads one member usage path and keeps the server message', async () => {
    memberRequest.mockResolvedValueOnce(wide)
    await expect(fetchOrgUsage('org-1', 60, fetch)).resolves.toMatchObject({ toDay: '2026-10-06' })
    expect(memberRequest).toHaveBeenCalledWith('/api/member/orgs/org-1/usage?days=60', { method: 'GET' }, fetch)

    memberRequest.mockRejectedValueOnce(new MemberOrgError('A3S Cloud usage is not configured', 503))
    await expect(fetchOrgUsage('org-1', 60, fetch)).rejects.toMatchObject({
      code: 'load',
      message: 'A3S Cloud usage is not configured'
    })

    await expect(fetchOrgUsage('personal', 60, fetch)).rejects.toMatchObject({ code: 'org' })
    await expect(fetchOrgUsage('org-1', 180, fetch)).rejects.toMatchObject({ code: 'invalid' })
    expect(usagePath('org-1', 60, { departmentId: 'dept-1' })).toBe(
      '/api/member/orgs/org-1/usage?days=60&department_id=dept-1'
    )
    expect(usagePath('org-1', 60, { userId: 'user-1', departmentId: 'dept-1' })).toBe(
      '/api/member/orgs/org-1/usage?days=60&user_id=user-1'
    )
    expect(memberRequest).toHaveBeenCalledTimes(2)
  })
})
