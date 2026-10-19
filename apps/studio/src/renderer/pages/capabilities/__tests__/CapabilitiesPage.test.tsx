import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { SWRConfig } from 'swr'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { clearPageEntrySession } from '@renderer/components/layout/PageEntry'
import { MemberOrgError } from '@renderer/data/memberOrgs'

import type * as CapabilitiesData from '../capabilitiesData'
import type { CapabilityEntry, CapabilityKind, CapabilityQuery } from '../capabilitiesData'
import CapabilitiesPage from '../CapabilitiesPage'
import { formatConnectorPaste } from '../connectorPaste'
import type { CustomConnectorBody } from '../customConnector'
import { customConnectorBody, customConnectorIssues } from '../customConnector'

const navigate = vi.fn()
const api = vi.hoisted(() => ({
  listCapabilities: vi.fn(),
  uninstallCapability: vi.fn(),
  prepareBuiltinSkill: vi.fn().mockResolvedValue(undefined),
  uploadSkillPackage: vi.fn(),
  listCustomConnectors: vi.fn().mockResolvedValue([]),
  saveCustomConnector: vi.fn().mockResolvedValue(undefined),
  removeCustomConnector: vi.fn().mockResolvedValue(undefined)
}))
const toastError = vi.hoisted(() => vi.fn())
const toastSuccess = vi.hoisted(() => vi.fn())

// The renderer setup mocks the @a3s-lab/ui barrel, and the installed @a3s-lab/ui
// copy lags the source tree. This file asserts the confirm panel the window
// actually loads, so overlay only that component.
vi.mock('@a3s-lab/ui', async () => {
  const actual = await vi.importActual<Record<string, unknown>>('@a3s-lab/ui')
  const source = await import('../../../../../packages/ui/src/components/console/PopConfirm')
  const empty = await import('../../../../../packages/ui/src/components/composites/empty-state')
  return { ...actual, PopConfirm: source.PopConfirm, EmptyState: empty.EmptyState }
})

vi.mock('react-i18next', () => ({
  initReactI18next: { type: '3rdParty', init: vi.fn() },
  useTranslation: () => ({
    t: (key: string) => key
  })
}))

vi.mock('@tanstack/react-router', () => ({
  useNavigate: () => navigate,
  getRouteApi: () => ({ useSearch: () => ({}) }),
  useParams: () => ({})
}))

vi.mock('@renderer/hooks/useMcpServer', () => ({
  useMcpServers: () => ({
    mcpServers: [],
    isLoading: false,
    addMcpServer: vi.fn(),
    reorderMcpServers: vi.fn(),
    refetch: vi.fn()
  }),
  useMcpServer: () => ({ server: undefined, isLoading: false, updateMcpServer: vi.fn() })
}))

vi.mock('@renderer/ipc', () => ({
  ipcApi: { request: vi.fn().mockResolvedValue([]) }
}))

vi.mock('@renderer/pages/settings/DependenciesSettings/EnvironmentDependencies', () => ({
  default: () => null
}))

vi.mock('../../settings/McpSettings/AddMcpServerModal', () => ({ default: () => null }))
vi.mock('../../settings/McpSettings/QuickCreateMcpServerDialog', () => ({ default: () => null }))
vi.mock('../../settings/McpSettings/McpProtocolInstallDialog', () => ({ default: () => null }))

vi.mock('@renderer/services/toast', () => ({
  toast: { error: toastError, success: toastSuccess }
}))

vi.mock('@data/hooks/usePreference', () => ({
  usePreference: () => ['', vi.fn()]
}))

vi.mock('../capabilitiesData', async (importOriginal) => ({
  ...(await importOriginal<typeof CapabilitiesData>()),
  ...api
}))

function entry(slug: string, patch: Partial<CapabilityEntry> = {}): CapabilityEntry {
  return {
    slug,
    kind: 'connector',
    category: 'office',
    name: slug,
    description: `${slug} description`,
    color: '#171717',
    featured: false,
    installed: false,
    config: [],
    ...patch
  }
}

const CATALOG = [
  entry('ifind', { name: '同花顺iFinD', featured: true }),
  entry('zoom', { name: 'Zoom' }),
  entry('deep-research', { kind: 'skill', name: 'Deep Research', category: 'other', featured: true })
]

function categoriesFor(kind: CapabilityKind, rows: CapabilityEntry[]) {
  const counts = new Map<string, number>()
  for (const row of rows) {
    if (row.kind !== kind) continue
    counts.set(row.category, (counts.get(row.category) ?? 0) + 1)
  }
  return [...counts.entries()].map(([id, count]) => ({ id, count }))
}

function pageFor(query: CapabilityQuery, rows: CapabilityEntry[] = CATALOG) {
  const page = query.page && query.page > 0 ? query.page : 1
  const pageSize = query.pageSize && query.pageSize > 0 ? query.pageSize : 24
  const needle = query.q?.trim().toLocaleLowerCase() ?? ''
  const matched = rows.filter((row) => {
    if (row.kind !== query.kind) return false
    if (query.featured && !row.featured) return false
    if (query.installed && !row.installed) return false
    if (query.category && row.category !== query.category) return false
    if (needle && !`${row.name} ${row.description}`.toLocaleLowerCase().includes(needle)) return false
    return true
  })
  return {
    items: matched.slice((page - 1) * pageSize, page * pageSize),
    total: matched.length,
    page,
    pageSize,
    categories: categoriesFor(query.kind, rows)
  }
}

function renderPage(
  kind: CapabilityKind = 'connector',
  swr: Record<string, unknown> = {},
  onUseSkill?: (request: { name: string; prompt: string }) => void
) {
  return render(
    <SWRConfig value={{ provider: () => new Map(), dedupingInterval: 0, ...swr }}>
      <CapabilitiesPage kind={kind} onUseSkill={onUseSkill} />
    </SWRConfig>
  )
}

const shellToken = 'ab'.repeat(32)

function jsonResponse(payload: unknown, status = 200): Response {
  return new Response(JSON.stringify(payload), {
    status,
    headers: { 'content-type': 'application/json' }
  })
}

function installStream() {
  let pushChunk: (chunk: string) => void = () => undefined
  let closeStream: () => void = () => undefined
  let opened = false
  const encoder = new TextEncoder()
  const response = {
    ok: true,
    status: 200,
    headers: {
      get: (name: string) => (name.toLowerCase() === 'content-type' ? 'application/x-ndjson' : null)
    },
    body: {
      getReader: () => {
        opened = true
        const queued: Array<{ done: boolean; value?: Uint8Array }> = []
        let pending: ((value: { done: boolean; value?: Uint8Array }) => void) | null = null
        const deliver = (item: { done: boolean; value?: Uint8Array }) => {
          if (pending) {
            const resolve = pending
            pending = null
            resolve(item)
          } else {
            queued.push(item)
          }
        }
        pushChunk = (chunk: string) => deliver({ done: false, value: encoder.encode(chunk) })
        closeStream = () => deliver({ done: true })
        return {
          read: () =>
            new Promise<{ done: boolean; value?: Uint8Array }>((resolve) => {
              const next = queued.shift()
              if (next) resolve(next)
              else pending = resolve
            })
        }
      }
    },
    text: async () => {
      throw new Error('stream')
    }
  }
  return {
    response: response as unknown as Response,
    opened: () => opened,
    push: (chunk: string) => pushChunk(chunk),
    close: () => closeStream()
  }
}

