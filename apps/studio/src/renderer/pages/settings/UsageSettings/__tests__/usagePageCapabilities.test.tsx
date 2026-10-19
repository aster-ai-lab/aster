import type * as CherryStudioUi from '@a3s-lab/ui'
import { MockUseCacheUtils } from '@test-mocks/renderer/useCache'
import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeAll, beforeEach, describe, expect, it, vi } from 'vitest'

import UsageSettings from '../UsageSettings'
import type { OrgUsageEntry } from '../useUsageData'

const usageDataOverride = vi.hoisted(() => ({ current: {} as Record<string, unknown> }))
const distribution = vi.hoisted(() => vi.fn(() => null))

vi.mock('@a3s-lab/ui', async (importOriginal) => importOriginal<typeof CherryStudioUi>())

vi.mock('@renderer/hooks/useProvider', () => ({
  useProviders: () => ({ providers: [] })
}))

vi.mock('../UsageDistributionChart', () => ({
  UsageDistributionChart: (props: unknown) => distribution(props)
}))

vi.mock('../useUsageData', () => {
  const totals = {
    costCurrency: null,
    totalCost: 0,
    totalInputTokens: 0,
    totalOutputTokens: 0,
    totalTokens: 0,
    totalNoCacheTokens: 0,
    totalCacheReadTokens: 0,
    totalCacheWriteTokens: 0,
    recordCount: 0,
    requestCount: 0,
    estimatedRequestCount: 0,
    unpricedRequestCount: 0
  }

  return {
    useUsageData: () => ({
      costTotals: [],
      costCurrency: undefined,
      timelineBuckets: [],
      overviewBuckets: [],
      exploreBuckets: [],
      exploreTimelineRows: [],
      overviewTotals: totals,
      previousOverviewTotals: totals,
      exploreTotals: totals,
      exploreOther: totals,
      timelineLoading: false,
      overviewLoading: false,
      exploreStatsLoading: false,
      exploreTimelineLoading: false,
      hasPrevious: false,
      entries: [],
      needsOrg: false,
      error: null,
      ...usageDataOverride.current
    })
  }
})

beforeAll(() => {
  if (!HTMLElement.prototype.hasPointerCapture) HTMLElement.prototype.hasPointerCapture = () => false
  if (!HTMLElement.prototype.releasePointerCapture) HTMLElement.prototype.releasePointerCapture = () => {}
  if (!HTMLElement.prototype.setPointerCapture) HTMLElement.prototype.setPointerCapture = () => {}
  HTMLElement.prototype.scrollIntoView = () => {}
})

function ledger(
  current: { tokens: number; requests: number; succeeded: number; failed: number },
  hasPrevious: boolean
) {
  const previous = {
    totalTokens: 10,
    requestCount: 2,
    succeeded: 2,
    failed: 1
  }
  return {
    hasPrevious,
    overviewTotals: {
      totalTokens: current.tokens,
      requestCount: current.requests,
      succeeded: current.succeeded,
      failed: current.failed
    },
    previousOverviewTotals: hasPrevious ? previous : { totalTokens: 0, requestCount: 0, succeeded: 0, failed: 0 },
    entries: [] as OrgUsageEntry[]
  }
}

function card(label: string): HTMLElement {
  const cell = screen
    .getAllByText(label)
    .map((node) => node.closest('[data-ui="settings.metric-cell"]'))
    .find((node): node is HTMLElement => node instanceof HTMLElement)
  expect(cell).toBeTruthy()
  return cell as HTMLElement
}

