import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { SWRConfig } from 'swr'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import { bindCenterMember, releaseCenterMember } from '@renderer/data/memberOrgs'
import type * as MemberOrgApi from '@renderer/data/memberOrgs'
import type { PlatformCatalogQuery, PlatformItem } from '@renderer/data/platformCatalog'
import type * as SupplierApi from '@renderer/data/platformSuppliers'

import PlatformPage from '../PlatformPage'

const account = vi.hoisted(() => ({
  readCenterId: vi.fn(),
  subscribeCenter: vi.fn<(listener: () => void) => () => void>(() => () => undefined),
  listMemberOrgs: vi.fn(),
  writeCenterId: vi.fn(),
  listPlatformItems: vi.fn(),
  readPlatformCatalogPage: vi.fn(),
  listCustomerPeople: vi.fn(),
  listFunctionReleases: vi.fn(),
  listCellRevisions: vi.fn(),
  readSupplierRelations: vi.fn(),
  listModelRoutes: vi.fn(),
  listPlatformUsage: vi.fn(),
  listPlatformLogLevels: vi.fn(),
  findAgentRuntimes: vi.fn(),
  readAgentRuntimeLogs: vi.fn(),
  readPlatformOverview: vi.fn()
}))

const navigate = vi.hoisted(() => vi.fn())

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, values?: Record<string, unknown>) =>
      key === 'collection.page'
        ? `${key} ${values?.page}`
        : key === 'platform.nav.openSection'
          ? `${key} ${values?.name}`
          : key
  })
}))

vi.mock('@tanstack/react-router', () => ({
  useNavigate: () => navigate
}))

vi.mock('@renderer/data/platformSuppliers', async (original) => ({
  ...(await original<typeof SupplierApi>()),
  readSupplierAccess: async () => ({ read: true, create: false, update: false, delete: false }),
  readSupplierRelations: (id: string, query: SupplierApi.SupplierRelationsQuery) =>
    account.readSupplierRelations(id, query)
}))

vi.mock('@renderer/data/memberOrgs', async () => {
  const actual = await vi.importActual<typeof MemberOrgApi>('@renderer/data/memberOrgs')
  return {
    ...actual,
    readCenterId: () => account.readCenterId(),
    subscribeCenter: (listener: () => void) => account.subscribeCenter(listener),
    listMemberOrgs: () => account.listMemberOrgs(),
    writeCenterId: (id: string) => account.writeCenterId(id)
  }
})

vi.mock('@renderer/data/platformCatalog', () => ({
  isPlatformAbsent: (error: unknown) => error instanceof Error && error.message.startsWith('platform.absent.'),
  listPlatformItems: (...args: unknown[]) => account.listPlatformItems(...args),
  readPlatformCatalogPage: (...args: unknown[]) => account.readPlatformCatalogPage(...args),
  listCustomerPeople: (id: string) => account.listCustomerPeople(id),
  listFunctionReleases: (id: string) => account.listFunctionReleases(id),
  listCellRevisions: (id: string) => account.listCellRevisions(id),
  listModelRoutes: () => account.listModelRoutes(),
  findAgentRuntimes: (id: string) => account.findAgentRuntimes(id),
  readAgentRuntimeLogs: (id: string, cursor?: string) => account.readAgentRuntimeLogs(id, cursor),
  listPlatformLogs: (window: string, level: string) => account.listPlatformItems('logs', undefined, window, level),
  listPlatformLogLevels: (window: string) => account.listPlatformLogLevels(window),
  LOG_WINDOWS: ['15m', '1h', '6h', '24h', '7d'],
  LOG_LEVELS: ['all', 'error', 'warn', 'info', 'debug', 'trace'],
  LOG_PAGE_LIMIT: 200,
  listPlatformUsage: (days: string) => account.listPlatformUsage(days),
  USAGE_WINDOWS: ['7', '30', '90'],
  readPlatformOverview: () => account.readPlatformOverview(),
  platformErrorText: (error: unknown, translate: (key: string) => string) => {
    const message = error instanceof Error ? error.message : ''
    return translate(message.startsWith('platform.absent.') ? message : 'platform.error.unknown')
  },
  platformErrorDetail: (error: unknown) =>
    error instanceof Error && !error.message.startsWith('platform.absent.') ? error.message : ''
}))

function renderPage() {
  return render(
    <SWRConfig value={{ provider: () => new Map() }}>
      <PlatformPage />
    </SWRConfig>
  )
}