describe('CapabilitiesPage', () => {
  let fetchMock: ReturnType<typeof vi.fn>

  afterEach(() => {
    vi.useRealTimers()
    vi.unstubAllGlobals()
    delete (globalThis as { __A3S_STUDIO_RUNTIME__?: unknown }).__A3S_STUDIO_RUNTIME__
  })

  beforeEach(() => {
    navigate.mockReset()
    toastError.mockReset()
    toastSuccess.mockReset()
    localStorage.clear()
    api.listCapabilities.mockReset().mockImplementation((query: CapabilityQuery) => Promise.resolve(pageFor(query)))
    api.uninstallCapability.mockReset().mockResolvedValue(undefined)
    api.listCustomConnectors.mockReset().mockResolvedValue([])
    api.saveCustomConnector.mockReset().mockResolvedValue(undefined)
    api.removeCustomConnector.mockReset().mockResolvedValue(undefined)
    sessionStorage.clear()
    Object.defineProperty(globalThis, '__A3S_STUDIO_RUNTIME__', {
      configurable: true,
      value: { apiBaseUrl: 'http://127.0.0.1:4000', token: shellToken }
    })
    fetchMock = vi.fn(async () => jsonResponse({ code: 0, data: { installed: true } }))
    vi.stubGlobal('fetch', fetchMock)
  })

  it('opens the connector catalog in the plugin-store chrome', async () => {
    renderPage()

    expect(screen.getByRole('radio', { name: 'plugins.tab.market' })).toHaveAttribute('aria-checked', 'true')
    expect(screen.getByRole('heading', { level: 1 })).toHaveTextContent('capabilities.tab.connectors')
    expect(screen.getByRole('searchbox')).toHaveAttribute('placeholder', 'capabilities.search.connectors')
    expect(document.querySelector('[data-ui="capabilities.categories"]')).toHaveClass(
      '@[640px]/capabilities:w-[260px]',
      '@[640px]/capabilities:min-w-[260px]'
    )
    expect(screen.getByRole('button', { name: 'capabilities.category.all' })).toHaveAttribute('aria-current', 'page')
    expect(await screen.findByText('Zoom')).toBeInTheDocument()
    expect(api.listCapabilities).toHaveBeenCalledWith(
      expect.objectContaining({ kind: 'connector', page: 1, pageSize: 24 })
    )
    expect(screen.getAllByText('同花顺iFinD').length).toBeGreaterThan(0)
    expect(screen.getByText('zoom description')).toBeInTheDocument()
    const card = screen.getByText('Zoom').closest('[data-ui="capabilities.card"]')
    expect(card).toHaveClass('rounded-[14px]', 'flex-row', 'items-center')
    expect(card?.querySelector('[data-ui="capabilities.mark"]')).toHaveTextContent('Z')
    expect(card?.querySelector('[data-ui="capabilities.ribbon"]')).toBeNull()
    const featured = screen.getByText('同花顺iFinD').closest('[data-ui="capabilities.card"]')
    const ribbon = featured?.querySelector('[data-ui="capabilities.ribbon"]')
    expect(ribbon).toHaveTextContent('capabilities.category.featured')
    expect(ribbon).toHaveClass('-rotate-45', 'bg-[#3dba6c]', 'text-white', 'top-1.5', '-left-[26px]')
    expect(document.querySelector('[data-ui="capabilities.grid"]')).toHaveClass(
      'grid-cols-1',
      'w-full',
      '@[720px]/catalog-grid:grid-cols-2'
    )
    expect(screen.queryByRole('button', { name: 'capabilities.category.code' })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'capabilities.category.science' })).not.toBeInTheDocument()
    const refresh = screen.getByRole('button', { name: 'common.refresh' })
    expect(refresh).toHaveClass('size-[30px]')
    expect(refresh.querySelector('svg')).toHaveAttribute('stroke-width', '1.7')
    expect(refresh.querySelector('svg')).toHaveClass('text-current')
    const create = screen.getByRole('button', { name: 'capabilities.create' })
    expect(create).toHaveClass('h-[30px]', 'bg-[#171717]')
    expect(create.querySelector('svg')).toHaveAttribute('stroke-width', '1.7')
    expect(create.querySelector('svg')).toHaveClass('text-current')
    expect(screen.queryByRole('button', { name: 'capabilities.manage' })).not.toBeInTheDocument()

    fireEvent.click(screen.getByRole('button', { name: 'capabilities.category.office' }))
    expect(screen.getByRole('button', { name: 'capabilities.category.office' })).toHaveAttribute('aria-current', 'page')
    await waitFor(() =>
      expect(api.listCapabilities).toHaveBeenCalledWith(
        expect.objectContaining({ kind: 'connector', category: 'office', page: 1 })
      )
    )

    fireEvent.click(screen.getByRole('button', { name: 'capabilities.category.featured' }))
    await waitFor(() =>
      expect(api.listCapabilities).toHaveBeenCalledWith(
        expect.objectContaining({ kind: 'connector', featured: true, page: 1 })
      )
    )
    await waitFor(() => expect(screen.queryByText('Zoom')).not.toBeInTheDocument())

    fireEvent.click(screen.getByRole('radio', { name: 'plugins.tab.installed' }))
    await waitFor(() =>
      expect(api.listCapabilities).toHaveBeenCalledWith(
        expect.objectContaining({ kind: 'connector', featured: true, installed: true, page: 1 })
      )
    )

    fireEvent.change(screen.getByRole('searchbox'), { target: { value: 'zoom' } })
    await waitFor(() =>
      expect(api.listCapabilities).toHaveBeenCalledWith(
        expect.objectContaining({ kind: 'connector', q: 'zoom', featured: true, installed: true, page: 1 })
      )
    )
    expect(api.listCapabilities.mock.calls.every(([query]) => query.kind === 'connector')).toBe(true)
  })

  it('opens the skills catalog as its own plugin-store page', async () => {
    renderPage('skill')

    expect(screen.getByRole('heading', { level: 1 })).toHaveTextContent('capabilities.tab.skills')
    expect(screen.getByRole('searchbox')).toHaveAttribute('placeholder', 'capabilities.search.skills')
    const create = screen.getByRole('button', { name: 'capabilities.create' })
    expect(create).toHaveClass('h-[30px]', 'bg-[#171717]')
    expect(create.querySelector('svg')).toHaveAttribute('stroke-width', '1.7')
    expect(create.querySelector('svg')).toHaveClass('text-current')
    expect(screen.queryByRole('button', { name: 'capabilities.manage' })).not.toBeInTheDocument()
    expect(await screen.findByText('Deep Research')).toBeInTheDocument()
    expect(screen.queryByText('同花顺iFinD')).not.toBeInTheDocument()
    expect(screen.getByText('Deep Research').closest('[data-ui="capabilities.card"]')).toHaveClass('rounded-[14px]')
    expect(
      screen
        .getByText('Deep Research')
        .closest('[data-ui="capabilities.card"]')
        ?.querySelector('[data-ui="capabilities.ribbon"]')
    ).toHaveTextContent('capabilities.category.featured')
    expect(screen.queryByRole('button', { name: 'capabilities.category.office' })).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'capabilities.category.other' })).toBeInTheDocument()
    expect(api.listCapabilities).toHaveBeenCalledWith(expect.objectContaining({ kind: 'skill', page: 1 }))
    const user = userEvent.setup()
    await user.click(screen.getByRole('button', { name: 'capabilities.create' }))
    const menu = await screen.findByRole('menu', { name: 'capabilities.create' })
    for (const name of [
      'capabilities.skill.menu.upload',
      'capabilities.skill.menu.create'
    ]) {
      const item = within(menu).getByRole('menuitem', { name })
      expect(item.querySelector('svg')).toHaveAttribute('stroke-width', '1.7')
      expect(item.querySelector('svg')).toHaveClass('text-current')
    }
    expect(navigate).not.toHaveBeenCalled()
    await user.keyboard('{Escape}')

    fireEvent.change(screen.getByRole('searchbox'), { target: { value: 'deep' } })
    await waitFor(() =>
      expect(api.listCapabilities).toHaveBeenCalledWith(expect.objectContaining({ kind: 'skill', q: 'deep', page: 1 }))
    )
    expect(api.listCapabilities.mock.calls.every(([query]) => query.kind === 'skill')).toBe(true)

    fireEvent.click(screen.getByText('Deep Research'))
    const dialog = await screen.findByRole('dialog')
    expect(dialog.querySelector('[data-ui="capabilities.skill-dialog"]')).toBeTruthy()
    expect(within(dialog).getByText('capabilities.skill.about')).toBeInTheDocument()
    expect(within(dialog).getByText('capabilities.skill.version')).toBeInTheDocument()
    expect(within(dialog).getByText('—')).toBeInTheDocument()
  })

  it('leaves plugin packages off the skills list', async () => {
    const rows = [
      entry('domain-knowledge', {
        kind: 'plugin',
        name: '领域知识',
        installed: true,
        surfaces: ['skill', 'okf']
      }),
      entry('deep-research', { kind: 'skill', name: 'Deep Research', category: 'other', featured: true })
    ]
    api.listCapabilities.mockImplementation((query: CapabilityQuery) => Promise.resolve(pageFor(query, rows)))
    renderPage('skill')
    expect(await screen.findByText('Deep Research')).toBeInTheDocument()
    expect(screen.queryByText('领域知识')).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'plugins.okf.configure' })).not.toBeInTheDocument()
    expect(api.listCapabilities.mock.calls.every(([query]) => query.kind === 'skill')).toBe(true)
  })

  it('installs a skill from a shortcut and asks the home composer to insert it', async () => {
    const onUseSkill = vi.fn()
    api.listCapabilities.mockImplementation((query: CapabilityQuery) =>
      Promise.resolve(
        pageFor(query, [
          entry('ppt', {
            kind: 'skill',
            name: 'PPT 演示文稿',
            description: 'PowerPoint 演示文稿生成',
            version: '1.0.2',
            prompts: ['生成一份客户汇报 PPT', '把大纲做成海外 launch deck']
          })
        ])
      )
    )
    renderPage('skill', {}, onUseSkill)
    fireEvent.click(await screen.findByText('PPT 演示文稿'))
    const dialog = await screen.findByRole('dialog')
    expect(within(dialog).getByText('capabilities.skill.try')).toBeInTheDocument()
    expect(within(dialog).getByText('v1.0.2')).toBeInTheDocument()
    fireEvent.click(within(dialog).getByRole('button', { name: '生成一份客户汇报 PPT' }))
    await waitFor(() =>
      expect(fetchMock).toHaveBeenCalledWith(
        'http://127.0.0.1:4000/api/capabilities/installations/ppt',
        expect.objectContaining({ method: 'PUT' })
      )
    )
    await waitFor(() =>
      expect(onUseSkill).toHaveBeenCalledWith({ name: 'PPT 演示文稿', prompt: '生成一份客户汇报 PPT' })
    )
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument())
  })

  it('spins the refresh icon while the catalog reloads', async () => {
    renderPage()
    expect(await screen.findByText('Zoom')).toBeInTheDocument()
    let release: (page: ReturnType<typeof pageFor>) => void = () => undefined
    api.listCapabilities.mockImplementation(
      () =>
        new Promise((resolve) => {
          release = resolve
        })
    )
    fireEvent.click(screen.getByRole('button', { name: 'common.refresh' }))
    await waitFor(() =>
      expect(screen.getByRole('button', { name: 'common.refresh' }).querySelector('svg')).toHaveClass('animate-spin')
    )
    expect(screen.getByRole('button', { name: 'common.refresh' }).querySelector('svg')).toHaveStyle({
      animationDuration: '0.7s'
    })
    release(pageFor({ kind: 'connector', page: 1, pageSize: 24 }))
    await waitFor(() =>
      expect(screen.getByRole('button', { name: 'common.refresh' }).querySelector('svg')).not.toHaveClass(
        'animate-spin'
      )
    )
    expect(api.listCapabilities.mock.calls.length).toBeGreaterThan(1)
  })

  it('installs and uninstalls through the OS', async () => {
    renderPage()
    await screen.findByText('Zoom')

    const zoomCard = screen.getByText('Zoom').closest('[data-ui="capabilities.card"]')!
    fireEvent.click(zoomCard.querySelector('button')!)

    await waitFor(() =>
      expect(fetchMock).toHaveBeenCalledWith(
        'http://127.0.0.1:4000/api/capabilities/installations/zoom',
        expect.objectContaining({ method: 'PUT' })
      )
    )
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    await waitFor(() =>
      expect(zoomCard.querySelector('button')).toHaveAttribute('aria-label', 'capabilities.uninstall')
    )
    expect(zoomCard.querySelector('button')).toHaveAttribute('aria-pressed', 'true')

    fireEvent.click(zoomCard.querySelector('button')!)
    // The uninstall click opens an anchored confirm instead of uninstalling.
    const confirmDialog = await screen.findByRole('dialog')
    expect(confirmDialog).toHaveClass('fixed')
    expect(confirmDialog).toHaveStyle({ position: 'fixed', width: '18.5rem' })
    expect(confirmDialog.parentElement).toBe(document.body)
    const danger = within(confirmDialog).getByText('capabilities.uninstall')
    expect(danger).toHaveStyle({ backgroundColor: '#c23b3b' })
    expect(api.uninstallCapability).not.toHaveBeenCalled()
    fireEvent.click(within(confirmDialog).getByText('capabilities.uninstall'))
    await waitFor(() => expect(api.uninstallCapability).toHaveBeenCalledWith('zoom'))
    // Confirming also re-reads the catalog so the shell reconciles local cleanup.
    await waitFor(() => expect(api.listCapabilities.mock.calls.length).toBeGreaterThan(1))
    await waitFor(() => expect(zoomCard.querySelector('button')).toHaveAttribute('aria-label', 'capabilities.install'))
  })

  it('sends one installation request while another is in flight', async () => {
    let release: () => void = () => undefined
    fetchMock.mockImplementation((input: RequestInfo | URL, init?: RequestInit) => {
      const url = String(input)
      if (init?.method === 'PUT' && url.endsWith('/installations/zoom')) {
        return new Promise((resolve) => {
          release = () => resolve(jsonResponse({ code: 0, data: { installed: true } }))
        })
      }
      return Promise.resolve(jsonResponse({ code: 0, data: { installed: true } }))
    })
    renderPage()
    await screen.findByText('Zoom')

    const zoomCard = screen.getByText('Zoom').closest('[data-ui="capabilities.card"]')!
    const zoom = zoomCard.querySelector('button')!
    const other = screen.getByText('同花顺iFinD').closest('[data-ui="capabilities.card"]')!.querySelector('button')!
    fireEvent.click(zoom)
    fireEvent.click(zoom)
    await waitFor(() => expect(zoom).toBeDisabled())
    expect(other).toBeDisabled()
    fireEvent.click(other)

    const puts = fetchMock.mock.calls.filter((call) => call[1]?.method === 'PUT')
    expect(puts).toHaveLength(1)
    expect(String(puts[0]?.[0])).toBe('http://127.0.0.1:4000/api/capabilities/installations/zoom')
    expect(zoomCard.querySelector('[data-ui="capabilities.applying"]')).toHaveTextContent('capabilities.installing')
    expect(zoomCard.querySelector('[role="progressbar"]')).toBeNull()
    expect(zoomCard.textContent ?? '').not.toMatch(/\d+\s*%/)
    expect(zoomCard.textContent ?? '').not.toMatch(/\d+\/\d+/)
    console.log(
      `connector applying without percent: ${zoomCard.querySelector('[data-ui="capabilities.applying"]')?.textContent} progressbar=absent`
    )
    expect(zoom).toHaveAttribute('aria-pressed', 'false')
    release()
    // The install flips the action to the PopConfirm-wrapped uninstall button —
    // a different node — so re-query instead of holding the stale reference.
    await waitFor(() =>
      expect(zoomCard.querySelector('button')).toHaveAttribute('aria-label', 'capabilities.uninstall')
    )
    expect(zoomCard.querySelector('[data-ui="capabilities.applying"]')).toBeNull()
    expect(other).toHaveAttribute('aria-label', 'capabilities.install')
  })

  it('shows byte progress before a skill is installed', async () => {
    const stream = installStream()
    fetchMock.mockImplementation(async (input: RequestInfo | URL, init?: RequestInit) => {
      if (init?.method === 'PUT' && String(input).endsWith('/installations/deep-research')) return stream.response
      return jsonResponse({ code: 0, data: { installed: true } })
    })
    renderPage('skill')
    await screen.findByText('Deep Research')

    const card = screen.getByText('Deep Research').closest('[data-ui="capabilities.card"]')!
    fireEvent.click(card.querySelector('button')!)
    await waitFor(() => expect(stream.opened()).toBe(true))
    stream.push('{"received":10,"total":40}\n')
    const bar = () => card.querySelector('[role="progressbar"]')
    await waitFor(() => {
      expect(bar()).toHaveAttribute('aria-valuenow', '10')
      expect(bar()).toHaveAttribute('aria-valuemax', '40')
    })
    expect(bar()).toHaveAttribute('data-ui', 'capabilities.progress')
    expect(bar()).toHaveTextContent('10/40')
    expect(card.querySelector('button')).toHaveAttribute('aria-pressed', 'false')
    expect(card.querySelector('button')).not.toHaveAttribute('aria-label', 'capabilities.uninstall')
    console.log(`progress before installed: ${bar()?.textContent} aria-pressed=false`)
    stream.push('{"received":30,"total":40}\n')
    await waitFor(() => expect(bar()).toHaveAttribute('aria-valuenow', '30'))
    expect(bar()).toHaveTextContent('30/40')
    console.log(`progress advanced: ${bar()?.textContent} aria-pressed=false`)
    stream.push('{"installed":true}\n')
    stream.close()
    await waitFor(() => expect(card.querySelector('button')).toHaveAttribute('aria-pressed', 'true'))
    expect(card.querySelector('[role="progressbar"]')).toBeNull()
    expect(card.querySelector('button')).toHaveAttribute('aria-label', 'capabilities.uninstall')
  })

  it('keeps a rejected skill download uninstalled and offers retry', async () => {
    const stream = installStream()
    fetchMock.mockImplementation(async (input: RequestInfo | URL, init?: RequestInit) => {
      if (init?.method === 'PUT' && String(input).endsWith('/installations/deep-research')) return stream.response
      return jsonResponse({ code: 0, data: { installed: true } })
    })
    renderPage('skill')
    await screen.findByText('Deep Research')
    api.listCapabilities.mockImplementation((query: CapabilityQuery) =>
      Promise.resolve({
        ...pageFor(query),
        items: pageFor(query).items.map((row) => (row.slug === 'deep-research' ? { ...row, installed: true } : row))
      })
    )

    const card = screen.getByText('Deep Research').closest('[data-ui="capabilities.card"]')!
    fireEvent.click(card.querySelector('button')!)
    await waitFor(() => expect(stream.opened()).toBe(true))
    stream.push('{"received":10,"total":40}\n')
    stream.push('{"received":30,"total":40}\n')
    stream.push('{"error":"bundle download returned HTTP 502"}\n')
    stream.close()

    await waitFor(() => expect(card.querySelector('button')).toHaveAttribute('aria-label', 'capabilities.retry'))
    expect(card.querySelector('button')).toHaveAttribute('aria-pressed', 'false')
    console.log(`failed download retry: ${card.querySelector('button')?.getAttribute('aria-label')} aria-pressed=false`)
    expect(toastError).toHaveBeenCalledWith(expect.stringContaining('bundle download returned HTTP 502'))
    expect(card.querySelector('[role="progressbar"]')).toBeNull()
  })

  it('does not list a plugin package among skills', async () => {
    const rows = [
      entry('a3s-approval', {
        kind: 'plugin',
        name: '审批',
        category: 'other',
        installed: true,
        description: '提交和查看审批。'
      }),
      entry('deep-research', {
        kind: 'skill',
        name: 'Deep Research',
        category: 'other',
        featured: true,
        installed: true
      })
    ]
    api.listCapabilities.mockImplementation((query: CapabilityQuery) => Promise.resolve(pageFor(query, rows)))
    renderPage('skill')

    expect(await screen.findByText('Deep Research')).toBeInTheDocument()
    expect(screen.queryByText('审批')).not.toBeInTheDocument()
    const card = screen.getByText('Deep Research').closest('[data-ui="capabilities.card"]')!
    fireEvent.click(card.querySelector('button')!)
    const confirm = await screen.findByRole('dialog')
    expect(confirm).toHaveClass('ui-popconfirm')
    fireEvent.click(within(confirm).getByText('common.cancel'))
    expect(api.uninstallCapability).not.toHaveBeenCalled()
  })

  it('reports a failed uninstall without flipping the row', async () => {
    api.listCapabilities.mockImplementation((query: CapabilityQuery) =>
      Promise.resolve(pageFor(query, [entry('zoom', { name: 'Zoom', installed: true })]))
    )
    api.uninstallCapability.mockRejectedValueOnce(new MemberOrgError('still installed', 500))
    renderPage()
    await screen.findByText('Zoom')

    const button = screen.getByText('Zoom').closest('[data-ui="capabilities.card"]')!.querySelector('button')!
    expect(button).toHaveAttribute('aria-label', 'capabilities.uninstall')
    fireEvent.click(button)
    fireEvent.click(within(await screen.findByRole('dialog')).getByText('capabilities.uninstall'))

    await waitFor(() => expect(toastError).toHaveBeenCalled())
    expect(button).toHaveAttribute('aria-label', 'capabilities.uninstall')
    expect(button).toHaveAttribute('aria-pressed', 'true')
  })

  it('reports a failed install without flipping the row', async () => {
    fetchMock.mockResolvedValueOnce(
      jsonResponse({ code: 404, message: 'capability is not available', data: null }, 404)
    )
    renderPage()
    await screen.findByText('Zoom')

    const zoomCard = screen.getByText('Zoom').closest('[data-ui="capabilities.card"]')!
    fireEvent.click(zoomCard.querySelector('button')!)

    await waitFor(() => expect(toastError).toHaveBeenCalled())
    expect(zoomCard.querySelector('button')).toHaveAttribute('aria-label', 'capabilities.install')
  })

  it('names the team rule instead of the server text when a team refuses an install', async () => {
    fetchMock.mockResolvedValueOnce(
      jsonResponse(
        { success: false, code: 40300, message: 'a team you belong to does not allow this capability: lab' },
        403
      )
    )
    renderPage()
    await screen.findByText('Zoom')

    const zoomCard = screen.getByText('Zoom').closest('[data-ui="capabilities.card"]')!
    fireEvent.click(zoomCard.querySelector('button')!)

    await waitFor(() => expect(toastError).toHaveBeenCalled())
    const [message] = toastError.mock.calls[0]
    expect(String(message)).toContain('capabilities.install_team_refused')
    expect(String(message)).not.toContain('does not allow')
    expect(zoomCard.querySelector('button')).toHaveAttribute('aria-label', 'capabilities.install')
  })

  it('does not re-read a failed apply into the installed state', async () => {
    fetchMock.mockResolvedValueOnce(
      jsonResponse(
        {
          code: 502,
          message: 'saved to your account, but this device could not apply it: zoom',
          data: null
        },
        502
      )
    )
    renderPage()
    await screen.findByText('Zoom')
    api.listCapabilities.mockImplementation((query: CapabilityQuery) =>
      Promise.resolve({
        ...pageFor(query),
        items: pageFor(query).items.map((row) => (row.slug === 'zoom' ? { ...row, installed: true } : row))
      })
    )

    const zoomCard = screen.getByText('Zoom').closest('[data-ui="capabilities.card"]')!
    fireEvent.click(zoomCard.querySelector('button')!)

    await waitFor(() => expect(toastError).toHaveBeenCalledWith(expect.stringContaining('could not apply it')))
    expect(zoomCard.querySelector('button')).toHaveAttribute('aria-label', 'capabilities.install')
    expect(zoomCard.querySelector('button')).toHaveAttribute('aria-pressed', 'false')
  })

  it('loads the next page from the OS', async () => {
    api.listCapabilities.mockImplementation((query: CapabilityQuery) =>
      Promise.resolve({
        items:
          query.page === 2
            ? [entry('page-two', { name: 'Page Two' })]
            : [entry('ifind', { name: '同花顺iFinD', featured: true }), entry('zoom', { name: 'Zoom' })],
        total: 30,
        page: query.page ?? 1,
        pageSize: 24
      })
    )
    renderPage()
    expect(await screen.findByText('Zoom')).toBeInTheDocument()

    fireEvent.click(screen.getByRole('button', { name: 'capabilities.next' }))

    expect(await screen.findByText('Page Two')).toBeInTheDocument()
    expect(api.listCapabilities).toHaveBeenLastCalledWith(
      expect.objectContaining({ kind: 'connector', page: 2, pageSize: 24 })
    )
  })

  it('shows a loading catalog, then an empty result', async () => {
    clearPageEntrySession()
    let release: (page: ReturnType<typeof pageFor>) => void = () => undefined
    api.listCapabilities.mockImplementation(
      () =>
        new Promise((resolve) => {
          release = resolve
        })
    )
    renderPage()

    expect(await screen.findByText('common.loading')).toBeInTheDocument()
    expect(document.querySelector('.page-loader--contain')).not.toBeNull()
    expect(screen.queryByText('Zoom')).not.toBeInTheDocument()
    release({ items: [], total: 0, page: 1, pageSize: 24, categories: [] })
    const empty = await screen.findByText('capabilities.empty.connectors')
    const region = empty.closest('[data-ui="capabilities.empty"]')
    expect(region).toHaveAttribute('role', 'status')
    expect(region?.querySelector('svg')).toBeTruthy()
    expect(screen.getByText('capabilities.empty.connectors_hint')).toBeInTheDocument()
    fireEvent.click(within(region as HTMLElement).getByRole('button', { name: 'capabilities.create' }))
    expect(await screen.findByRole('heading', { name: 'capabilities.connector.custom.title' })).toBeInTheDocument()
  })

  it('retries a failed catalog load', async () => {
    api.listCapabilities.mockRejectedValueOnce(new MemberOrgError('catalog down', 500))
    renderPage('connector', { shouldRetryOnError: false })

    expect(await screen.findByText('capabilities.load_failed')).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'common.retry' }))
    expect(await screen.findByText('Zoom')).toBeInTheDocument()
    expect(api.listCapabilities.mock.calls.length).toBeGreaterThan(1)
  })

  it('asks a signed-out member to sign in', async () => {
    api.listCapabilities.mockRejectedValue(new MemberOrgError('HTTP 401', 401))
    renderPage()

    expect(await screen.findByText('capabilities.sign_in_required')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'common.retry' })).toBeInTheDocument()
  })

  it('opens a custom connector dialog and does not save when it is dismissed', async () => {
    const user = userEvent.setup()
    renderPage()
    expect(await screen.findByText('Zoom')).toBeInTheDocument()

    fireEvent.click(screen.getByRole('button', { name: 'capabilities.create' }))
    const dialog = await screen.findByRole('dialog')
    expect(within(dialog).getByRole('heading', { name: 'capabilities.connector.custom.title' })).toHaveClass(
      'text-[15px]',
      'font-medium',
      'text-[#171717]'
    )
    expect(dialog).toHaveClass('rounded-[12px]', 'bg-white', 'shadow-[0_12px_32px_rgba(0,0,0,0.12)]')
    expect(dialog.className).not.toContain('bg-card')
    expect(document.querySelector('[data-slot="dialog-overlay"]')).toHaveClass('bg-black/20')
    const name = within(dialog).getByRole('textbox', { name: 'capabilities.connector.custom.name' })
    expect(name).toHaveClass('h-9', 'rounded-[8px]', 'bg-[#f5f5f5]', 'text-[13px]', 'placeholder:text-[#8a8a8a]')
    expect(name).toHaveAttribute('placeholder', 'capabilities.connector.custom.name_placeholder')
    expect(name.closest('label')?.querySelector('span')).toHaveClass('text-[12px]', 'text-[#8a8a8a]')
    expect(within(dialog).queryByText('capabilities.connector.custom.invalid_name')).not.toBeInTheDocument()
    const command = within(dialog).getByRole('textbox', { name: 'capabilities.connector.custom.command' })
    expect(command).toHaveAttribute('placeholder', 'capabilities.connector.custom.command_placeholder')
    expect(within(dialog).getByRole('textbox', { name: 'capabilities.connector.custom.args' })).toHaveAttribute(
      'placeholder',
      'capabilities.connector.custom.args_placeholder'
    )
    const formTab = within(dialog).getByRole('tab', { name: 'capabilities.connector.custom.form' })
    expect(formTab).toHaveAttribute('data-state', 'active')
    expect(formTab.querySelector('svg')).toHaveAttribute('stroke-width', '1.7')
    expect(formTab.querySelector('svg')).toHaveClass('size-4')
    expect(
      within(dialog).queryByRole('textbox', { name: 'capabilities.connector.custom.json' })
    ).not.toBeInTheDocument()
    const localCommand = within(dialog).getByRole('radio', { name: 'capabilities.connection.stdio' })
    expect(localCommand).toHaveClass('h-7', 'font-medium', 'shadow-[0_1px_2px_rgba(0,0,0,0.06)]')
    expect(localCommand.querySelector('svg')).toHaveAttribute('stroke-width', '1.7')
    expect(localCommand.querySelector('svg')).toHaveClass('size-4')
    expect(within(dialog).getByRole('radio', { name: 'capabilities.connection.sse' })).toHaveClass('text-[#6f6f6f]')
    await user.click(within(dialog).getByRole('button', { name: 'capabilities.connector.custom.add_field' }))
    expect(within(dialog).getByLabelText('capabilities.connector.custom.key')).toHaveAttribute(
      'placeholder',
      'capabilities.connector.custom.key_placeholder'
    )
    expect(within(dialog).getByLabelText('capabilities.connector.custom.value')).toHaveAttribute(
      'placeholder',
      'capabilities.connector.custom.value_placeholder'
    )
    const save = within(dialog).getByRole('button', { name: 'capabilities.connector.custom.save' })
    expect(save).toBeDisabled()
    expect(save).toHaveClass('h-9', 'bg-[#171717]', 'disabled:opacity-40')
    expect(
      within(dialog)
        .getAllByRole('button')
        .filter((button) => button.className.includes('bg-[#171717]'))
    ).toHaveLength(1)
    const addField = within(dialog).getByRole('button', { name: 'capabilities.connector.custom.add_field' })
    expect(addField.querySelector('svg')).toHaveAttribute('stroke-width', '1.7')
    expect(addField.querySelector('svg')).toHaveClass('size-4')
    expect(within(dialog).getByRole('button', { name: 'common.close' }).querySelector('svg')).toHaveAttribute(
      'stroke-width',
      '1.7'
    )
    expect(dialog.innerHTML).not.toContain('#b42318')
    expect(navigate).not.toHaveBeenCalled()

    await user.keyboard('{Escape}')
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument())
    expect(api.saveCustomConnector).not.toHaveBeenCalled()

    fireEvent.click(screen.getByRole('button', { name: 'capabilities.create' }))
    const again = await screen.findByRole('dialog')
    fireEvent.click(document.querySelector('[data-slot="dialog-overlay"]')!)
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument())
    expect(again).not.toBeInTheDocument()
    expect(api.saveCustomConnector).not.toHaveBeenCalled()
    expect(api.removeCustomConnector).not.toHaveBeenCalled()
  })

  it('refuses an empty name, a missing command, a missing url, and an unknown transport', async () => {
    expect(customConnectorBody({ name: '', transport: 'stdio', command: 'npx' })).toBeNull()
    expect(customConnectorIssues({ name: 'Bad Name', transport: 'stdio', command: 'npx' }).name).toBe('invalid_name')
    expect(customConnectorBody({ name: 'local-fs', transport: 'stdio', command: '   ' })).toBeNull()
    expect(customConnectorIssues({ name: 'local-fs', transport: 'stdio', command: '   ' }).command).toBe(
      'missing_command'
    )
    expect(customConnectorBody({ name: 'docs', transport: 'sse', url: '' })).toBeNull()
    expect(customConnectorIssues({ name: 'docs', transport: 'sse', url: '' }).url).toBe('missing_url')
    expect(customConnectorIssues({ name: 'docs', transport: 'sse', url: 'javascript:alert(1)' }).url).toBe(
      'invalid_url'
    )
    expect(customConnectorBody({ name: 'search', transport: 'streamable_http', url: '   ' })).toBeNull()
    expect(customConnectorBody({ name: 'mem', transport: 'in-memory', command: 'node' })).toBeNull()
    expect(customConnectorIssues({ name: 'mem', transport: 'in-memory', command: 'node' }).transport).toBe(
      'unknown_transport'
    )
    expect(
      customConnectorIssues({
        name: 'local-fs',
        transport: 'stdio',
        command: 'npx',
        values: [{ key: '1bad', value: 'x' }]
      }).fields
    ).toEqual(['invalid_config'])
    expect(customConnectorIssues({ name: 'local-fs', transport: 'stdio', command: 'npx' }).name).toBeNull()

    const user = userEvent.setup()
    renderPage()
    fireEvent.click(await screen.findByRole('button', { name: 'capabilities.create' }))
    const dialog = await screen.findByRole('dialog')
    const save = within(dialog).getByRole('button', { name: 'capabilities.connector.custom.save' })
    const name = within(dialog).getByRole('textbox', { name: 'capabilities.connector.custom.name' })
    await user.type(name, 'Bad Name')
    expect(within(dialog).queryByText('capabilities.connector.custom.invalid_name')).not.toBeInTheDocument()
    await user.type(within(dialog).getByRole('textbox', { name: 'capabilities.connector.custom.command' }), 'npx')
    expect(within(dialog).getByText('capabilities.connector.custom.invalid_name')).toBeInTheDocument()
    expect(name).toHaveAttribute('aria-invalid', 'true')
    expect(dialog.innerHTML).not.toContain('#b42318')
    expect(save).toBeDisabled()
    fireEvent.click(save)
    expect(api.saveCustomConnector).not.toHaveBeenCalled()

    await user.clear(name)
    await user.type(name, 'local-fs')
    expect(within(dialog).queryByText('capabilities.connector.custom.invalid_name')).not.toBeInTheDocument()
    await user.clear(within(dialog).getByRole('textbox', { name: 'capabilities.connector.custom.command' }))
    expect(save).toBeDisabled()
    fireEvent.click(save)
    expect(within(dialog).getByText('capabilities.connector.custom.missing_command')).toBeInTheDocument()
    expect(api.saveCustomConnector).not.toHaveBeenCalled()

    await user.click(within(dialog).getByRole('radio', { name: 'capabilities.connection.sse' }))
    expect(within(dialog).getByRole('textbox', { name: 'capabilities.connector.custom.url' })).toHaveAttribute(
      'placeholder',
      'capabilities.connector.custom.sse_placeholder'
    )
    expect(save).toBeDisabled()
    fireEvent.click(save)
    expect(within(dialog).getByText('capabilities.connector.custom.missing_url')).toBeInTheDocument()
    expect(api.saveCustomConnector).not.toHaveBeenCalled()
  })

  it('adds, edits, and removes a stdio, sse, and streamable http connector', async () => {
    const stored: CustomConnectorBody[] = []
    api.listCustomConnectors.mockImplementation(async () =>
      stored.map((row) => ({ ...row, values: { ...row.values }, args: row.args ? [...row.args] : undefined }))
    )
    api.saveCustomConnector.mockImplementation(async (body: CustomConnectorBody) => {
      const copy = { ...body, values: { ...body.values }, args: body.args ? [...body.args] : undefined }
      const index = stored.findIndex((row) => row.name === body.name)
      if (index >= 0) stored[index] = copy
      else stored.push(copy)
    })
    api.removeCustomConnector.mockImplementation(async (name: string) => {
      const index = stored.findIndex((row) => row.name === name)
      if (index >= 0) stored.splice(index, 1)
    })
    const user = userEvent.setup()
    renderPage()
    fireEvent.click(await screen.findByRole('button', { name: 'capabilities.create' }))
    const dialog = await screen.findByRole('dialog')

    const fillSecret = async (key: string, value: string) => {
      await user.click(within(dialog).getByRole('button', { name: 'capabilities.connector.custom.add_field' }))
      const keys = within(dialog).getAllByLabelText('capabilities.connector.custom.key')
      const values = within(dialog).getAllByLabelText('capabilities.connector.custom.value')
      await user.type(keys[keys.length - 1]!, key)
      await user.type(values[values.length - 1]!, value)
    }

    await user.type(within(dialog).getByRole('textbox', { name: 'capabilities.connector.custom.name' }), 'local-fs')
    await user.type(within(dialog).getByRole('textbox', { name: 'capabilities.connector.custom.command' }), 'npx')
    await user.type(within(dialog).getByRole('textbox', { name: 'capabilities.connector.custom.args' }), '-y\npkg')
    await fillSecret('TOKEN', 'tok_test')
    const stdio = customConnectorBody({
      name: 'local-fs',
      transport: 'stdio',
      command: 'npx',
      args: '-y\npkg',
      values: [{ key: 'TOKEN', value: 'tok_test' }]
    })
    expect(stdio).toEqual({
      name: 'local-fs',
      transport: 'stdio',
      command: 'npx',
      args: ['-y', 'pkg'],
      values: { TOKEN: 'tok_test' }
    })
    await user.click(within(dialog).getByRole('button', { name: 'capabilities.connector.custom.save' }))
    await waitFor(() => expect(api.saveCustomConnector).toHaveBeenCalledWith(stdio))
    expect(within(dialog).getByRole('textbox', { name: 'capabilities.connector.custom.name' })).toBeDisabled()

    await user.clear(within(dialog).getByRole('textbox', { name: 'capabilities.connector.custom.command' }))
    await user.type(within(dialog).getByRole('textbox', { name: 'capabilities.connector.custom.command' }), 'node')
    const edited = customConnectorBody({
      name: 'local-fs',
      transport: 'stdio',
      command: 'node',
      args: '-y\npkg',
      values: [{ key: 'TOKEN', value: 'tok_test' }]
    })
    await user.click(within(dialog).getByRole('button', { name: 'capabilities.connector.custom.save' }))
    await waitFor(() => expect(api.saveCustomConnector).toHaveBeenLastCalledWith(edited))
    expect(dialog.innerHTML).not.toContain('#b42318')
    await user.click(within(dialog).getByRole('button', { name: 'capabilities.connector.custom.remove' }))
    expect(within(dialog).getByRole('button', { name: 'capabilities.connector.custom.remove_confirm' })).toHaveClass(
      'text-[#b42318]'
    )
    await user.click(within(dialog).getByRole('button', { name: 'capabilities.connector.custom.remove_confirm' }))
    await waitFor(() => expect(api.removeCustomConnector).toHaveBeenCalledWith('local-fs'))
    await waitFor(() => expect(within(dialog).queryByRole('button', { name: 'local-fs' })).not.toBeInTheDocument())

    await user.click(within(dialog).getByRole('radio', { name: 'capabilities.connection.sse' }))
    await user.type(within(dialog).getByRole('textbox', { name: 'capabilities.connector.custom.name' }), 'docs')
    await user.type(
      within(dialog).getByRole('textbox', { name: 'capabilities.connector.custom.url' }),
      'https://docs.example/sse'
    )
    await fillSecret('Authorization', 'Bearer tok_test')
    const sse = customConnectorBody({
      name: 'docs',
      transport: 'sse',
      url: 'https://docs.example/sse',
      values: [{ key: 'Authorization', value: 'Bearer tok_test' }]
    })
    await user.click(within(dialog).getByRole('button', { name: 'capabilities.connector.custom.save' }))
    await waitFor(() => expect(api.saveCustomConnector).toHaveBeenLastCalledWith(sse))
    await user.clear(within(dialog).getByRole('textbox', { name: 'capabilities.connector.custom.url' }))
    await user.type(
      within(dialog).getByRole('textbox', { name: 'capabilities.connector.custom.url' }),
      'https://docs.example/v2'
    )
    const sseEdited = customConnectorBody({
      name: 'docs',
      transport: 'sse',
      url: 'https://docs.example/v2',
      values: [{ key: 'Authorization', value: 'Bearer tok_test' }]
    })
    await user.click(within(dialog).getByRole('button', { name: 'capabilities.connector.custom.save' }))
    await waitFor(() => expect(api.saveCustomConnector).toHaveBeenLastCalledWith(sseEdited))
    await user.click(within(dialog).getByRole('button', { name: 'capabilities.connector.custom.remove' }))
    await user.click(within(dialog).getByRole('button', { name: 'capabilities.connector.custom.remove_confirm' }))
    await waitFor(() => expect(api.removeCustomConnector).toHaveBeenCalledWith('docs'))

    await user.click(within(dialog).getByRole('radio', { name: 'capabilities.connection.http' }))
    await user.type(within(dialog).getByRole('textbox', { name: 'capabilities.connector.custom.name' }), 'search')
    await user.type(
      within(dialog).getByRole('textbox', { name: 'capabilities.connector.custom.url' }),
      'https://search.example/mcp'
    )
    await fillSecret('X_API_KEY', 'tok_test')
    const http = customConnectorBody({
      name: 'search',
      transport: 'streamable_http',
      url: 'https://search.example/mcp',
      values: [{ key: 'X_API_KEY', value: 'tok_test' }]
    })
    expect(http).toMatchObject({ transport: 'streamable_http', url: 'https://search.example/mcp' })
    await user.click(within(dialog).getByRole('button', { name: 'capabilities.connector.custom.save' }))
    await waitFor(() => expect(api.saveCustomConnector).toHaveBeenLastCalledWith(http))
    await user.clear(within(dialog).getByRole('textbox', { name: 'capabilities.connector.custom.url' }))
    await user.type(
      within(dialog).getByRole('textbox', { name: 'capabilities.connector.custom.url' }),
      'https://search.example/v2'
    )
    const httpEdited = customConnectorBody({
      name: 'search',
      transport: 'streamable_http',
      url: 'https://search.example/v2',
      values: [{ key: 'X_API_KEY', value: 'tok_test' }]
    })
    await user.click(within(dialog).getByRole('button', { name: 'capabilities.connector.custom.save' }))
    await waitFor(() => expect(api.saveCustomConnector).toHaveBeenLastCalledWith(httpEdited))
    await user.click(within(dialog).getByRole('button', { name: 'capabilities.connector.custom.remove' }))
    await user.click(within(dialog).getByRole('button', { name: 'capabilities.connector.custom.remove_confirm' }))
    await waitFor(() => expect(api.removeCustomConnector).toHaveBeenCalledWith('search'))
    expect(navigate).not.toHaveBeenCalled()
  })

  it('fills one pasted connector into the form and saves it with the existing button', async () => {
    const stored: CustomConnectorBody[] = []
    api.listCustomConnectors.mockImplementation(async () =>
      stored.map((row) => ({ ...row, values: { ...row.values } }))
    )
    api.saveCustomConnector.mockImplementation(async (body: CustomConnectorBody) => {
      stored.push(body)
    })
    const user = userEvent.setup()
    renderPage()
    fireEvent.click(await screen.findByRole('button', { name: 'capabilities.create' }))
    const dialog = await screen.findByRole('dialog')
    await user.type(within(dialog).getByRole('textbox', { name: 'capabilities.connector.custom.name' }), 'local-fs')
    await user.type(within(dialog).getByRole('textbox', { name: 'capabilities.connector.custom.command' }), 'npx')
    await user.click(within(dialog).getByRole('tab', { name: 'capabilities.connector.custom.json' }))
    const json = within(dialog).getByRole('textbox', { name: 'capabilities.connector.custom.json' })
    const seeded = customConnectorBody({ name: 'local-fs', transport: 'stdio', command: 'npx' })
    expect(json).toHaveValue(formatConnectorPaste(seeded!))
    expect(json).toHaveClass('rounded-[8px]', 'bg-[#f5f5f5]', 'font-mono', 'text-[13px]')
    expect(
      within(dialog).queryByRole('textbox', { name: 'capabilities.connector.custom.name' })
    ).not.toBeInTheDocument()
    expect(
      within(dialog)
        .getAllByRole('button')
        .filter((button) => button.className.includes('bg-[#171717]'))
    ).toHaveLength(1)
    await user.click(within(dialog).getByRole('tab', { name: 'capabilities.connector.custom.form' }))
    await user.type(within(dialog).getByRole('textbox', { name: 'capabilities.connector.custom.command' }), 'x')
    await user.click(within(dialog).getByRole('tab', { name: 'capabilities.connector.custom.json' }))
    expect(within(dialog).getByRole('textbox', { name: 'capabilities.connector.custom.json' })).toHaveValue(
      formatConnectorPaste(seeded!)
    )

    const snippet = `{
      "mcpServers": {
        "local-fs": { "command": "npx", "args": ["-y", "filesystem"], "env": { "API_KEY": "secret" } }
      }
    }`
    const jsonField = within(dialog).getByRole('textbox', { name: 'capabilities.connector.custom.json' })
    await user.click(jsonField)
    ;(jsonField as HTMLTextAreaElement).setSelectionRange(0, (jsonField as HTMLTextAreaElement).value.length)
    await user.paste(snippet)
    expect(jsonField).toHaveValue(snippet)
    await user.click(within(dialog).getByRole('tab', { name: 'capabilities.connector.custom.form' }))
    const name = within(dialog).getByRole('textbox', { name: 'capabilities.connector.custom.name' })
    expect(name).toHaveValue('local-fs')
    expect(name).toBeEnabled()
    expect(within(dialog).getByRole('textbox', { name: 'capabilities.connector.custom.command' })).toHaveValue('npx')
    expect(within(dialog).getByRole('textbox', { name: 'capabilities.connector.custom.args' })).toHaveValue(
      '-y\nfilesystem'
    )
    expect(within(dialog).getByLabelText('capabilities.connector.custom.key')).toHaveValue('API_KEY')
    expect(
      within(dialog).queryByRole('button', { name: 'capabilities.connector.custom.remove' })
    ).not.toBeInTheDocument()
    expect(api.saveCustomConnector).not.toHaveBeenCalled()
    const save = within(dialog).getByRole('button', { name: 'capabilities.connector.custom.save' })
    expect(save).toBeEnabled()
    await user.click(save)
    const saved = customConnectorBody({
      name: 'local-fs',
      transport: 'stdio',
      command: 'npx',
      args: '-y\nfilesystem',
      values: [{ key: 'API_KEY', value: 'secret' }]
    })
    await waitFor(() => expect(api.saveCustomConnector).toHaveBeenCalledWith(saved))
    expect(name).toBeDisabled()

    await user.click(within(dialog).getByRole('tab', { name: 'capabilities.connector.custom.json' }))
    const edited = within(dialog).getByRole('textbox', { name: 'capabilities.connector.custom.json' })
    await user.click(edited)
    ;(edited as HTMLTextAreaElement).setSelectionRange(0, (edited as HTMLTextAreaElement).value.length)
    await user.paste('{ "command": "node" }')
    expect(api.saveCustomConnector).toHaveBeenCalledTimes(1)
    await user.click(within(dialog).getByRole('tab', { name: 'capabilities.connector.custom.form' }))
    const reviewed = within(dialog).getByRole('textbox', { name: 'capabilities.connector.custom.name' })
    expect(reviewed).toHaveValue('local-fs')
    expect(reviewed).toBeDisabled()
    expect(within(dialog).getByRole('textbox', { name: 'capabilities.connector.custom.command' })).toHaveValue('node')
  })

  it('imports several pasted connectors only when apply is clicked', async () => {
    const stored: CustomConnectorBody[] = []
    api.listCustomConnectors.mockImplementation(async () =>
      stored.map((row) => ({ ...row, values: { ...row.values } }))
    )
    api.saveCustomConnector.mockImplementation(async (body: CustomConnectorBody) => {
      const index = stored.findIndex((row) => row.name === body.name)
      if (index >= 0) stored[index] = body
      else stored.push(body)
    })
    const user = userEvent.setup()
    renderPage()
    fireEvent.click(await screen.findByRole('button', { name: 'capabilities.create' }))
    const dialog = await screen.findByRole('dialog')
    await user.click(within(dialog).getByRole('tab', { name: 'capabilities.connector.custom.json' }))
    const json = within(dialog).getByRole('textbox', { name: 'capabilities.connector.custom.json' })
    const many = `{
      "mcpServers": {
        "alpha": { "command": "npx", "args": ["-y", "alpha"] },
        "beta": { "type": "http", "url": "https://example.com/mcp", "headers": { "Authorization": "Bearer x" } }
      }
    }`
    await user.click(json)
    await user.paste(many)
    await user.click(within(dialog).getByRole('tab', { name: 'capabilities.connector.custom.form' }))
    expect(within(dialog).getByRole('textbox', { name: 'capabilities.connector.custom.name' })).toHaveValue('')
    await user.click(within(dialog).getByRole('tab', { name: 'capabilities.connector.custom.json' }))
    expect(api.saveCustomConnector).not.toHaveBeenCalled()
    const apply = within(dialog).getByRole('button', { name: 'capabilities.connector.custom.json_import' })
    expect(apply.className).not.toContain('bg-[#171717]')
    await user.click(apply)
    const alpha = customConnectorBody({ name: 'alpha', transport: 'stdio', command: 'npx', args: '-y\nalpha' })
    const beta = customConnectorBody({
      name: 'beta',
      transport: 'streamable_http',
      url: 'https://example.com/mcp',
      values: [{ key: 'Authorization', value: 'Bearer x' }]
    })
    await waitFor(() => expect(api.saveCustomConnector).toHaveBeenNthCalledWith(1, alpha))
    expect(api.saveCustomConnector).toHaveBeenNthCalledWith(2, beta)
    expect(toastSuccess).toHaveBeenCalledWith('capabilities.connector.custom.json_imported')
    const name = within(dialog).getByRole('textbox', { name: 'capabilities.connector.custom.name' })
    expect(name).toHaveValue('alpha')
    expect(name).toBeDisabled()
  })

  it('leaves storage unchanged when pasted JSON is refused, and stops a batch on the first server error', async () => {
    const stored: CustomConnectorBody[] = []
    api.listCustomConnectors.mockImplementation(async () =>
      stored.map((row) => ({ ...row, values: { ...row.values } }))
    )
    api.saveCustomConnector.mockImplementation(async (body: CustomConnectorBody) => {
      if (body.name === 'beta') throw new MemberOrgError('connector_exists', 409)
      stored.push(body)
    })
    const user = userEvent.setup()
    renderPage()
    fireEvent.click(await screen.findByRole('button', { name: 'capabilities.create' }))
    const dialog = await screen.findByRole('dialog')
    await user.click(within(dialog).getByRole('tab', { name: 'capabilities.connector.custom.json' }))
    const json = within(dialog).getByRole('textbox', { name: 'capabilities.connector.custom.json' })
    await user.click(json)
    await user.paste('{')
    expect(json).toHaveValue('{')
    expect(within(dialog).getByRole('alert')).toHaveTextContent('capabilities.connector.custom.invalid_json')
    expect(dialog.innerHTML).not.toContain('#b42318')
    await user.click(within(dialog).getByRole('button', { name: 'capabilities.connector.custom.json_apply' }))
    expect(api.saveCustomConnector).not.toHaveBeenCalled()

    await user.clear(json)
    await user.paste('{ "mcpServers": { "alpha": { "command": "npx" }, "My Server": { "command": "node" } } }')
    expect(within(dialog).getByRole('alert')).toHaveTextContent('capabilities.connector.custom.invalid_name')
    expect(api.saveCustomConnector).not.toHaveBeenCalled()

    await user.clear(json)
    await user.paste(`{
      "mcpServers": {
        "alpha": { "command": "npx" },
        "beta": { "command": "node" }
      }
    }`)
    await user.click(within(dialog).getByRole('button', { name: 'capabilities.connector.custom.json_import' }))
    await waitFor(() =>
      expect(within(dialog).getByText('capabilities.connector.custom.connector_exists')).toBeInTheDocument()
    )
    expect(api.saveCustomConnector).toHaveBeenCalledTimes(2)
    expect(toastSuccess).not.toHaveBeenCalled()
    expect(within(dialog).getByRole('button', { name: 'alpha' })).toBeInTheDocument()
    expect(within(dialog).queryByRole('button', { name: 'beta' })).not.toBeInTheDocument()
  })

  it('opens a connector card into a detail dialog and installs into the config form', async () => {
    renderPage()
    fireEvent.click(await screen.findByText('Zoom'))

    const dialog = await screen.findByRole('dialog')
    expect(within(dialog).getByRole('heading', { name: 'Zoom' })).toBeInTheDocument()
    expect(within(dialog).getByText('zoom description')).toBeInTheDocument()
    fireEvent.click(within(dialog).getByRole('button', { name: 'capabilities.install' }))

    await waitFor(() =>
      expect(fetchMock).toHaveBeenCalledWith(
        'http://127.0.0.1:4000/api/capabilities/installations/zoom',
        expect.objectContaining({ method: 'PUT' })
      )
    )
    expect(within(dialog).getByText('capabilities.config.empty')).toBeInTheDocument()
    expect(within(dialog).getByRole('button', { name: 'capabilities.uninstall' })).toBeInTheDocument()
    expect(within(dialog).queryByRole('button', { name: 'common.save' })).not.toBeInTheDocument()
  })

  it('configures an installed connector from its field list', async () => {
    api.listCapabilities.mockImplementation((query: CapabilityQuery) =>
      Promise.resolve(
        pageFor(query, [
          entry('github', {
            name: 'GitHub',
            installed: true,
            description: 'Repos',
            install: { transport: 'stdio', command: 'npx', args: ['-y', 'server-github'] },
            config: [
              {
                key: 'GITHUB_PERSONAL_ACCESS_TOKEN',
                description: 'PAT',
                required: true,
                secret: true
              }
            ]
          })
        ])
      )
    )
    renderPage()
    const card = (await screen.findByText('GitHub')).closest('[data-ui="capabilities.card"]')
    expect(within(card as HTMLElement).queryByRole('button', { name: 'plugins.okf.configure' })).toBeNull()
    fireEvent.click(card as HTMLElement)

    const dialog = await screen.findByRole('dialog')
    const token = within(dialog).getByLabelText(/GITHUB_PERSONAL_ACCESS_TOKEN/)
    expect(token).toHaveAttribute('type', 'password')
    expect(within(dialog).getByText('npx -y server-github')).toBeInTheDocument()
    expect(within(dialog).getByRole('button', { name: 'common.save' })).toBeDisabled()

    fireEvent.change(token, { target: { value: 'ghp_test' } })
    fireEvent.click(within(dialog).getByRole('button', { name: 'common.save' }))
    await waitFor(() => expect(toastSuccess).toHaveBeenCalledWith('common.saved'))
    expect(within(dialog).getByRole('button', { name: 'common.save' })).toBeDisabled()

    fireEvent.click(within(dialog).getByRole('button', { name: 'common.close' }))
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    fireEvent.click(screen.getByText('GitHub'))
    const saved = await screen.findByLabelText(/GITHUB_PERSONAL_ACCESS_TOKEN/)
    expect(saved).toHaveValue('ghp_test')
    expect(saved).toHaveAttribute('type', 'password')
    const catalogCalls = JSON.stringify([...api.listCapabilities.mock.calls, ...api.uninstallCapability.mock.calls])
    expect(catalogCalls).not.toContain('ghp_test')
    expect(localStorage.getItem('a3s.connector.config.v1')).toContain('ghp_test')
  })

  it('reports a failed device save instead of a silent success', async () => {
    api.listCapabilities.mockImplementation((query: CapabilityQuery) =>
      Promise.resolve(
        pageFor(query, [
          entry('github', {
            name: 'GitHub',
            installed: true,
            description: 'Repos',
            install: { transport: 'stdio', command: 'npx', args: ['-y', 'server-github'] },
            config: [
              {
                key: 'GITHUB_PERSONAL_ACCESS_TOKEN',
                description: 'PAT',
                required: true,
                secret: true
              }
            ]
          })
        ])
      )
    )
    fetchMock.mockImplementation((input: RequestInfo | URL, init?: RequestInit) => {
      const url = String(input)
      if (url.includes('/api/capabilities/config/') && init?.method === 'PUT')
        return jsonResponse({ error: 'config_failed' }, 500)
      if (url.endsWith('/api/capabilities/config')) return jsonResponse({})
      return jsonResponse({ code: 0, data: { installed: true } })
    })
    renderPage()
    fireEvent.click(await screen.findByText('GitHub'))

    const dialog = await screen.findByRole('dialog')
    const token = within(dialog).getByLabelText(/GITHUB_PERSONAL_ACCESS_TOKEN/)
    fireEvent.change(token, { target: { value: 'ghp_test' } })
    fireEvent.click(within(dialog).getByRole('button', { name: 'common.save' }))

    await waitFor(() => expect(toastError).toHaveBeenCalledWith('capabilities.config.save_failed'))
    expect(toastSuccess).not.toHaveBeenCalled()
    // The form keeps the typed value so the member can retry.
    expect(token).toHaveValue('ghp_test')
    expect(localStorage.getItem('a3s.connector.config.v1')).toContain('ghp_test')
  })

  it('hides rows that are not featured while that filter is loading', async () => {
    renderPage()
    expect(await screen.findByText('Zoom')).toBeInTheDocument()
    let release: (page: ReturnType<typeof pageFor>) => void = () => undefined
    api.listCapabilities.mockImplementation((query: CapabilityQuery) => {
      if (query.featured)
        return new Promise((resolve) => {
          release = resolve
        })
      return Promise.resolve(pageFor(query))
    })

    fireEvent.click(screen.getByRole('button', { name: 'capabilities.category.featured' }))

    expect(screen.getByRole('heading', { level: 2 })).toHaveTextContent('capabilities.category.featured')
    expect(screen.queryByText('Zoom')).not.toBeInTheDocument()
    expect(screen.getByText('同花顺iFinD')).toBeInTheDocument()
    expect(screen.queryByText('capabilities.empty.featured')).not.toBeInTheDocument()
    await act(async () => {
      release(pageFor({ kind: 'connector', featured: true, page: 1, pageSize: 24 }))
    })
    await waitFor(() =>
      expect(api.listCapabilities).toHaveBeenCalledWith(
        expect.objectContaining({ kind: 'connector', featured: true, page: 1 })
      )
    )
    expect(screen.queryByText('Zoom')).not.toBeInTheDocument()
    expect(screen.getByText('同花顺iFinD')).toBeInTheDocument()
  })

  it('shows the featured empty state when the skill catalog has no picks', async () => {
    api.listCapabilities.mockImplementation((query: CapabilityQuery) =>
      Promise.resolve(pageFor(query, [entry('dev-expert', { kind: 'skill', name: 'dev-expert', featured: false })]))
    )
    renderPage('skill')
    expect(await screen.findByText('dev-expert')).toBeInTheDocument()

    fireEvent.click(screen.getByRole('button', { name: 'capabilities.category.featured' }))

    expect(await screen.findByText('capabilities.empty.featured')).toBeInTheDocument()
    expect(screen.getByText('capabilities.empty.featured_hint')).toBeInTheDocument()
    expect(screen.queryByText('dev-expert')).not.toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'capabilities.empty.show_all' }))
    expect(await screen.findByText('dev-expert')).toBeInTheDocument()
    expect(api.listCapabilities).toHaveBeenCalledWith(
      expect.objectContaining({ kind: 'skill', featured: true, page: 1 })
    )
  })

  it('names an empty category, an empty install list, and an empty skill catalog', async () => {
    api.listCapabilities.mockImplementation((query: CapabilityQuery) => {
      if (query.kind === 'skill') return Promise.resolve({ items: [], total: 0, page: 1, pageSize: 24, categories: [] })
      if (query.installed) {
        return Promise.resolve({
          items: [],
          total: 0,
          page: 1,
          pageSize: 24,
          categories: [{ id: 'office', count: 1 }]
        })
      }
      if (query.category === 'office') {
        return Promise.resolve({
          items: [],
          total: 0,
          page: 1,
          pageSize: 24,
          categories: [{ id: 'office', count: 1 }]
        })
      }
      return Promise.resolve(pageFor(query))
    })

    const connectors = renderPage()
    expect(await screen.findByText('Zoom')).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'capabilities.category.office' }))
    expect(await screen.findByText('capabilities.empty.category')).toBeInTheDocument()
    expect(screen.getByText('capabilities.empty.category_hint')).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'capabilities.empty.show_all' }))
    expect(await screen.findByText('Zoom')).toBeInTheDocument()
    fireEvent.click(screen.getByRole('radio', { name: 'plugins.tab.installed' }))
    expect(await screen.findByText('capabilities.empty.installed')).toBeInTheDocument()
    expect(screen.getByText('capabilities.empty.installed_hint')).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'capabilities.empty.browse' }))
    expect(await screen.findByText('Zoom')).toBeInTheDocument()
    connectors.unmount()

    renderPage('skill')
    const skillsEmpty = await screen.findByText('capabilities.empty.skills')
    expect(screen.getByText('capabilities.empty.skills_hint')).toBeInTheDocument()
    fireEvent.click(
      within(skillsEmpty.closest('[data-ui="capabilities.empty"]') as HTMLElement).getByRole('button', {
        name: 'capabilities.create'
      })
    )
    expect(await screen.findByRole('menuitem', { name: 'capabilities.skill.menu.create' })).toBeInTheDocument()
    expect(navigate).not.toHaveBeenCalled()
  })

  it('fills the home composer with the built-in create skill', async () => {
    const onUseSkill = vi.fn()
    renderPage('skill', {}, onUseSkill)
    expect(await screen.findByText('Deep Research')).toBeInTheDocument()
    const user = userEvent.setup()

    await user.click(screen.getByRole('button', { name: 'capabilities.create' }))
    await user.click(await screen.findByRole('menuitem', { name: 'capabilities.skill.menu.create' }))
    await waitFor(() => expect(api.prepareBuiltinSkill).toHaveBeenCalledWith('skill-creator'))
    expect(onUseSkill).toHaveBeenCalledWith({ name: 'skill-creator', prompt: '' })
  })

  it('uploads a skill package from the create menu and refuses the wrong file type', async () => {
    api.uploadSkillPackage.mockResolvedValue({ name: 'demo-skill' })
    renderPage('skill')
    expect(await screen.findByText('Deep Research')).toBeInTheDocument()
    const user = userEvent.setup()
    await user.click(screen.getByRole('button', { name: 'capabilities.create' }))
    await user.click(await screen.findByRole('menuitem', { name: 'capabilities.skill.menu.upload' }))

    const dialog = await screen.findByRole('dialog')
    await waitFor(() => expect(screen.queryByRole('menu')).not.toBeInTheDocument())
    expect(within(dialog).getByRole('heading', { name: 'capabilities.skill.upload.title' })).toBeInTheDocument()
    expect(within(dialog).getByText('capabilities.skill.upload.limits')).toBeInTheDocument()
    const install = within(dialog).getByRole('button', { name: 'capabilities.skill.upload.install' })
    expect(install).toBeDisabled()

    const input = dialog.querySelector('input[type="file"]') as HTMLInputElement
    fireEvent.change(input, { target: { files: [new File(['hello'], 'notes.txt', { type: 'text/plain' })] } })
    expect(within(dialog).getByRole('alert')).toHaveTextContent('capabilities.skill.upload.bad_type')
    expect(api.uploadSkillPackage).not.toHaveBeenCalled()

    fireEvent.change(input, {
      target: { files: [new File([new Uint8Array([0x50, 0x4b])], 'demo.skill', { type: 'application/zip' })] }
    })
    expect(within(dialog).getByText('demo.skill')).toBeInTheDocument()
    fireEvent.click(within(dialog).getByRole('button', { name: 'capabilities.skill.upload.install' }))
    await waitFor(() => expect(api.uploadSkillPackage).toHaveBeenCalledTimes(1))
    expect(api.uploadSkillPackage.mock.calls[0]?.[0]).toMatchObject({ name: 'demo.skill' })
    await waitFor(() => expect(toastSuccess).toHaveBeenCalledWith('capabilities.skill.upload.success'))
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument())
  })

  async function settleSearchTimer() {
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 350))
    })
  }

  it('debounces typing into one request for the final query', async () => {
    renderPage()
    expect(await screen.findByText('Zoom')).toBeInTheDocument()
    await settleSearchTimer()
    vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] })
    const before = api.listCapabilities.mock.calls.length
    const box = screen.getByRole('searchbox')

    fireEvent.change(box, { target: { value: 'z' } })
    fireEvent.change(box, { target: { value: 'zo' } })
    await act(async () => {
      await vi.advanceTimersByTimeAsync(299)
    })
    expect(api.listCapabilities.mock.calls.length).toBe(before)

    await act(async () => {
      await vi.advanceTimersByTimeAsync(1)
    })
    expect(api.listCapabilities.mock.calls.length).toBe(before + 1)
    expect(api.listCapabilities).toHaveBeenLastCalledWith(
      expect.objectContaining({ kind: 'connector', q: 'zo', page: 1 })
    )
  })

  it('waits out IME composition before debouncing the search', async () => {
    renderPage()
    expect(await screen.findByText('Zoom')).toBeInTheDocument()
    await settleSearchTimer()
    vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] })
    const before = api.listCapabilities.mock.calls.length
    const box = screen.getByRole('searchbox')

    fireEvent.compositionStart(box)
    fireEvent.change(box, { target: { value: 'zhong' } })
    fireEvent.keyDown(box, { key: 'Enter', isComposing: true })
    await act(async () => {
      await vi.advanceTimersByTimeAsync(300)
    })
    expect(api.listCapabilities.mock.calls.length).toBe(before)

    fireEvent.compositionEnd(box)
    await act(async () => {
      await vi.advanceTimersByTimeAsync(299)
    })
    expect(api.listCapabilities.mock.calls.length).toBe(before)
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1)
    })
    expect(api.listCapabilities).toHaveBeenLastCalledWith(
      expect.objectContaining({ kind: 'connector', q: 'zhong', page: 1 })
    )
  })

  it('commits Enter and clear immediately', async () => {
    renderPage()
    expect(await screen.findByText('Zoom')).toBeInTheDocument()
    await settleSearchTimer()
    vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] })
    const box = screen.getByRole('searchbox')
    const before = api.listCapabilities.mock.calls.length

    fireEvent.change(box, { target: { value: 'nope' } })
    fireEvent.click(screen.getByRole('button', { name: 'common.clear' }))
    await act(async () => {
      await vi.advanceTimersByTimeAsync(300)
    })
    expect(box).toHaveValue('')
    expect(api.listCapabilities.mock.calls.length).toBe(before)

    fireEvent.change(box, { target: { value: 'zoom' } })
    fireEvent.keyDown(box, { key: 'Enter' })
    await act(async () => {
      await vi.advanceTimersByTimeAsync(0)
    })
    expect(api.listCapabilities.mock.calls.length).toBe(before + 1)
    expect(api.listCapabilities).toHaveBeenLastCalledWith(expect.objectContaining({ q: 'zoom', page: 1 }))

    fireEvent.click(screen.getByRole('button', { name: 'common.clear' }))
    await act(async () => {
      await vi.advanceTimersByTimeAsync(0)
    })
    expect(box).toHaveValue('')
    expect(screen.getByText('Zoom')).toBeInTheDocument()
    expect(screen.getByText('同花顺iFinD')).toBeInTheDocument()
    await act(async () => {
      await vi.advanceTimersByTimeAsync(300)
    })
    expect(api.listCapabilities.mock.calls.filter(([query]) => query.q === 'zoom')).toHaveLength(1)
  })

  it('keeps the empty sentence hidden until a search with no matches settles', async () => {
    renderPage()
    expect(await screen.findByText('Zoom')).toBeInTheDocument()
    await settleSearchTimer()
    let release: (page: ReturnType<typeof pageFor>) => void = () => undefined
    api.listCapabilities.mockImplementation((query: CapabilityQuery) => {
      if (query.q) {
        return new Promise((resolve) => {
          release = () =>
            resolve({ items: [], total: 0, page: 1, pageSize: 24, categories: [{ id: 'office', count: 1 }] })
        })
      }
      return Promise.resolve(pageFor(query))
    })
    vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] })
    fireEvent.change(screen.getByRole('searchbox'), { target: { value: 'zzzz' } })
    await act(async () => {
      await vi.advanceTimersByTimeAsync(300)
    })

    expect(screen.getByText('common.loading')).toBeInTheDocument()
    expect(screen.queryByText('capabilities.no_results')).not.toBeInTheDocument()
    expect(screen.queryByText('Zoom')).not.toBeInTheDocument()

    vi.useRealTimers()
    await act(async () => {
      release({ items: [], total: 0, page: 1, pageSize: 24, categories: [] })
    })
    const noResults = await screen.findByText('capabilities.no_results')
    expect(screen.getByText('capabilities.no_results_hint')).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'capabilities.empty.clear_search' }))
    expect(await screen.findByText('Zoom')).toBeInTheDocument()
    expect(noResults).not.toBeInTheDocument()
  })
})