describe('usage page capabilities', () => {
  beforeEach(() => {
    MockUseCacheUtils.resetMocks()
    usageDataOverride.current = {}
    distribution.mockClear()
  })

  it('shows the server message and does not draw zero charts when the read fails', () => {
    usageDataOverride.current = {
      error: 'A3S Cloud usage is not configured',
      timelineLoading: false,
      overviewLoading: false
    }

    render(<UsageSettings />)

    expect(screen.getByText('用量没有载入')).toBeInTheDocument()
    expect(screen.getByText('A3S Cloud usage is not configured')).toBeInTheDocument()
    expect(screen.queryByText('成功请求')).not.toBeInTheDocument()
    expect(screen.queryByText(/0 Token/)).not.toBeInTheDocument()
    expect(distribution).not.toHaveBeenCalled()
  })

  it('does not draw usage charts for the personal center', () => {
    usageDataOverride.current = { needsOrg: true, error: null }

    render(<UsageSettings />)

    expect(screen.getByText('选择组织')).toBeInTheDocument()
    expect(screen.getByText('个人中心没有组织用量。在账户里选择组织后再看这一页。')).toBeInTheDocument()
    expect(screen.queryByText('成功请求')).not.toBeInTheDocument()
    expect(screen.queryByText(/0 Token/)).not.toBeInTheDocument()
    expect(distribution).not.toHaveBeenCalled()
  })

  it('shows succeeded, requests, tokens, and failed counts with the previous period', async () => {
    const user = userEvent.setup()
    usageDataOverride.current = ledger({ tokens: 22, requests: 5, succeeded: 4, failed: 2 }, true)

    render(<UsageSettings />)

    expect(card('成功请求')).toHaveTextContent('4')
    expect(card('请求数')).toHaveTextContent('5')
    expect(card('总 Token 数')).toHaveTextContent('22')
    expect(card('失败请求')).toHaveTextContent('2')
    expect(screen.getAllByText('较上期').length).toBeGreaterThan(0)
    expect(screen.queryByText('总成本')).not.toBeInTheDocument()

    await user.click(screen.getByRole('combobox', { name: '分组' }))
    expect(await screen.findByRole('option', { name: '模型' })).toBeInTheDocument()
    expect(screen.getByRole('option', { name: '接口' })).toBeInTheDocument()
    expect(screen.queryByRole('option', { name: '供应商' })).not.toBeInTheDocument()
    await user.keyboard('{Escape}')

    await user.click(screen.getByRole('combobox', { name: '指标' }))
    expect(await screen.findByRole('option', { name: 'Token' })).toBeInTheDocument()
    expect(screen.getByRole('option', { name: '请求数' })).toBeInTheDocument()
    expect(screen.queryByRole('option', { name: '成本' })).not.toBeInTheDocument()
  })

  it('leaves the previous-period comparison empty when the window has no earlier half', () => {
    usageDataOverride.current = ledger({ tokens: 22, requests: 5, succeeded: 4, failed: 2 }, false)

    render(<UsageSettings />)

    expect(card('总 Token 数')).toHaveTextContent('22')
    expect(screen.queryByText('较上期')).not.toBeInTheDocument()
  })

  it('sorts and pages the entry counts', async () => {
    const user = userEvent.setup()
    const entries: OrgUsageEntry[] = Array.from({ length: 26 }, (_, index) => {
      const day = String(index + 1).padStart(2, '0')
      return {
        id: `2026-09-${day}:model-${index}:chat`,
        modelId: `model-${index}`,
        modelName: `Model ${index}`,
        endpoint: 'chat',
        day: `2026-09-${day}`,
        tokens: 26 - index,
        requests: 1,
        succeeded: 1,
        failed: 0
      }
    })
    usageDataOverride.current = {
      ...ledger({ tokens: 26, requests: 26, succeeded: 26, failed: 0 }, false),
      entries
    }

    const view = render(<UsageSettings />)

    expect(screen.getByText('26 条记录')).toBeInTheDocument()
    expect(screen.queryByRole('columnheader', { name: '成本' })).not.toBeInTheDocument()
    expect(screen.getByText('Model 25').closest('tr')).not.toBeNull()
    expect(screen.queryByText('Model 0')).not.toBeInTheDocument()

    const tokenHeader = screen.getByRole('columnheader', { name: 'Token' })
    await user.click(within(tokenHeader).getByRole('button', { name: 'Token' }))
    view.unmount()
    render(<UsageSettings />)

    expect(screen.getByText('Model 0')).toBeInTheDocument()
    expect(screen.queryByText('Model 25')).not.toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: '加载更多' }))

    expect(screen.getByText('Model 25')).toBeInTheDocument()
  })
})
