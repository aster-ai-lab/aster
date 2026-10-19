// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import type {
  ButtonHTMLAttributes,
  ComponentProps,
  HTMLAttributes,
  ReactNode,
  TableHTMLAttributes,
  TdHTMLAttributes
} from 'react'
import { describe, expect, it, vi } from 'vitest'

import type { OrgUsageEntry } from '../useUsageData'

vi.mock('@a3s-lab/ui', () => ({
  Button: ({
    children,
    size,
    variant,
    ...props
  }: ButtonHTMLAttributes<HTMLButtonElement> & { size?: string; variant?: string }) => (
    <button type="button" data-size={size} data-variant={variant} {...props}>
      {children}
    </button>
  ),
  EmptyState: ({ description, title }: { description?: string; title: string }) => (
    <div>
      <h2>{title}</h2>
      {description && <p>{description}</p>}
    </div>
  ),
  Skeleton: (props: HTMLAttributes<HTMLDivElement>) => <div {...props} />,
  Table: (props: TableHTMLAttributes<HTMLTableElement>) => <table {...props} />,
  TableBody: (props: HTMLAttributes<HTMLTableSectionElement>) => <tbody {...props} />,
  TableCell: (props: TdHTMLAttributes<HTMLTableCellElement>) => <td {...props} />,
  TableHead: (props: HTMLAttributes<HTMLTableCellElement>) => <th {...props} />,
  TableHeader: (props: HTMLAttributes<HTMLTableSectionElement>) => <thead {...props} />,
  TableRow: (props: HTMLAttributes<HTMLTableRowElement>) => <tr {...props} />
}))

vi.mock('../UsageSettingsPrimitives', () => ({
  UsageModelAvatar: () => <span data-testid="model-avatar" />,
  UsagePanel: ({ children }: { children: ReactNode }) => <div data-testid="usage-panel">{children}</div>,
  UsagePanelHeader: ({ children }: { children: ReactNode }) => <div data-testid="usage-panel-header">{children}</div>,
  UsagePanelTitle: ({ children }: { children: ReactNode }) => <h3>{children}</h3>
}))

import { UsageEntriesTable } from '../UsageEntriesTable'

const entry: OrgUsageEntry = {
  id: '2026-10-06:model-a:chat',
  modelId: 'model-a',
  modelName: 'Alpha',
  endpoint: 'chat',
  day: '2026-10-06',
  tokens: 12,
  requests: 3,
  succeeded: 2,
  failed: 1
}

function formatter(value: string): Intl.DateTimeFormat {
  return { format: () => value } as unknown as Intl.DateTimeFormat
}

function renderTable(overrides: Partial<ComponentProps<typeof UsageEntriesTable>> = {}) {
  const onSort = vi.fn()
  const onLoadNext = vi.fn()
  render(
    <UsageEntriesTable
      entries={[entry]}
      entryTotal={1}
      isLoading={false}
      isRefreshing={false}
      hasNextPage={false}
      sortBy="createdAt"
      sortOrder="desc"
      onSort={onSort}
      onLoadNext={onLoadNext}
      dateFormatter={formatter('2026年10月6日')}
      {...overrides}
    />
  )
  return { onSort, onLoadNext }
}

describe('UsageEntriesTable', () => {
  it('shows the model, endpoint, and counts for one ledger line', () => {
    renderTable()

    const row = screen.getByText('Alpha').closest('tr')
    expect(row).not.toBeNull()
    const cells = within(row!)
    expect(cells.getByTestId('model-avatar')).toBeInTheDocument()
    expect(cells.getByText('chat')).toBeInTheDocument()
    expect(cells.getByText('2026年10月6日')).toBeInTheDocument()
    expect(cells.getByText('12')).toHaveClass('text-right')
    expect(cells.getByText('3')).toHaveClass('text-right')
    expect(cells.getByText('2')).toHaveClass('text-right')
    expect(cells.getByText('1')).toHaveClass('text-right')
    expect(screen.getByRole('columnheader', { name: '请求' })).toBeInTheDocument()
    expect(screen.getByRole('columnheader', { name: '成功' })).toBeInTheDocument()
    expect(screen.getByRole('columnheader', { name: '失败' })).toBeInTheDocument()
    expect(screen.queryByRole('columnheader', { name: '成本' })).not.toBeInTheDocument()
  })

  it('names a line with no endpoint as unattributed', () => {
    renderTable({
      entries: [{ ...entry, id: '2026-10-06:model-a:', endpoint: '' }]
    })

    const row = screen.getByText('Alpha').closest('tr')
    expect(within(row!).getByText('未归因来源')).toBeInTheDocument()
  })

  it('keeps the loaded rows mounted while a sort refresh is marked busy', async () => {
    const user = userEvent.setup()
    const { onSort } = renderTable({ isLoading: true, isRefreshing: true })

    expect(screen.getByText('Alpha')).toBeInTheDocument()
    expect(screen.getByRole('table').parentElement).toHaveAttribute('aria-busy', 'true')
    await user.click(screen.getByRole('button', { name: 'Token' }))
    expect(onSort).toHaveBeenCalledWith('totalTokens')
  })

  it('offers the next page only when more lines remain', async () => {
    const user = userEvent.setup()
    const { onLoadNext } = renderTable({ hasNextPage: true, entryTotal: 26 })

    await user.click(screen.getByRole('button', { name: '加载更多' }))
    expect(onLoadNext).toHaveBeenCalledOnce()
  })
})