describe('PlatformPage', () => {
  it('lands a supplier-only delegate on the permitted collection with no unrelated platform destinations', async () => {
    account.listMemberOrgs.mockResolvedValue([
      {
        id: 'org-platform',
        name: 'Platform',
        logo: '',
        isOfficial: true,
        creator: false,
        membership: 'member',
        rules: [{ action: 'read', subject: 'PlatformSupplier' }],
        dataScope: { kind: 'self', departmentIds: [] }
      }
    ])
    renderPage()
    expect(await screen.findByRole('button', { name: 'platform.nav.suppliers' })).toHaveAttribute(
      'aria-current',
      'page'
    )
    expect(screen.queryByRole('button', { name: 'platform.nav.overview' })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'platform.nav.models' })).not.toBeInTheDocument()
    await waitFor(() =>
      expect(account.readPlatformCatalogPage).toHaveBeenCalledWith('suppliers', expect.objectContaining({ page: 1 }))
    )
  })

  beforeEach(() => {
    vi.clearAllMocks()
    localStorage.clear()
    releaseCenterMember()
    account.readCenterId.mockReturnValue('org-platform')
    account.listMemberOrgs.mockResolvedValue([
      {
        id: 'org-platform',
        name: '平台',
        logo: '',
        membership: 'admin',
        creator: true,
        isOfficial: true
      }
    ])
    account.listPlatformItems.mockResolvedValue([])
    account.readPlatformCatalogPage.mockImplementation(async (resource: string, query: PlatformCatalogQuery) => {
      const source = (await account.listPlatformItems(resource)) as PlatformItem[]
      const value = (row: PlatformItem, key: string) => row.fields.find((field) => field.key === key)?.value ?? ''
      const searched = source.filter((row) =>
        [row.name, row.id, ...row.fields.map((field) => field.value)].join(' ').includes(query.q.trim())
      )
      const filterKey = resource === 'suppliers' ? 'kind' : 'state'
      const selected = searched.filter(
        (row) =>
          (!query.filter || value(row, filterKey) === query.filter) &&
          (!query.brand || value(row, 'brand') === query.brand)
      )
      const facet = (rows: PlatformItem[], key: string) =>
        [...new Set(rows.map((row) => value(row, key)).filter(Boolean))].map((entry) => ({
          value: entry,
          count: rows.filter((row) => value(row, key) === entry).length
        }))
      const facets =
        resource === 'suppliers'
          ? { kind: facet(searched, 'kind') }
          : {
              state: facet(
                searched.filter((row) => !query.brand || value(row, 'brand') === query.brand),
                'state'
              ),
              brand: facet(
                searched.filter((row) => !query.filter || value(row, 'state') === query.filter),
                'brand'
              )
            }
      const page = Math.min(query.page, Math.max(1, Math.ceil(selected.length / query.limit)))
      return {
        rows: selected.slice((page - 1) * query.limit, page * query.limit),
        total: selected.length,
        page,
        limit: query.limit,
        facets
      }
    })
    account.listCustomerPeople.mockResolvedValue([])
    account.listFunctionReleases.mockResolvedValue([])
    account.listCellRevisions.mockResolvedValue([])
    account.readSupplierRelations.mockImplementation(async (id: string, query: SupplierApi.SupplierRelationsQuery) => ({
      supplierId: id,
      kind: query.kind,
      items: [],
      total: 0,
      page: 1,
      limit: query.limit,
      revision: 1,
      dependencyTotal: 0
    }))
    account.listModelRoutes.mockResolvedValue([])
    account.listPlatformUsage.mockResolvedValue([])
    account.listPlatformLogLevels.mockResolvedValue([])
    account.findAgentRuntimes.mockResolvedValue([])
    account.readAgentRuntimeLogs.mockResolvedValue({ rows: [], nextCursor: null })
    account.readPlatformOverview.mockResolvedValue({
      customers: 1,
      partners: 1,
      suppliers: 0,
      models: 2,
      services: 3
    })
    account.listPlatformItems.mockImplementation(async (resource: string) => {
      if (resource === 'partners') {
        return [
          {
            id: 'partner-1',
            name: '渠道',
            meta: 'standard · 品牌',
            orgId: 'org-channel',
            fields: [
              { key: 'grade', value: 'standard' },
              { key: 'brand', value: '品牌' }
            ]
          }
        ]
      }
      if (resource === 'customers') {
        return [
          {
            id: 'customer-1',
            name: '甲',
            meta: '',
            fields: [
              { key: 'kind', value: 'org' },
              { key: 'plan', value: 'pro' },
              { key: 'status', value: 'active' },
              { key: 'subscription_status', value: 'active' },
              { key: 'created', value: '2026-09-01' }
            ]
          },
          {
            id: 'customer-2',
            name: '乙',
            meta: '',
            fields: [
              { key: 'kind', value: 'org' },
              { key: 'plan', value: 'team' },
              { key: 'status', value: 'disabled' },
              { key: 'created', value: '2026-09-02' }
            ]
          }
        ]
      }
      return []
    })
  })

  it('lists bound channel partners and opens the team page', async () => {
    renderPage()

    expect(await screen.findByRole('heading', { name: 'platform.title' })).toBeInTheDocument()
    for (const name of [
      'platform.group.overview',
      'platform.group.customers',
      'platform.group.business',
      'platform.group.catalog',
      'platform.group.runtime',
      'platform.group.observe',
      'platform.group.system'
    ]) {
      expect(screen.getByRole('group', { name })).toBeInTheDocument()
    }
    expect(screen.getByRole('heading', { name: 'platform.nav.overview' })).toHaveClass('text-[24px]', 'font-bold')
    fireEvent.click(await screen.findByRole('button', { name: 'platform.nav.channels' }))
    expect(await screen.findByRole('columnheader', { name: 'platform.col.name' })).toBeInTheDocument()
    fireEvent.click(await screen.findByRole('button', { name: 'platform.channels.open' }))

    expect(account.writeCenterId).toHaveBeenCalledWith('org-channel')
    expect(navigate).toHaveBeenCalledWith({ to: '/app/org' })
    expect(screen.getByRole('columnheader', { name: 'platform.col.grade' })).toBeInTheDocument()
    expect(screen.getByRole('columnheader', { name: 'platform.col.tenant' })).toBeInTheDocument()
  })

  it('shows customer columns, a plan picker, and the subscription in the row detail', async () => {
    renderPage()

    fireEvent.click(await screen.findByRole('button', { name: 'platform.nav.customers' }))
    expect(await screen.findByRole('columnheader', { name: 'platform.col.kind' })).toBeInTheDocument()
    expect(screen.getByRole('columnheader', { name: 'platform.col.plan' })).toBeInTheDocument()
    expect(screen.getByRole('columnheader', { name: 'platform.col.created' })).toBeInTheDocument()
    expect(screen.getByText('2026-09-01')).toBeInTheDocument()
    expect(within(screen.getByRole('row', { name: /甲/ })).getByText('active')).toHaveClass('h-5', 'rounded-[4px]')
    expect(screen.getByRole('combobox', { name: 'platform.col.plan' })).toBeInTheDocument()
    expect(screen.queryByRole('columnheader', { name: 'platform.col.subscription' })).not.toBeInTheDocument()

    account.listCustomerPeople.mockResolvedValue([
      {
        id: 'person-1',
        name: '张三',
        meta: '',
        fields: [
          { key: 'contact', value: '13800000000' },
          { key: 'status', value: 'active' }
        ]
      }
    ])
    fireEvent.click(screen.getByRole('button', { name: /甲/ }))
    expect(await screen.findByText('customer-1')).toBeInTheDocument()
    expect(screen.getByText('subscription_status')).toBeInTheDocument()
    expect(await screen.findByText('张三')).toBeInTheDocument()
    expect(screen.getByText('13800000000')).toBeInTheDocument()
    expect(account.listCustomerPeople).toHaveBeenCalledWith('customer-1')
    fireEvent.click(screen.getByRole('button', { name: /甲/ }))

    fireEvent.change(screen.getByRole('combobox', { name: 'platform.col.plan' }), { target: { value: 'team' } })
    expect(screen.queryByRole('button', { name: /甲/ })).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: /乙/ })).toBeInTheDocument()
  })

  it('splits customers by kind before status when both kinds are present', async () => {
    const customer = (id: string, kind: string) => ({
      id,
      name: id,
      meta: '',
      fields: [
        { key: 'kind', value: kind },
        { key: 'status', value: 'active' }
      ]
    })
    account.listPlatformItems.mockImplementation(async (resource: string) =>
      resource === 'customers' ? [customer('张三', 'individual'), customer('甲公司', 'organization')] : []
    )

    renderPage()
    fireEvent.click(await screen.findByRole('button', { name: 'platform.nav.customers' }))
    fireEvent.click(await screen.findByRole('button', { name: 'individual 1' }))
    expect(screen.getByRole('button', { name: /张三/ })).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: /甲公司/ })).not.toBeInTheDocument()
  })

  it('shows a ticket absence sentence and captcha values on a card', async () => {
    account.listPlatformItems.mockImplementation(async (resource: string, facet?: string) => {
      if (resource === 'tickets') throw new Error('platform.absent.tickets')
      if (resource === 'sign-in' && facet === 'captcha') {
        return [{ id: 'tolerance_px', name: '容差', meta: '8', fields: [] }]
      }
      if (resource === 'sign-in') {
        return [{ id: 'auth.wechat', name: '微信', meta: '扫码', fields: [] }]
      }
      return []
    })

    renderPage()

    fireEvent.click(await screen.findByRole('button', { name: 'platform.nav.tickets' }))
    expect(await screen.findByText('platform.absent.tickets')).toBeInTheDocument()

    fireEvent.click(screen.getByRole('button', { name: 'platform.nav.signIn' }))
    expect(await screen.findByText('微信')).toBeInTheDocument()
    expect(screen.getByText('扫码')).toBeInTheDocument()
    expect(screen.getByText('platform.signIn.liveCount')).toBeInTheDocument()
    expect(screen.getByText('0/3')).toBeInTheDocument()
    expect(screen.queryByRole('columnheader', { name: 'platform.col.key' })).not.toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: /微信/ }))
    expect(await screen.findByRole('heading', { name: 'platform.signIn.steps' })).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'platform.signIn.back' }))

    fireEvent.click(screen.getByRole('button', { name: 'platform.signIn.captcha' }))
    expect(await screen.findByRole('heading', { name: 'platform.captcha.group.align' })).toHaveClass(
      'text-[18px]',
      'font-semibold'
    )
    expect(screen.getByText('8')).toBeInTheDocument()
  })

  it('keeps the installation catalog inside the services page', async () => {
    renderPage()

    expect(await screen.findByRole('button', { name: 'platform.nav.services' })).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'platform.services.agents' })).not.toBeInTheDocument()

    fireEvent.click(screen.getByRole('button', { name: 'platform.nav.services' }))
    expect(await screen.findByText('platform.services.empty')).toBeInTheDocument()

    for (const name of [
      'platform.services.agents',
      'platform.services.applets',
      'platform.services.connectors',
      'platform.services.skills',
      'platform.services.plugins'
    ]) {
      expect(screen.getByRole('button', { name })).toBeInTheDocument()
    }
  })

  it('collapses a sidebar sub-menu with its disclosure and reopens it when the menu is chosen', async () => {
    renderPage()
    const services = await screen.findByRole('button', { name: 'platform.nav.services' })
    fireEvent.click(services)
    const disclosure = services.parentElement?.querySelector('[aria-controls="platform-nav-sections-services"]')
    expect(disclosure).toHaveAttribute('aria-expanded', 'true')
    const agents = screen.getByRole('button', { name: 'platform.nav.openSection platform.services.agents' })
    expect(agents.querySelector('svg')).not.toBeNull()

    fireEvent.click(disclosure as HTMLElement)
    expect(disclosure).toHaveAttribute('aria-expanded', 'false')
    expect(screen.queryByRole('button', { name: 'platform.nav.openSection platform.services.agents' })).toBeNull()
    expect(document.getElementById('platform-nav-sections-services')).toHaveAttribute('inert')
    expect(services).toHaveAttribute('aria-current', 'page')

    fireEvent.click(services)
    expect(disclosure).toHaveAttribute('aria-expanded', 'true')
    expect(screen.getByRole('button', { name: 'platform.nav.openSection platform.services.agents' })).toBeVisible()
  })

  it('lists a reference catalog and keeps ledgers that do not exist as one sentence', async () => {
    account.listPlatformItems.mockImplementation(async (resource: string, facet?: string) => {
      if (resource === 'orders' && facet === 'orders') throw new Error('platform.absent.orders')
      if (resource === 'orders') {
        return [
          {
            id: 'sub-1',
            name: '甲',
            meta: '',
            fields: [
              { key: 'plan', value: 'pro' },
              { key: 'subscription_status', value: 'active' }
            ]
          }
        ]
      }
      if (resource === 'tickets') throw new Error('platform.absent.tickets')
      if (resource === 'system' && facet === 'industries') {
        return [
          {
            id: 'C',
            name: '制造业',
            meta: '',
            fields: [
              { key: 'code', value: 'C' },
              { key: 'detail', value: '门类' }
            ]
          }
        ]
      }
      if (resource === 'system' && facet === 'chains') {
        return [
          {
            id: '110000:0',
            name: '新一代信息技术',
            meta: '',
            fields: [
              { key: 'code', value: '110000' },
              { key: 'detail', value: '北京市' }
            ]
          }
        ]
      }
      return []
    })

    renderPage()

    fireEvent.click(await screen.findByRole('button', { name: 'platform.nav.orders' }))
    expect(await screen.findByText('platform.absent.orders')).toBeInTheDocument()
    expect(screen.queryByRole('columnheader')).not.toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'platform.orders.subscriptions' }))
    expect(await screen.findByRole('columnheader', { name: 'platform.col.plan' })).toBeInTheDocument()

    fireEvent.click(screen.getByRole('button', { name: 'platform.nav.system' }))
    fireEvent.click(await screen.findByRole('button', { name: 'platform.system.reference' }))
    expect(await screen.findByRole('columnheader', { name: 'platform.col.code' })).toBeInTheDocument()
    expect(screen.getByText('制造业')).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'platform.reference.chains' }))
    expect(await screen.findByText('新一代信息技术')).toBeInTheDocument()
    expect(screen.queryByText('platform.absent.reference')).not.toBeInTheDocument()
  })

  it('stays closed for a team that is not the official platform team', async () => {
    account.readCenterId.mockReturnValue('org-channel')
    account.listMemberOrgs.mockResolvedValue([
      {
        id: 'org-channel',
        name: '渠道',
        logo: '',
        membership: 'admin',
        creator: true,
        isOfficial: false
      }
    ])

    renderPage()

    expect(await screen.findByRole('heading', { name: 'platform.title' })).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: '渠道' })).not.toBeInTheDocument()
    expect(account.listPlatformItems).not.toHaveBeenCalled()
    expect(account.readPlatformOverview).not.toHaveBeenCalled()
  })

  it('filters a table by status with counts and clears a filter that matches nothing', async () => {
    account.listPlatformItems.mockImplementation(async (resource: string, kind?: string) => {
      if (resource !== 'gateway' || kind) return []
      return [
        {
          id: 'r1',
          name: 'a.example.com',
          meta: '',
          fields: [
            { key: 'state', value: 'active' },
            { key: 'path', value: '/' }
          ]
        },
        {
          id: 'r2',
          name: 'b.example.com',
          meta: '',
          fields: [
            { key: 'state', value: 'active' },
            { key: 'path', value: '/v1' }
          ]
        },
        {
          id: 'r3',
          name: 'c.example.com',
          meta: '',
          fields: [
            { key: 'state', value: 'pending' },
            { key: 'path', value: '/x' }
          ]
        }
      ]
    })

    renderPage()
    fireEvent.click(await screen.findByRole('button', { name: 'platform.nav.gateway' }))
    expect(await screen.findByText('c.example.com')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'common.all 3' })).toBeInTheDocument()

    fireEvent.click(screen.getByRole('button', { name: 'pending 1' }))
    expect(screen.queryByText('a.example.com')).not.toBeInTheDocument()
    expect(screen.getByText('c.example.com')).toBeInTheDocument()

    fireEvent.change(screen.getByPlaceholderText('common.search'), { target: { value: 'zzz' } })
    expect(screen.getByText('platform.nomatchQuery')).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'platform.clearFilters' }))
    expect(screen.getByText('a.example.com')).toBeInTheDocument()
    expect(screen.getByText('c.example.com')).toBeInTheDocument()
  })

  it('looks up agent runtimes by id and pages their trajectory', async () => {
    const execution = '7f8c1c62-6a32-4e7d-9a1b-3a2f0c4d5e6f'
    account.findAgentRuntimes.mockResolvedValue([
      {
        id: execution,
        name: execution,
        meta: '',
        fields: [
          { key: 'status', value: 'running' },
          { key: 'bound', value: 'partial' },
          { key: 'conversation', value: 'conv-1' },
          { key: 'profile_digest', value: 'sha256:harness' }
        ]
      }
    ])
    account.readAgentRuntimeLogs.mockImplementation(async (_id: string, cursor?: string) =>
      cursor
        ? { rows: [{ id: '2', name: 'tool_result', meta: '', fields: [] }], nextCursor: null }
        : {
            rows: [{ id: '1', name: 'model_output', meta: '', fields: [{ key: 'fields', value: '{"text":"hi"}' }] }],
            nextCursor: 'c-2'
          }
    )

    renderPage()
    fireEvent.click(await screen.findByRole('button', { name: 'platform.nav.workloads' }))
    fireEvent.click(await screen.findByRole('button', { name: 'platform.workloads.runtimes' }))
    expect(await screen.findByText('platform.runtimes.prompt')).toBeInTheDocument()
    expect(screen.queryByPlaceholderText('common.search')).not.toBeInTheDocument()
    expect(account.findAgentRuntimes).not.toHaveBeenCalled()

    const field = screen.getByPlaceholderText('platform.runtimes.placeholder')
    fireEvent.change(field, { target: { value: 'not-an-id' } })
    fireEvent.click(screen.getByRole('button', { name: 'platform.runtimes.lookup' }))
    expect(screen.getByRole('alert')).toHaveTextContent('platform.runtimes.invalid')
    expect(account.findAgentRuntimes).not.toHaveBeenCalled()

    fireEvent.change(field, { target: { value: ` ${execution.toUpperCase()} ` } })
    fireEvent.click(screen.getByRole('button', { name: 'platform.runtimes.lookup' }))
    expect(await screen.findByRole('columnheader', { name: 'platform.col.execution' })).toBeInTheDocument()
    expect(account.findAgentRuntimes).toHaveBeenCalledWith(execution)
    expect(screen.getByText('partial')).toBeInTheDocument()

    fireEvent.click(screen.getByRole('button', { expanded: false, name: new RegExp(execution) }))
    expect(await screen.findByText('model_output')).toBeInTheDocument()
    expect(screen.getByText('{"text":"hi"}')).toBeInTheDocument()
    expect(screen.getByText('sha256:harness')).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'platform.runtimes.more' }))
    expect(await screen.findByText('tool_result')).toBeInTheDocument()
    expect(account.readAgentRuntimeLogs).toHaveBeenLastCalledWith(execution, 'c-2')
    expect(screen.queryByRole('button', { name: 'platform.runtimes.more' })).not.toBeInTheDocument()
  })

  it('shows function releases and cell revisions in the row detail', async () => {
    account.listPlatformItems.mockImplementation(async (resource: string, facet?: string) => {
      if (resource === 'workloads' && facet === 'functions') {
        return [
          {
            id: 'fn-1',
            name: 'resize',
            meta: '',
            fields: [
              { key: 'version', value: '1.2.0' },
              { key: 'mode', value: 'hosted_task' },
              { key: 'artifact', value: 'sha256:artifact' }
            ]
          }
        ]
      }
      if (resource === 'workloads' && facet === 'cells') {
        return [
          {
            id: 'p/e/a',
            name: 'ledger',
            meta: '',
            fields: [
              { key: 'state', value: 'running' },
              { key: 'project', value: 'core' },
              { key: 'environment', value: 'prod' }
            ]
          }
        ]
      }
      return []
    })
    account.listFunctionReleases.mockResolvedValue([
      {
        id: 'rel-1',
        name: '1.1.0',
        meta: '',
        fields: [
          { key: 'state', value: 'yanked' },
          { key: 'mode', value: 'external' },
          { key: 'artifact', value: 'sha256:old' }
        ]
      }
    ])
    account.listCellRevisions.mockResolvedValue([
      { id: 'rev-3', name: '#3', meta: '', fields: [{ key: 'digest', value: 'sha256:def' }] }
    ])

    renderPage()
    fireEvent.click(await screen.findByRole('button', { name: 'platform.nav.workloads' }))
    fireEvent.click(await screen.findByRole('button', { name: 'platform.workloads.functions' }))
    expect(await screen.findByRole('columnheader', { name: 'platform.col.release' })).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { expanded: false, name: /resize/ }))
    expect(await screen.findByText('sha256:old')).toBeInTheDocument()
    expect(account.listFunctionReleases).toHaveBeenCalledWith('fn-1')
    expect(screen.getByText('yanked')).toBeInTheDocument()

    fireEvent.click(screen.getByRole('button', { name: 'platform.workloads.cells' }))
    expect(await screen.findByRole('columnheader', { name: 'platform.col.environment' })).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { expanded: false, name: /ledger/ }))
    expect(await screen.findByText('sha256:def')).toBeInTheDocument()
    expect(account.listCellRevisions).toHaveBeenCalledWith('p/e/a')
  })

  it('shows each model its own supplier routes and each supplier its declared models', async () => {
    account.listPlatformItems.mockImplementation(async (resource: string) => {
      if (resource === 'models') {
        return [
          {
            id: 'deepseek/chat',
            name: 'DeepSeek Chat',
            meta: '',
            fields: [
              { key: 'failover', value: 'retryable_status' },
              { key: 'multiplier', value: '1.5' }
            ]
          },
          { id: 'glm', name: 'GLM', meta: '', fields: [] }
        ]
      }
      if (resource === 'suppliers') {
        return [{ id: 'sup-1', name: 'Relay', meta: '', fields: [{ key: 'kind', value: 'relay' }] }]
      }
      return []
    })
    const route = (id: string, offering: string, upstream: string, status: string) => ({
      id,
      name: 'Relay',
      meta: '',
      fields: [
        { key: 'offering', value: offering },
        { key: 'upstream', value: upstream },
        { key: 'priority', value: '1' },
        { key: 'weight', value: '100' },
        { key: 'status', value: status }
      ]
    })
    account.listModelRoutes.mockResolvedValue([
      route('r-1', 'deepseek/chat', 'ds-upstream', 'missing'),
      route('r-2', 'glm', 'glm-upstream', 'ready')
    ])
    account.readSupplierRelations.mockImplementation(async (id: string, query: SupplierApi.SupplierRelationsQuery) => ({
      supplierId: id,
      kind: query.kind,
      items:
        query.kind === 'models'
          ? [{ id: 'ds-upstream', name: 'ds-upstream', state: null, upstreamModels: [], dependencyCount: 1 }]
          : [
              {
                id: 'deepseek/chat',
                name: 'DeepSeek Chat',
                state: 'draft',
                upstreamModels: ['ds-upstream'],
                dependencyCount: 0
              }
            ],
      total: 1,
      page: 1,
      limit: query.limit,
      revision: 1,
      dependencyTotal: 1
    }))

    renderPage()
    fireEvent.click(await screen.findByRole('button', { name: 'platform.nav.models' }))
    fireEvent.click(await screen.findByRole('button', { expanded: false, name: /DeepSeek Chat/ }))
    expect(await screen.findByText('ds-upstream')).toBeInTheDocument()
    expect(screen.queryByText('glm-upstream')).not.toBeInTheDocument()
    expect(screen.getByText('missing')).toBeInTheDocument()
    expect(screen.getByText('×1.5')).toBeInTheDocument()
    expect(screen.getByText('retryable_status')).toBeInTheDocument()

    fireEvent.click(screen.getByRole('button', { name: 'platform.nav.suppliers' }))
    fireEvent.click(await screen.findByRole('button', { expanded: false, name: /Relay/ }))
    expect(await screen.findByText('ds-upstream')).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'platform.supplierRelations.viewDependencies' }))
    expect(await screen.findByText('DeepSeek Chat')).toBeInTheDocument()
    expect(screen.getByText('platform.supplierRelations.state.draft')).toBeInTheDocument()
  })

  it('formats function timeouts and app sizes and filters functions by mode', async () => {
    account.listPlatformItems.mockImplementation(async (resource: string, facet?: string) => {
      if (resource === 'workloads' && facet === 'functions') {
        return [
          {
            id: 'fn-1',
            name: 'resize',
            meta: '',
            fields: [
              { key: 'mode', value: 'hosted_task' },
              { key: 'state', value: 'active' },
              { key: 'concurrency', value: '4' },
              { key: 'timeout', value: '30000' }
            ]
          },
          {
            id: 'fn-2',
            name: 'webhook',
            meta: '',
            fields: [
              { key: 'mode', value: 'external' },
              { key: 'state', value: 'active' },
              { key: 'timeout', value: '250' }
            ]
          }
        ]
      }
      if (resource === 'system' && facet === 'apps') {
        return [
          {
            id: 'app-1',
            name: '1.2.0',
            meta: '',
            fields: [
              { key: 'platform', value: 'macos' },
              { key: 'size', value: String(3 * 1024 * 1024) },
              { key: 'state', value: 'published' }
            ]
          }
        ]
      }
      return []
    })

    renderPage()
    fireEvent.click(await screen.findByRole('button', { name: 'platform.nav.workloads' }))
    fireEvent.click(await screen.findByRole('button', { name: 'platform.workloads.functions' }))
    expect(await screen.findByText('30 s')).toBeInTheDocument()
    expect(screen.getByText('250 ms')).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'external 1' }))
    expect(screen.queryByText('resize')).not.toBeInTheDocument()
    expect(screen.getByText('webhook')).toBeInTheDocument()

    fireEvent.click(screen.getByRole('button', { name: 'platform.nav.system' }))
    expect(await screen.findByText('3.0 MB')).toBeInTheDocument()
  })

  it('shows shelf categories, installs and deployment on services and resources on workflows', async () => {
    account.listPlatformItems.mockImplementation(async (resource: string, facet?: string) => {
      if (resource === 'services' && facet === 'skills') {
        return [
          {
            id: 'doc-writer',
            name: 'writer',
            meta: '',
            fields: [
              { key: 'category', value: 'office' },
              { key: 'version', value: '1.2.0' },
              { key: 'author', value: 'a3s' },
              { key: 'installs', value: '42' },
              { key: 'featured', value: 'featured' },
              { key: 'status', value: 'enabled' },
              { key: 'updated', value: '2026-10-11' },
              { key: 'tags', value: 'docs, office' },
              { key: 'notes', value: 'Writes documents' }
            ]
          },
          {
            id: 'chart',
            name: 'charts',
            meta: '',
            fields: [
              { key: 'category', value: 'data' },
              { key: 'installs', value: '3' },
              { key: 'status', value: 'disabled' },
              { key: 'updated', value: '2026-09-01' }
            ]
          }
        ]
      }
      if (resource === 'services' && facet === 'connectors') {
        return [
          {
            id: 'github',
            name: 'github',
            meta: '',
            fields: [
              { key: 'category', value: 'code' },
              { key: 'provider', value: 'GitHub' },
              { key: 'deployment', value: 'hosted' },
              { key: 'transport', value: 'streamable_http' },
              { key: 'tools', value: '12' },
              { key: 'status', value: 'enabled' }
            ]
          },
          {
            id: 'fs',
            name: 'files',
            meta: '',
            fields: [
              { key: 'category', value: 'productivity' },
              { key: 'deployment', value: 'local' },
              { key: 'transport', value: 'stdio' },
              { key: 'status', value: 'enabled' }
            ]
          }
        ]
      }
      if (resource === 'workloads' && facet === 'workflows') {
        return [
          {
            id: 'wl-1',
            name: 'edge-api',
            meta: '',
            fields: [
              { key: 'health', value: 'degraded' },
              { key: 'replicas', value: '1/2' },
              { key: 'desired', value: 'running' },
              { key: 'cpu', value: '500m' },
              { key: 'memory', value: String(256 * 1024 * 1024) },
              { key: 'environment', value: 'prod' },
              { key: 'notes', value: '#0 ready · running\n#1 pending' }
            ]
          },
          {
            id: 'wl-2',
            name: 'worker',
            meta: '',
            fields: [
              { key: 'health', value: 'healthy' },
              { key: 'environment', value: 'prod' }
            ]
          }
        ]
      }
      return []
    })

    renderPage()
    fireEvent.click(await screen.findByRole('button', { name: 'platform.nav.services' }))
    fireEvent.click(await screen.findByRole('button', { name: 'platform.services.skills' }))
    expect(await screen.findByRole('columnheader', { name: 'platform.col.installs' })).toBeInTheDocument()
    expect(screen.getByText('2026-10-11')).toBeInTheDocument()
    expect(screen.getByText('42')).toBeInTheDocument()
    fireEvent.change(screen.getByRole('combobox', { name: 'platform.col.category' }), { target: { value: 'data' } })
    expect(screen.queryByRole('button', { name: /writer/ })).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: /charts/ })).toBeInTheDocument()
    fireEvent.change(screen.getByRole('combobox', { name: 'platform.col.category' }), { target: { value: '__all__' } })
    fireEvent.click(screen.getByRole('button', { name: /writer/ }))
    expect(await screen.findByText('docs, office')).toBeInTheDocument()
    expect(screen.getAllByText('Writes documents')).toHaveLength(2)

    fireEvent.click(screen.getByRole('button', { name: 'platform.services.connectors' }))
    expect(await screen.findByRole('columnheader', { name: 'platform.col.deployment' })).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'local 1' }))
    expect(screen.queryByRole('button', { name: /github/ })).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: /files/ })).toBeInTheDocument()

    fireEvent.click(screen.getByRole('button', { name: 'platform.nav.workloads' }))
    expect(await screen.findByText('256.0 MB')).toBeInTheDocument()
    expect(screen.getByText('500m')).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'degraded 1' }))
    expect(screen.queryByRole('button', { name: /worker/ })).not.toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: /edge-api/ }))
    expect(await screen.findAllByText(/#1 pending/)).toHaveLength(2)
    expect(screen.getByText('running')).toBeInTheDocument()
  })

  it('shows bootstrap sources, hides secret values, and filters settings that need attention', async () => {
    account.listPlatformItems.mockImplementation(async (resource: string, facet?: string) => {
      if (resource === 'system' && facet === 'bootstrap') {
        return [
          {
            id: 'A3S_DATABASE_URL',
            name: '连接串',
            meta: '',
            fields: [
              { key: 'group', value: '数据库' },
              { key: 'source', value: 'env' },
              { key: 'required', value: 'yes' },
              { key: 'secret', value: 'yes' }
            ]
          },
          {
            id: 'A3S_SMTP_HOST',
            name: '邮件主机',
            meta: '',
            fields: [
              { key: 'group', value: '邮件' },
              { key: 'source', value: 'default' },
              { key: 'required', value: 'yes' },
              { key: 'value', value: '' }
            ]
          },
          {
            id: 'A3S_POOL_SIZE',
            name: '连接池',
            meta: '',
            fields: [
              { key: 'group', value: '数据库' },
              { key: 'source', value: 'default' },
              { key: 'value', value: '16' }
            ]
          }
        ]
      }
      return []
    })

    renderPage()
    fireEvent.click(await screen.findByRole('button', { name: 'platform.nav.system' }))
    fireEvent.click(await screen.findByRole('button', { name: 'platform.system.bootstrap' }))
    expect(await screen.findByText('A3S_DATABASE_URL')).toBeInTheDocument()
    expect(screen.getByText('platform.settings.hidden')).toBeInTheDocument()
    expect(screen.getByText('16')).toBeInTheDocument()
    expect(screen.getByText('platform.settings.missing')).toBeInTheDocument()

    fireEvent.click(screen.getByRole('button', { name: 'platform.settings.source.attention 1' }))
    expect(screen.getByText('A3S_SMTP_HOST')).toBeInTheDocument()
    expect(screen.queryByText('A3S_DATABASE_URL')).not.toBeInTheDocument()
    expect(screen.queryByText('A3S_POOL_SIZE')).not.toBeInTheDocument()
  })

  it('reads logs by time window, polls when live, and shows structured fields', async () => {
    const at = '2026-10-11T06:02:11.120Z'
    account.listPlatformItems.mockImplementation(async (resource: string) => {
      if (resource !== 'logs') return []
      return [
        {
          id: 'log-1',
          name: 'upstream timeout',
          meta: '',
          fields: [
            { key: 'time', value: at },
            { key: 'level', value: 'error' },
            { key: 'service', value: 'gateway' },
            { key: 'target', value: 'a3s_gateway::proxy' },
            { key: 'fields', value: '{\n  "route": "/v1/chat"\n}' }
          ]
        }
      ]
    })

    renderPage()
    fireEvent.click(await screen.findByRole('button', { name: 'platform.nav.logs' }))
    const local = new Date(at)
    const pad = (value: number) => String(value).padStart(2, '0')
    const shown = `${local.getFullYear()}-${pad(local.getMonth() + 1)}-${pad(local.getDate())} ${pad(local.getHours())}:${pad(local.getMinutes())}:${pad(local.getSeconds())}`
    expect(await screen.findByText(shown)).toBeInTheDocument()
    expect(account.listPlatformItems).toHaveBeenCalledWith('logs', undefined, '1h', 'all')

    fireEvent.click(screen.getByRole('button', { name: 'platform.logs.window.6h' }))
    await screen.findByText(shown)
    expect(account.listPlatformItems).toHaveBeenCalledWith('logs', undefined, '6h', 'all')

    const live = screen.getByRole('button', { name: 'platform.logs.live' })
    expect(live).toHaveAttribute('aria-pressed', 'false')
    fireEvent.click(live)
    expect(live).toHaveAttribute('aria-pressed', 'true')

    fireEvent.click(screen.getByRole('button', { name: /upstream timeout/ }))
    expect(screen.getByText(/"route": "\/v1\/chat"/)).toBeInTheDocument()
    expect(screen.getByText('a3s_gateway::proxy')).toBeInTheDocument()
  })

  it('reads usage once per window and splits it by customer and by model', async () => {
    const fields = (entries: Record<string, string>) => Object.entries(entries).map(([key, value]) => ({ key, value }))
    account.listPlatformUsage.mockResolvedValue([
      {
        id: 'day:2026-10-10',
        name: '2026-10-10',
        meta: '',
        fields: fields({ section: 'days', tokens: '0', requests: '0', succeeded: '0', failed: '0' })
      },
      {
        id: 'day:2026-10-11',
        name: '2026-10-11',
        meta: '',
        fields: fields({ section: 'days', tokens: '1500', requests: '4', succeeded: '3', failed: '1' })
      },
      {
        id: 'org-a',
        name: '甲公司',
        meta: '',
        fields: fields({
          section: 'customers',
          tokens: '1500',
          requests: '4',
          success: '75.0',
          failed: '1',
          share: '100.0',
          models: '1',
          top_model: 'deepseek-chat',
          cloud: 'read'
        })
      },
      {
        id: 'org-b',
        name: '乙公司',
        meta: '',
        fields: fields({
          section: 'customers',
          tokens: '0',
          requests: '0',
          success: '',
          failed: '0',
          share: '0.0',
          models: '0',
          top_model: '',
          cloud: 'unavailable'
        })
      },
      {
        id: 'model:qwen-max',
        name: 'qwen-max',
        meta: '',
        fields: fields({
          section: 'models',
          tokens: '1500',
          requests: '4',
          success: '75.0',
          failed: '1',
          share: '100.0',
          customers: '1',
          endpoints: 'chat-completions'
        })
      }
    ])

    renderPage()
    const observe = await screen.findByRole('group', { name: 'platform.group.observe' })
    fireEvent.click(within(observe).getByRole('button', { name: 'platform.nav.usage' }))

    expect(await screen.findByText('甲公司')).toBeInTheDocument()
    expect(account.listPlatformUsage.mock.calls.filter(([days]) => days === '30')).toHaveLength(1)
    const summary = screen.getByText('platform.usage.stat.tokens').closest('dl') as HTMLElement
    expect(within(summary).getByText('1.5K')).toBeInTheDocument()
    expect(within(summary).getByText('75.0%')).toBeInTheDocument()
    expect(within(summary).getByText('1')).toBeInTheDocument()
    expect(screen.getByText('platform.usage.partial')).toBeInTheDocument()
    expect(screen.getByRole('img', { name: 'platform.usage.chart' })).toBeInTheDocument()
    expect(screen.getByRole('columnheader', { name: 'platform.usage.col.topModel' })).toBeInTheDocument()
    expect(screen.getByRole('columnheader', { name: 'platform.usage.col.cloud' })).toBeInTheDocument()
    expect(screen.getByText('乙公司')).toBeInTheDocument()
    expect(screen.queryByText('qwen-max')).not.toBeInTheDocument()
    expect(screen.queryByText('2026-10-11')).not.toBeInTheDocument()

    fireEvent.click(screen.getByRole('button', { name: 'platform.usage.models' }))
    expect(await screen.findByText('qwen-max')).toBeInTheDocument()
    expect(screen.getByRole('columnheader', { name: 'platform.usage.col.customers' })).toBeInTheDocument()
    expect(screen.queryByText('甲公司')).not.toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: /qwen-max/ }))
    expect(screen.getByText('chat-completions')).toBeInTheDocument()

    const rows = await account.listPlatformUsage.mock.results.at(-1)?.value
    let release: (value: unknown) => void = () => undefined
    account.listPlatformUsage.mockImplementation(() => new Promise((resolve) => (release = resolve)))
    fireEvent.click(screen.getByRole('button', { name: 'platform.usage.window.7' }))
    expect(account.listPlatformUsage).toHaveBeenCalledWith('7')
    fireEvent.click(screen.getByRole('button', { name: 'platform.usage.window.90' }))
    expect(account.listPlatformUsage).toHaveBeenCalledWith('90')
    await waitFor(() =>
      expect(document.querySelector('[data-ui="platform.usage.summary"]')).toHaveAttribute('aria-busy', 'true')
    )
    expect(document.querySelector('[data-ui="platform.list"]')).toHaveAttribute('aria-busy', 'true')
    release(rows)
    await waitFor(() =>
      expect(document.querySelector('[data-ui="platform.usage.summary"]')).not.toHaveAttribute('aria-busy')
    )
    expect(screen.getByText('qwen-max')).toBeInTheDocument()
  })

  it('expands a row into every field it carries', async () => {
    account.listPlatformItems.mockImplementation(async (resource: string, kind?: string) => {
      if (resource !== 'gateway' || kind) return []
      return [
        {
          id: 'route-1',
          name: 'a.example.com',
          meta: '',
          fields: [
            { key: 'path', value: '/api' },
            { key: 'target', value: 'svc-a' },
            { key: 'state', value: 'active' }
          ]
        }
      ]
    })

    renderPage()
    fireEvent.click(await screen.findByRole('button', { name: 'platform.nav.gateway' }))
    const toggle = await screen.findByRole('button', { name: /a\.example\.com/ })
    expect(toggle).toHaveAttribute('aria-expanded', 'false')
    fireEvent.click(toggle)
    expect(toggle).toHaveAttribute('aria-expanded', 'true')
    expect(screen.getByText('route-1')).toBeInTheDocument()
    expect(screen.getAllByText('svc-a')).toHaveLength(2)
  })

  it('summarises the live gateway and switches between routes, platform routes and standards', async () => {
    account.listPlatformItems.mockImplementation(async (resource: string, kind?: string) => {
      if (resource !== 'gateway') return []
      if (kind === 'live') {
        return [
          {
            id: 'gateway',
            name: 'http://127.0.0.1:9900',
            meta: '',
            fields: [
              { key: 'state', value: 'running' },
              { key: 'uptime', value: '93784' },
              { key: 'connections', value: '3' },
              { key: 'mounted', value: '4/9' },
              { key: 'requests', value: '1200' },
              { key: 'errors', value: '7' }
            ]
          }
        ]
      }
      if (kind === 'platform') {
        return [
          {
            id: 'core-api',
            name: 'core-api',
            meta: '',
            fields: [
              { key: 'path', value: 'prefix /api' },
              { key: 'target', value: 'http://127.0.0.1:3100' },
              { key: 'latency', value: '5' },
              { key: 'backends', value: '1/1' },
              { key: 'middlewares', value: 'cors, auth' }
            ]
          }
        ]
      }
      if (kind === 'standards') {
        return [
          {
            id: 'model:openai.chat',
            name: 'openai.chat',
            meta: '',
            fields: [
              { key: 'plane', value: 'model' },
              { key: 'coverage', value: 'mounted' },
              { key: 'paths', value: '1' },
              { key: 'notes', value: 'POST /v1/chat/completions' }
            ]
          },
          {
            id: 'mcp:mcp.streamable',
            name: 'mcp.streamable',
            meta: '',
            fields: [
              { key: 'plane', value: 'mcp' },
              { key: 'coverage', value: 'absent' },
              { key: 'paths', value: '0' }
            ]
          }
        ]
      }
      return [
        {
          id: 'route-1',
          name: 'a.example.com',
          meta: '',
          fields: [
            { key: 'state', value: 'active' },
            { key: 'path', value: '/v1' },
            { key: 'replicas', value: '1/2' },
            { key: 'gateway_node', value: 'edge-1' },
            { key: 'members', value: '2/3' },
            { key: 'workload', value: 'orders-api' },
            { key: 'port', value: 'http' },
            { key: 'upstream', value: 'http://10.0.0.5:8080' }
          ]
        }
      ]
    })

    renderPage()
    fireEvent.click(await screen.findByRole('button', { name: 'platform.nav.gateway' }))
    const stats = await screen.findByText('platform.gateway.stat.uptime')
    const strip = stats.closest('dl') as HTMLElement
    expect(within(strip).getByText('1d 2h')).toBeInTheDocument()
    expect(within(strip).getByText('4/9')).toBeInTheDocument()
    expect(within(strip).getByText((1200).toLocaleString())).toBeInTheDocument()
    expect(within(strip).getByText('running')).toBeInTheDocument()

    expect(await screen.findByRole('columnheader', { name: 'platform.col.replicas' })).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { expanded: false, name: /a\.example\.com/ }))
    const chain = (await screen.findByText('platform.gateway.chain.title')).closest('section') as HTMLElement
    for (const hop of ['a.example.com', '/v1', 'edge-1', 'orders-api', 'http://10.0.0.5:8080', 'http']) {
      expect(within(chain).getByText(hop)).toBeInTheDocument()
    }
    expect(within(chain).getAllByRole('listitem')).toHaveLength(4)

    fireEvent.click(screen.getByRole('button', { name: 'platform.gateway.platform' }))
    expect(await screen.findByText('5 ms')).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: /core-api/ }))
    expect(screen.getByText('cors, auth')).toBeInTheDocument()

    fireEvent.click(screen.getByRole('button', { name: 'platform.gateway.standards' }))
    expect(await screen.findByText('openai.chat')).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'absent 1' }))
    expect(screen.queryByText('openai.chat')).not.toBeInTheDocument()
    expect(screen.getByText('mcp.streamable')).toBeInTheDocument()
    expect(account.listPlatformItems).toHaveBeenCalledWith('gateway', 'standards')
  })

  it('narrows cluster nodes from a node pool card and shows pool health on the topology', async () => {
    const observed = new Date(Date.now() - 5 * 60_000).toISOString()
    const node = (id: string, pool: string, state = 'ready') => ({
      id,
      name: id,
      meta: '',
      fields: [
        { key: 'state', value: state },
        { key: 'availability', value: 'online' },
        { key: 'pool', value: pool },
        { key: 'observed', value: observed }
      ]
    })
    account.listPlatformItems.mockImplementation(async (resource: string, kind?: string) => {
      if (resource !== 'fleet') return []
      if (kind === 'topology') {
        return [
          {
            id: 'p1',
            name: 'gpu',
            meta: '',
            fields: [
              { key: 'nodes', value: '2' },
              { key: 'online', value: '1/2' },
              { key: 'maintenance', value: 'scheduled' },
              { key: 'notes', value: 'kernel upgrade' }
            ]
          },
          { id: 'p2', name: 'cpu', meta: '', fields: [{ key: 'online', value: '1/1' }] }
        ]
      }
      return [node('node-a', 'gpu'), node('node-b', 'gpu', 'unreachable'), node('node-c', 'cpu')]
    })

    renderPage()
    fireEvent.click(await screen.findByRole('button', { name: 'platform.nav.fleet' }))
    expect(await screen.findByText('node-c')).toBeInTheDocument()
    const rowOf = (name: string) => screen.getByText(name).closest('tr') as HTMLElement
    expect(within(rowOf('node-b')).getByText('unreachable').closest('[data-attention]')).not.toBeNull()
    expect(within(rowOf('node-a')).getByText('ready').closest('[data-attention]')).toBeNull()
    expect(within(rowOf('node-a')).getByText('platform.ago.minutes').closest('[title]')).toHaveAttribute(
      'title',
      expect.stringMatching(/^\d{4}-\d{2}-\d{2} \d{2}:\d{2}$/)
    )
    const order = () =>
      screen
        .getAllByRole('row')
        .slice(1)
        .map((row) => row.textContent ?? '')
    expect(order()[0]).toContain('node-b')
    expect(order()[1]).toContain('node-a')
    expect(screen.getByText('platform.problemsFirst')).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'platform.col.name' }))
    expect(order()[0]).toContain('node-a')
    expect(screen.queryByText('platform.problemsFirst')).not.toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'platform.col.name' }))
    fireEvent.click(screen.getByRole('button', { name: 'platform.col.name' }))
    expect(order()[0]).toContain('node-b')
    const card = await screen.findByRole('button', { name: /gpu.*1\/2/ })
    expect(card).toHaveAttribute('aria-pressed', 'false')
    fireEvent.click(card)
    expect(card).toHaveAttribute('aria-pressed', 'true')
    expect(screen.queryByText('node-c')).not.toBeInTheDocument()
    expect(screen.getByText('node-a')).toBeInTheDocument()
    fireEvent.click(card)
    expect(screen.getByText('node-c')).toBeInTheDocument()

    fireEvent.click(screen.getByRole('button', { name: 'platform.fleet.topology' }))
    expect(await screen.findByRole('columnheader', { name: 'platform.col.online' })).toBeInTheDocument()
    const pool = screen.getByRole('button', { name: /gpu/, expanded: false })
    fireEvent.click(pool)
    expect(pool).toHaveAttribute('aria-expanded', 'true')
    expect(screen.getAllByText('kernel upgrade')).toHaveLength(2)
  })

  it('shows supplier readiness without credentials and filters subscriptions by state', async () => {
    account.listPlatformItems.mockImplementation(async (resource: string, kind?: string) => {
      if (resource === 'suppliers') {
        return [
          {
            id: 's1',
            name: 'DeepSeek',
            meta: '',
            fields: [
              { key: 'kind', value: 'vendor' },
              { key: 'credential', value: 'missing' },
              { key: 'status', value: 'incomplete' },
              { key: 'created', value: '2026-09-10' }
            ]
          }
        ]
      }
      if (resource === 'models') {
        return [
          {
            id: 'deepseek-chat',
            name: 'DeepSeek Chat',
            meta: '',
            fields: [
              { key: 'brand', value: 'deepseek' },
              { key: 'context', value: '128000' },
              { key: 'price', value: '$0.27 / $1.1' },
              { key: 'served', value: 'vendor' },
              { key: 'capabilities', value: 'tool_call, vision' },
              { key: 'state', value: 'published' }
            ]
          }
        ]
      }
      if (resource === 'orders' && kind === 'subscriptions') {
        return [
          { id: 'o1', name: '甲', meta: '', fields: [{ key: 'subscription_status', value: 'active' }] },
          { id: 'o2', name: '乙', meta: '', fields: [{ key: 'subscription_status', value: 'past_due' }] }
        ]
      }
      if (resource === 'orders') throw new Error('platform.absent.orders')
      return []
    })

    renderPage()
    fireEvent.click(await screen.findByRole('button', { name: 'platform.nav.suppliers' }))
    expect(await screen.findByRole('columnheader', { name: 'platform.col.credential' })).toBeInTheDocument()
    expect(screen.getByText('missing').closest('[data-attention]')).not.toBeNull()
    expect(screen.getByText('2026-09-10')).toBeInTheDocument()

    fireEvent.click(screen.getByRole('button', { name: 'platform.nav.models' }))
    expect(await screen.findByText('128K')).toBeInTheDocument()
    expect(screen.getByText('$0.27 / $1.1')).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: /DeepSeek Chat/ }))
    expect(screen.getByText('tool_call, vision')).toBeInTheDocument()

    fireEvent.click(screen.getByRole('button', { name: 'platform.nav.orders' }))
    fireEvent.click(await screen.findByRole('button', { name: 'platform.orders.subscriptions' }))
    fireEvent.click(await screen.findByRole('button', { name: 'past_due 1' }))
    expect(screen.queryByText('甲')).not.toBeInTheDocument()
    expect(screen.getAllByText('乙').length).toBeGreaterThan(0)

    fireEvent.click(screen.getByRole('button', { name: 'platform.nav.signIn' }))
    expect(await screen.findByText('platform.signIn.empty')).toBeInTheDocument()
    expect(account.listPlatformItems).toHaveBeenCalledWith('sign-in', 'oauth')
    expect(account.listPlatformItems).not.toHaveBeenCalledWith('sign-in', 'subscriptions')
  })

  it('offers a retry for a failed read but not for a ledger the installation does not keep', async () => {
    let fail = true
    account.listPlatformItems.mockImplementation(async (resource: string) => {
      if (resource === 'tickets') throw new Error('platform.absent.tickets')
      if (resource === 'logs') {
        if (fail) throw new Error('boom')
        return [{ id: 'l1', name: 'started', meta: '', fields: [{ key: 'level', value: 'info' }] }]
      }
      return []
    })

    renderPage()
    fireEvent.click(await screen.findByRole('button', { name: 'platform.nav.tickets' }))
    expect(await screen.findByText('platform.absent.tickets')).toBeInTheDocument()
    expect(screen.getByText('platform.absent.title')).toBeInTheDocument()
    expect(screen.queryByText('platform.error.detail')).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'common.retry' })).not.toBeInTheDocument()

    fireEvent.click(screen.getByRole('button', { name: 'platform.nav.logs' }))
    expect(await screen.findByText('platform.error.title')).toBeInTheDocument()
    expect(screen.getByText('platform.error.unknown')).toBeInTheDocument()
    expect(screen.getByText('platform.error.detail').closest('details')).not.toHaveAttribute('open')
    expect(screen.getByText('boom')).toBeInTheDocument()
    fail = false
    fireEvent.click(screen.getByRole('button', { name: 'common.retry' }))
    expect(await screen.findByText('started')).toBeInTheDocument()
  })

  it('drops the level picker when the window has no log lines and suggests a longer window', async () => {
    account.listPlatformLogLevels.mockResolvedValue([
      { id: 'all', name: 'all', meta: '', fields: [{ key: 'count', value: '0' }] }
    ])
    renderPage()
    fireEvent.click(await screen.findByRole('button', { name: 'platform.nav.logs' }))
    expect(await screen.findByText('platform.logs.empty')).toBeInTheDocument()
    expect(screen.queryByRole('group', { name: 'platform.col.level' })).not.toBeInTheDocument()
    expect(document.querySelector('[data-ui="platform.logs.levels"]')).toBeNull()

    fireEvent.click(screen.getByRole('button', { name: 'platform.logs.widen' }))
    await waitFor(() => expect(account.listPlatformItems).toHaveBeenCalledWith('logs', undefined, '6h', 'all'))
  })

  it('pages a long catalog twenty rows at a time', async () => {
    account.listPlatformItems.mockImplementation(async (resource: string, facet?: string) => {
      if (resource === 'system' && facet === 'industries') {
        return Array.from({ length: 120 }, (_, index) => ({
          id: `i${index + 1}`,
          name: `行业${index + 1}`,
          meta: '',
          fields: [{ key: 'code', value: String(index + 1) }]
        }))
      }
      return []
    })

    renderPage()
    fireEvent.click(await screen.findByRole('button', { name: 'platform.nav.system' }))
    fireEvent.click(await screen.findByRole('button', { name: 'platform.system.reference' }))
    expect(await screen.findByText('行业20')).toBeInTheDocument()
    expect(screen.queryByText('行业21')).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'collection.page 1' })).toHaveAttribute('aria-current', 'page')

    fireEvent.click(screen.getByRole('button', { name: 'common.next' }))
    expect(screen.getByText('行业21')).toBeInTheDocument()
    expect(screen.queryByText('行业20')).not.toBeInTheDocument()
  })

  it('uses server page rows and total without slicing or deriving facets from the current page', async () => {
    account.readPlatformCatalogPage.mockImplementation(async (_resource: string, query: PlatformCatalogQuery) => ({
      rows: Array.from({ length: query.page === 1 ? 20 : 10 }, (_, index) => ({
        id: `remote-${query.page}-${index}`,
        name: `Remote ${query.page} ${index}`,
        meta: '',
        fields: []
      })),
      total: 30,
      page: query.page,
      limit: query.limit,
      facets: {
        state: [
          { value: 'draft', count: 25 },
          { value: 'published', count: 5 }
        ],
        brand: []
      }
    }))
    renderPage()
    fireEvent.click(await screen.findByRole('button', { name: 'platform.nav.models' }))
    expect(await screen.findByText('Remote 1 19')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'draft 25' })).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'collection.page 2' }))
    expect(await screen.findByText('Remote 2 9')).toBeInTheDocument()
    expect(screen.queryByText('Remote 1 0')).not.toBeInTheDocument()
    expect(account.readPlatformCatalogPage).toHaveBeenLastCalledWith(
      'models',
      expect.objectContaining({ page: 2, limit: 20 })
    )
    expect(screen.getByRole('button', { name: 'common.next' })).toBeDisabled()
    expect(screen.queryByRole('button', { name: 'platform.col.price' })).not.toBeInTheDocument()
  })

  it('sends server search, filter and sort changes and resets the page while retaining page size', async () => {
    account.readPlatformCatalogPage.mockImplementation(async (_resource: string, query: PlatformCatalogQuery) => ({
      rows: Array.from({ length: query.limit }, (_, index) => ({
        id: `s-${index}`,
        name: `Server result ${index}`,
        meta: '',
        fields: [{ key: 'kind', value: 'vendor' }]
      })),
      total: 120,
      page: query.page,
      limit: query.limit,
      facets: {
        kind: [
          { value: 'vendor', count: 120 },
          { value: 'relay', count: 80 }
        ]
      }
    }))
    renderPage()
    fireEvent.click(await screen.findByRole('button', { name: 'platform.nav.suppliers' }))
    await screen.findByText('Server result 19')
    fireEvent.click(screen.getByRole('combobox', { name: 'collection.rowsPerPage' }))
    fireEvent.click(screen.getByRole('option', { name: '50' }))
    await waitFor(() => expect(screen.getByRole('button', { name: 'collection.page 3' })).toBeEnabled())
    fireEvent.click(screen.getByRole('button', { name: 'collection.page 3' }))
    await waitFor(() =>
      expect(account.readPlatformCatalogPage).toHaveBeenLastCalledWith(
        'suppliers',
        expect.objectContaining({ page: 3, limit: 50 })
      )
    )
    fireEvent.change(screen.getByRole('searchbox', { name: 'common.search' }), { target: { value: 'literal query' } })
    await waitFor(() =>
      expect(account.readPlatformCatalogPage).toHaveBeenLastCalledWith(
        'suppliers',
        expect.objectContaining({ q: 'literal query', page: 1, limit: 50 })
      )
    )
    fireEvent.click(await screen.findByRole('button', { name: 'vendor 120' }))
    await waitFor(() =>
      expect(account.readPlatformCatalogPage).toHaveBeenLastCalledWith(
        'suppliers',
        expect.objectContaining({ filter: 'vendor', page: 1, limit: 50 })
      )
    )
    fireEvent.click(screen.getByRole('button', { name: 'platform.col.name' }))
    await waitFor(() =>
      expect(account.readPlatformCatalogPage).toHaveBeenLastCalledWith(
        'suppliers',
        expect.objectContaining({ sort: 'name', direction: 'asc', filter: 'vendor' })
      )
    )
  })

  it('withdraws cached remote rows on a failed authority refresh and offers retry', async () => {
    account.readPlatformCatalogPage.mockResolvedValue({
      rows: [{ id: 'secret-row', name: 'Scoped supplier', meta: '', fields: [] }],
      total: 1,
      page: 1,
      limit: 50,
      facets: { kind: [] }
    })
    renderPage()
    fireEvent.click(await screen.findByRole('button', { name: 'platform.nav.suppliers' }))
    await screen.findByText('Scoped supplier')
    account.readPlatformCatalogPage.mockRejectedValue(new Error('HTTP 404'))
    fireEvent.click(screen.getByRole('button', { name: 'common.refresh' }))
    await screen.findByText('platform.error.unknown')
    expect(screen.queryByText('Scoped supplier')).not.toBeInTheDocument()
    expect(screen.queryByRole('navigation', { name: 'collection.pagination' })).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'common.retry' })).toBeInTheDocument()
  })

  it('ignores old query completions after the server search changes', async () => {
    let resolveOld!: (value: unknown) => void
    const row = (name: string) => ({
      rows: [{ id: name, name, meta: '', fields: [] }],
      total: 1,
      page: 1,
      limit: 50,
      facets: { kind: [] }
    })
    account.readPlatformCatalogPage.mockImplementation(async (_resource: string, query: PlatformCatalogQuery) =>
      query.q === 'old'
        ? new Promise((resolve) => {
            resolveOld = resolve
          })
        : row(query.q || 'Initial')
    )
    renderPage()
    fireEvent.click(await screen.findByRole('button', { name: 'platform.nav.suppliers' }))
    await screen.findByText('Initial')
    const search = screen.getByRole('searchbox', { name: 'common.search' })
    fireEvent.change(search, { target: { value: 'old' } })
    await waitFor(() => expect(resolveOld).toBeTypeOf('function'))
    fireEvent.change(search, { target: { value: 'New server result' } })
    await screen.findByText('New server result')
    await act(async () => resolveOld(row('Old completion')))
    await waitFor(() => expect(screen.queryByText('Old completion')).not.toBeInTheDocument())
    expect(screen.getByText('New server result')).toBeInTheDocument()
  })

  it('isolates remote page caches when a different account binds the same team', async () => {
    let centerChanged: () => void = () => undefined
    account.subscribeCenter.mockImplementation((listener) => {
      centerChanged = listener
      return () => undefined
    })
    account.readPlatformCatalogPage.mockResolvedValue({
      rows: [{ id: 'first', name: 'First account supplier', meta: '', fields: [] }],
      total: 1,
      page: 1,
      limit: 50,
      facets: { kind: [] }
    })
    bindCenterMember('11111111-1111-4111-8111-111111111111')
    renderPage()
    fireEvent.click(await screen.findByRole('button', { name: 'platform.nav.suppliers' }))
    await screen.findByText('First account supplier')
    account.readPlatformCatalogPage.mockImplementation(() => new Promise(() => undefined))
    bindCenterMember('22222222-2222-4222-8222-222222222222')
    act(() => centerChanged())
    await waitFor(() => expect(screen.queryByText('First account supplier')).not.toBeInTheDocument())
    expect(account.readPlatformCatalogPage).toHaveBeenCalledTimes(2)
  })

  it('changes page size, jumps to a page, and preserves the size when returning to a catalog', async () => {
    account.listPlatformItems.mockImplementation(async (resource: string) =>
      resource === 'suppliers'
        ? Array.from({ length: 120 }, (_, index) => ({
            id: `s${index + 1}`,
            name: `供应商${index + 1}`,
            meta: '',
            fields: []
          }))
        : []
    )
    renderPage()
    fireEvent.click(await screen.findByRole('button', { name: 'platform.nav.suppliers' }))
    expect(await screen.findByText('供应商20')).toBeInTheDocument()
    fireEvent.click(screen.getByRole('combobox', { name: 'collection.rowsPerPage' }))
    fireEvent.click(screen.getByRole('option', { name: '10' }))
    await waitFor(() => expect(screen.queryByText('供应商11')).not.toBeInTheDocument())
    await waitFor(() => expect(screen.getByRole('button', { name: 'collection.page 12' })).toBeEnabled())
    fireEvent.click(screen.getByRole('button', { name: 'collection.page 12' }))
    expect(await screen.findByText('供应商111')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'common.next' })).toBeDisabled()
    fireEvent.click(screen.getByRole('button', { name: 'platform.nav.customers' }))
    fireEvent.click(screen.getByRole('button', { name: 'platform.nav.suppliers' }))
    expect(await screen.findByRole('combobox', { name: 'collection.rowsPerPage' })).toHaveTextContent('10')
    expect(screen.getByRole('button', { name: 'collection.page 12' })).toHaveAttribute('aria-current', 'page')
    fireEvent.change(screen.getByRole('searchbox', { name: 'common.search' }), { target: { value: '供应商1' } })
    await waitFor(() =>
      expect(screen.getByRole('button', { name: 'collection.page 1' })).toHaveAttribute('aria-current', 'page')
    )
  })

  it('opens nested features from sidebar search and synchronizes the in-page segments', async () => {
    renderPage()
    const search = await screen.findByRole('searchbox', { name: 'platform.nav.search' })
    fireEvent.change(search, { target: { value: 'platform.reference.regions' } })
    fireEvent.click(screen.getByRole('button', { name: 'platform.nav.openSection platform.reference.regions' }))
    await waitFor(() => expect(account.listPlatformItems).toHaveBeenCalledWith('system', 'regions'))
    fireEvent.change(search, { target: { value: '' } })
    fireEvent.click(screen.getByRole('button', { name: 'platform.nav.openSection platform.system.apps' }))
    await waitFor(() => expect(account.listPlatformItems).toHaveBeenCalledWith('system', 'apps'))
    fireEvent.click(screen.getByRole('button', { name: 'platform.system.reference' }))
    expect(screen.getByRole('button', { name: 'platform.nav.openSection platform.system.reference' })).toHaveAttribute(
      'aria-current',
      'location'
    )
    fireEvent.change(search, { target: { value: 'no-such-platform-feature' } })
    expect(screen.getByText('platform.nav.noMatch')).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'platform.nav.suppliers' })).not.toBeInTheDocument()
  })

  it('retains selected filters when search has no matching rows', async () => {
    renderPage()
    fireEvent.click(await screen.findByRole('button', { name: 'platform.nav.customers' }))
    const plan = await screen.findByRole('combobox', { name: 'platform.col.plan' })
    fireEvent.change(plan, { target: { value: 'team' } })
    fireEvent.change(screen.getByRole('searchbox', { name: 'common.search' }), { target: { value: '甲' } })
    expect(screen.queryByRole('button', { name: /甲/ })).not.toBeInTheDocument()
    expect(plan).toHaveValue('team')
    expect(screen.getByText('platform.nomatchQuery')).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'platform.clearFilters' }))
    expect(screen.getByRole('button', { name: /甲/ })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /乙/ })).toBeInTheDocument()
  })

  it('persists the last valid page after rows are removed', async () => {
    let count = 120
    account.listPlatformItems.mockImplementation(async (resource: string) =>
      resource === 'suppliers'
        ? Array.from({ length: count }, (_, index) => ({
            id: `s${index + 1}`,
            name: `供应商${index + 1}`,
            meta: '',
            fields: []
          }))
        : []
    )
    renderPage()
    fireEvent.click(await screen.findByRole('button', { name: 'platform.nav.suppliers' }))
    fireEvent.click(await screen.findByRole('button', { name: 'collection.page 3' }))
    await waitFor(() => expect(screen.getByRole('button', { name: 'common.refresh' })).toBeEnabled())
    count = 15
    fireEvent.click(screen.getByRole('button', { name: 'common.refresh' }))
    await waitFor(() =>
      expect(screen.getByRole('button', { name: 'collection.page 1' })).toHaveAttribute('aria-current', 'page')
    )
    count = 120
    await waitFor(() => expect(screen.getByRole('button', { name: 'common.refresh' })).toBeEnabled())
    fireEvent.click(screen.getByRole('button', { name: 'common.refresh' }))
    await screen.findByRole('button', { name: 'collection.page 3' })
    expect(screen.getByRole('button', { name: 'collection.page 1' })).toHaveAttribute('aria-current', 'page')
    expect(screen.getByText('供应商1')).toBeInTheDocument()
  })

  it('paginates customer people independently of the main catalog', async () => {
    account.listCustomerPeople.mockResolvedValue(
      Array.from({ length: 42 }, (_, index) => ({
        id: `p${index + 1}`,
        name: `人员${index + 1}`,
        meta: '',
        fields: []
      }))
    )
    renderPage()
    fireEvent.click(await screen.findByRole('button', { name: 'platform.nav.customers' }))
    fireEvent.click(await screen.findByRole('button', { name: /甲/ }))
    expect(await screen.findByText('人员20')).toBeInTheDocument()
    expect(screen.queryByText('人员21')).not.toBeInTheDocument()
    const people = document.querySelector('[data-ui="platform.customers.people"]') as HTMLElement
    fireEvent.click(within(people).getByRole('button', { name: 'common.next' }))
    expect(screen.getByText('人员21')).toBeInTheDocument()
    expect(screen.queryByText('人员20')).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: /甲/ })).toHaveAttribute('aria-expanded', 'true')
  })

  it('shows access failures with a retry and explains a denied team instead of loading forever', async () => {
    account.listMemberOrgs.mockRejectedValueOnce(new Error('offline')).mockResolvedValue([])
    renderPage()
    expect(await screen.findByText('platform.error.title')).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'platform.nav.customers' })).not.toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'common.retry' }))
    expect(await screen.findByText('platform.access.denied')).toBeInTheDocument()
  })

  it('shows the last seven days of usage on the overview with model and customer rankings', async () => {
    const usageRow = (id: string, name: string, pairs: Record<string, string>) => ({
      id,
      name,
      meta: '',
      fields: Object.entries(pairs).map(([key, value]) => ({ key, value }))
    })
    account.listPlatformUsage.mockResolvedValue([
      usageRow('day:2026-10-10', '2026-10-10', { section: 'days', tokens: '1200', requests: '8', failed: '2' }),
      usageRow('day:2026-10-11', '2026-10-11', { section: 'days', tokens: '300', requests: '2', failed: '0' }),
      usageRow('org-b', '乙公司', {
        section: 'customers',
        tokens: '1000',
        requests: '6',
        share: '66.7',
        cloud: 'read'
      }),
      usageRow('org-a', '甲公司', { section: 'customers', tokens: '500', requests: '4', share: '33.3', cloud: 'read' }),
      usageRow('org-c', '丙公司', {
        section: 'customers',
        tokens: '0',
        requests: '0',
        share: '0.0',
        cloud: 'unavailable'
      }),
      usageRow('model:deepseek-chat', 'deepseek-chat', {
        section: 'models',
        tokens: '1500',
        requests: '10',
        share: '100.0'
      })
    ])

    renderPage()
    const usage = await screen.findByText('platform.overview.usage')
    const card = usage.closest('article') as HTMLElement
    expect(account.listPlatformUsage).toHaveBeenCalledWith('7')
    expect(account.listPlatformUsage).toHaveBeenCalledTimes(1)
    const stats = card.querySelector('dl') as HTMLElement
    expect(within(stats).getByText('1.5K')).toBeInTheDocument()
    expect(within(stats).getByText('80.0%')).toBeInTheDocument()
    expect(within(stats).getByText('2')).toBeInTheDocument()
    expect(within(card).getByText('platform.usage.partial')).toBeInTheDocument()
    const models = card.querySelector('[data-ui="platform.overview.topModels"]') as HTMLElement
    expect(within(models).getByText('deepseek-chat')).toBeInTheDocument()
    expect(within(models).getByText('100.0%')).toBeInTheDocument()
    const customers = card.querySelector('[data-ui="platform.overview.topCustomers"]') as HTMLElement
    expect(
      within(customers)
        .getAllByRole('listitem')
        .map((item) => item.textContent)
    ).toEqual([expect.stringContaining('乙公司'), expect.stringContaining('甲公司')])
    expect(within(customers).queryByText('丙公司')).not.toBeInTheDocument()

    fireEvent.click(within(card).getByRole('button', { name: 'platform.overview.viewAll' }))
    expect(await screen.findByRole('heading', { name: 'platform.nav.usage' })).toBeInTheDocument()
  })

  it('reopens on the menu and segment the operator left', async () => {
    const first = renderPage()
    fireEvent.click(await screen.findByRole('button', { name: 'platform.nav.services' }))
    fireEvent.click(await screen.findByRole('button', { name: 'platform.services.skills' }))
    expect(account.listPlatformItems).toHaveBeenCalledWith('services', 'skills')
    first.unmount()
    account.listPlatformItems.mockClear()

    renderPage()
    expect(await screen.findByRole('heading', { name: 'platform.nav.services' })).toBeInTheDocument()
    expect(account.listPlatformItems).toHaveBeenCalledWith('services', 'skills')
    expect(account.listPlatformItems).not.toHaveBeenCalledWith('services', 'agents')
  })

  it('ignores a remembered view outside the closed menu set', async () => {
    localStorage.setItem('a3s.studio.platform.view:v1', JSON.stringify({ menu: 'billing', facets: { services: 'x' } }))
    renderPage()
    expect(await screen.findByRole('heading', { name: 'platform.nav.overview' })).toBeInTheDocument()
  })

  it('jumps to search with slash, clears it with escape, aligns counts and copies ids', async () => {
    const writeText = vi.fn(async () => undefined)
    Object.defineProperty(navigator, 'clipboard', { value: { writeText }, configurable: true })
    account.listPlatformItems.mockImplementation(async (resource: string) =>
      resource === 'customers'
        ? [
            {
              id: 'cust-1',
              name: '甲公司',
              meta: '',
              fields: [
                { key: 'seats', value: '12' },
                { key: 'status', value: 'active' }
              ]
            }
          ]
        : []
    )
    renderPage()
    fireEvent.click(await screen.findByRole('button', { name: 'platform.nav.customers' }))
    const seats = await screen.findByText('12')
    expect(seats.closest('td')).toHaveClass('text-right', 'tabular-nums')
    expect(screen.getByRole('columnheader', { name: 'platform.col.seats' })).toHaveClass('text-right')

    const search = screen.getByRole('searchbox', { name: 'common.search' })
    fireEvent.keyDown(window, { key: '/' })
    expect(search).toHaveFocus()
    fireEvent.change(search, { target: { value: '乙' } })
    expect(screen.queryByText('甲公司')).not.toBeInTheDocument()
    fireEvent.keyDown(search, { key: 'Escape' })
    expect(search).toHaveValue('')

    fireEvent.click(screen.getByRole('button', { name: /甲公司/ }))
    fireEvent.click(screen.getByRole('button', { name: 'common.copy' }))
    expect(writeText).toHaveBeenCalledWith('cust-1')
    expect(await screen.findByRole('button', { name: 'common.copied' })).toBeInTheDocument()
  })

  it('says so on the overview when usage cannot be read', async () => {
    account.listPlatformUsage.mockRejectedValue(new Error('boom'))
    renderPage()
    expect(await screen.findByText('platform.overview.usageFailed')).toBeInTheDocument()
  })

  it('counts runtime records on the overview and opens their menus', async () => {
    account.listPlatformItems.mockImplementation(async (resource: string) => {
      if (resource === 'gateway') {
        return [
          {
            id: 'r1',
            name: 'a.example.com',
            meta: '',
            fields: [
              { key: 'state', value: 'active' },
              { key: 'health', value: 'healthy' }
            ]
          },
          { id: 'r2', name: 'b.example.com', meta: '', fields: [{ key: 'state', value: 'active' }] }
        ]
      }
      if (resource === 'logs') {
        return Array.from({ length: 8 }, (_, index) => ({
          id: `l${index}`,
          name: `line ${index}`,
          meta: '',
          fields: [{ key: 'level', value: 'info' }]
        }))
      }
      return []
    })

    renderPage()
    const routes = await screen.findByRole('button', { name: 'platform.overview.routes platform.overview.ready 2' })
    expect(screen.getByRole('button', { name: 'platform.overview.nodes 0' })).toBeInTheDocument()
    expect(screen.getByText('line 4')).toBeInTheDocument()
    expect(screen.queryByText('line 5')).not.toBeInTheDocument()

    fireEvent.click(routes)
    expect(await screen.findByRole('heading', { name: 'platform.nav.gateway' })).toBeInTheDocument()
  })

  it('leads the overview with what needs attention and opens the menu that owns it', async () => {
    const row = (id: string, fields: Record<string, string>) => ({
      id,
      name: id,
      meta: '',
      fields: Object.entries(fields).map(([key, value]) => ({ key, value }))
    })
    account.listPlatformItems.mockImplementation(async (resource: string) => {
      if (resource === 'fleet') {
        return [
          row('n1', { state: 'ready', availability: 'online' }),
          row('n2', { state: 'unreachable', availability: 'offline' })
        ]
      }
      if (resource === 'suppliers') {
        return [
          row('s1', { status: 'active', credential: 'missing' }),
          row('s2', { status: 'active', credential: 'set' })
        ]
      }
      if (resource === 'gateway') return [row('r1', { health: 'healthy' })]
      return []
    })

    renderPage()
    const card = await screen.findByText('platform.overview.attentionTitle')
    const panel = card.closest('article') as HTMLElement
    expect(within(panel).getByRole('button', { name: 'platform.overview.issue.nodes' })).toBeInTheDocument()
    expect(within(panel).getByRole('button', { name: 'platform.overview.issue.suppliers' })).toBeInTheDocument()
    expect(within(panel).queryByRole('button', { name: 'platform.overview.issue.routes' })).not.toBeInTheDocument()
    expect(screen.getByText('platform.updated')).toBeInTheDocument()

    const run = screen.getByRole('group', { name: 'platform.group.runtime' })
    expect(within(run).getByRole('button', { name: 'platform.nav.fleet' })).toHaveAccessibleDescription(
      'platform.nav.attention'
    )
    expect(within(run).getByRole('button', { name: 'platform.nav.gateway' })).not.toHaveAccessibleDescription()
    const fleetBadge = within(run).getByRole('button', { name: 'platform.nav.fleet' })
    expect(fleetBadge.querySelector('[data-ui="platform.nav.attention"]')).toHaveTextContent('1')

    fireEvent.click(within(panel).getByRole('button', { name: 'platform.overview.issue.suppliers' }))
    expect(await screen.findByRole('heading', { name: 'platform.nav.suppliers' })).toBeInTheDocument()
    expect(screen.getByText('platform.updated')).toBeInTheDocument()
  })

  it('says all clear when every check reads and nothing is wrong', async () => {
    renderPage()
    expect(await screen.findByText('platform.overview.attentionClear')).toBeInTheDocument()
  })

  it('shows row placeholders while a list loads, then when it was read', async () => {
    let release: (rows: unknown[]) => void = () => undefined
    account.listPlatformItems.mockImplementation((resource: string) =>
      resource === 'customers' ? new Promise((resolve) => (release = resolve)) : Promise.resolve([])
    )
    renderPage()
    fireEvent.click(await screen.findByRole('button', { name: 'platform.nav.customers' }))
    expect(document.querySelector('[data-ui="platform.skeleton"]')).not.toBeNull()
    release([{ id: 'c1', name: '甲公司', meta: '', fields: [] }])
    expect(await screen.findByText('甲公司')).toBeInTheDocument()
    expect(document.querySelector('[data-ui="platform.skeleton"]')).toBeNull()
    expect(screen.getByText('platform.updated')).toHaveAttribute('title', expect.stringMatching(/^\d{4}-\d{2}-\d{2} /))
  })

  it('resumes a list where it was left, but opens a flagged list unfiltered from the overview', async () => {
    const row = (id: string, fields: Record<string, string>) => ({
      id,
      name: id,
      meta: '',
      fields: Object.entries(fields).map(([key, value]) => ({ key, value }))
    })
    account.listPlatformItems.mockImplementation(async (resource: string, kind?: string) => {
      if (resource === 'customers') return [row('甲公司', { status: 'active' }), row('乙公司', { status: 'active' })]
      if (resource === 'fleet' && kind === 'fleet') {
        return [
          row('node-ok', { state: 'ready', availability: 'online' }),
          row('node-down', { state: 'unreachable', availability: 'offline' })
        ]
      }
      return []
    })
    renderPage()
    fireEvent.click(await screen.findByRole('button', { name: 'platform.nav.customers' }))
    expect(await screen.findByText('乙公司')).toBeInTheDocument()
    fireEvent.change(screen.getByRole('searchbox', { name: 'common.search' }), { target: { value: '甲' } })
    expect(screen.queryByText('乙公司')).not.toBeInTheDocument()

    fireEvent.click(screen.getByRole('button', { name: 'platform.nav.channels' }))
    expect(await screen.findByRole('heading', { name: 'platform.nav.channels' })).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'platform.nav.customers' }))
    expect(await screen.findByText('甲公司')).toBeInTheDocument()
    expect(screen.getByRole('searchbox', { name: 'common.search' })).toHaveValue('甲')
    expect(screen.queryByText('乙公司')).not.toBeInTheDocument()

    fireEvent.change(screen.getByRole('searchbox', { name: 'common.search' }), { target: { value: 'zzz' } })
    expect(screen.getByText('platform.nomatchQuery')).toBeInTheDocument()

    fireEvent.click(screen.getByRole('button', { name: 'platform.nav.fleet' }))
    expect(await screen.findByText('node-down')).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'ready 1' }))
    expect(screen.queryByText('node-down')).not.toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'platform.fleet.topology' }))

    fireEvent.click(screen.getByRole('button', { name: 'platform.nav.overview' }))
    fireEvent.click(await screen.findByRole('button', { name: 'platform.overview.issue.nodes' }))
    expect(await screen.findByText('node-down')).toBeInTheDocument()
    expect(screen.getByText('node-ok')).toBeInTheDocument()
  })

  it('does not show the previous window as the answer while a new window loads', async () => {
    let release: (rows: unknown[]) => void = () => undefined
    account.listPlatformItems.mockImplementation(
      (resource: string, _facet?: string, window?: string): Promise<unknown[]> => {
        if (resource !== 'logs') return Promise.resolve([])
        if (window === '6h') return new Promise((resolve) => (release = resolve))
        if (window === '24h') return new Promise(() => undefined)
        return Promise.resolve([])
      }
    )
    renderPage()
    fireEvent.click(await screen.findByRole('button', { name: 'platform.nav.logs' }))
    expect(await screen.findByText('platform.logs.empty')).toBeInTheDocument()

    fireEvent.click(screen.getByRole('button', { name: 'platform.logs.window.6h' }))
    await waitFor(() => expect(document.querySelector('[data-ui="platform.skeleton"]')).not.toBeNull())
    expect(screen.queryByText('platform.logs.empty')).not.toBeInTheDocument()
    release([
      { id: 'l0', name: 'newest ok', meta: '', fields: [{ key: 'level', value: 'info' }] },
      { id: 'l1', name: 'disk full', meta: '', fields: [{ key: 'level', value: 'error' }] }
    ])
    expect(await screen.findByText('disk full')).toBeInTheDocument()
    const lines = screen.getAllByRole('row').map((row) => row.textContent ?? '')
    expect(lines.findIndex((line) => line.includes('newest ok'))).toBeLessThan(
      lines.findIndex((line) => line.includes('disk full'))
    )
    expect(screen.queryByText('platform.problemsFirst')).not.toBeInTheDocument()
    expect(document.querySelector('[data-ui="platform.list"]')).not.toHaveAttribute('aria-busy')

    fireEvent.click(screen.getByRole('button', { name: 'platform.logs.window.24h' }))
    expect(screen.getByText('disk full')).toBeInTheDocument()
    expect(document.querySelector('[data-ui="platform.list"]')).toHaveAttribute('aria-busy', 'true')
  })

  it('filters log lines by level on the server with whole-window counts and opens the whole message', async () => {
    const message = 'upstream refused the connection after three retries while reading the model catalog'
    const line = (id: string, name: string, level: string) => ({
      id,
      name,
      meta: '',
      fields: [
        { key: 'level', value: level },
        { key: 'service', value: 'api' }
      ]
    })
    account.listPlatformItems.mockImplementation(
      async (resource: string, _facet?: string, _window?: string, level?: string) => {
        if (resource !== 'logs') return []
        if (level === 'error') return [line('l1', message, 'error')]
        return [line('l1', message, 'error'), line('l2', 'served', 'info'), line('l3', 'served again', 'info')]
      }
    )
    account.listPlatformLogLevels.mockResolvedValue(
      [
        ['all', '450'],
        ['error', '120'],
        ['warn', '0'],
        ['info', '330'],
        ['debug', '0'],
        ['trace', '0']
      ].map(([id, count]) => ({ id, name: id, meta: '', fields: [{ key: 'count', value: count }] }))
    )

    renderPage()
    fireEvent.click(await screen.findByRole('button', { name: 'platform.nav.logs' }))
    expect(await screen.findByText('served')).toBeInTheDocument()
    expect(account.listPlatformLogLevels).toHaveBeenCalledWith('1h')
    expect(screen.getByRole('button', { name: 'common.all 450' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'info 330' })).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'warn 0' })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'error 1' })).not.toBeInTheDocument()
    expect(screen.getByText('platform.logs.truncated')).toBeInTheDocument()

    fireEvent.click(screen.getByRole('button', { name: 'error 120' }))
    expect(account.listPlatformItems).toHaveBeenCalledWith('logs', undefined, '1h', 'error')
    await vi.waitFor(() => expect(screen.queryByText('served')).not.toBeInTheDocument())
    expect(screen.queryByText('platform.logs.truncated')).not.toBeInTheDocument()

    const row = screen.getByRole('button', { name: new RegExp('upstream refused') })
    fireEvent.click(row)
    expect(row).toHaveAttribute('aria-expanded', 'true')
    expect(screen.getByText(message)).not.toHaveClass('truncate')
  })
})
