import { render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { SWRConfig } from 'swr'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { preferenceService } from '@data/PreferenceService'
import { publishMemberProfile } from '@renderer/components/account/memberProfile'
import { dataWorkbenchCatalogue } from '@renderer/pages/data/dataPackageCatalogue'
import { publishAixCatalogue } from '@renderer/services/plugins/aixCatalogueStore'

import { pinHomePlugin, readHomePluginPins } from '../../agents/homePins'
import { resetPluginInstallOverlayForTests } from '../../capabilities/pluginInstallSync'
import PluginStorePage from '../PluginStorePage'

const openWindow = vi.hoisted(() => vi.fn(async () => true))
const toastError = vi.hoisted(() => vi.fn())
const toastSuccess = vi.hoisted(() => vi.fn())
const DIGEST = `sha256:${'ab'.repeat(32)}`

vi.mock('react-i18next', () => ({
  initReactI18next: { type: '3rdParty', init: vi.fn() },
  useTranslation: () => ({
    t: (key: string) => key
  })
}))

vi.mock('@renderer/services/toast', () => ({
  toast: { error: toastError, success: toastSuccess, info: vi.fn() }
}))

vi.unmock('@data/hooks/usePreference')

vi.mock('@shared/studioRuntime', () => ({
  readStudioRuntime: () => ({ apiBaseUrl: 'http://127.0.0.1:4000', token: 'shell-token' }),
  readStudioHomePath: () => '/Users/roy'
}))

vi.mock('../../agents/openWorkbenchAppletWindow', () => ({
  openWorkbenchAppletWindow: openWindow
}))

vi.mock('@a3s-lab/office/react', async () => {
  const { createElement } = await import('react')
  return {
    MarkdownEditor: (props: {
      content: { markdown: string }
      presentation?: string
      onSave?: (markdown: string) => void | Promise<void>
    }) =>
      createElement(
        'div',
        { 'data-editor': 'markdown', 'data-presentation': props.presentation, 'data-markdown': props.content.markdown },
        createElement(
          'button',
          { type: 'button', onClick: () => props.onSave?.('---\ntitle: 领域\n---\n甲方付款已修订\n') },
          '保存'
        )
      )
  }
})

type Row = {
  packageType?: 'aix' | 'vsix'
  available?: boolean
  slug: string
  name: string
  description: string
  category: string
  featured: boolean
  builtin?: boolean
  installed: boolean
  surfaces: string[]
  packageId: string
  config?: Array<{ key: string; description: string; required: boolean; secret: boolean }>
}

const catalog = {
  failInstall: false,
  failMaterialize: false,
  markdown: '---\ntitle: 领域\n---\n甲方付款条款\n',
  rows: [] as Row[],
  callableDigest: DIGEST,
  afterInstall: undefined as (() => void) | undefined,
  callable: {} as Record<string, Array<{ kind: string; id: string }>>
}

function row(slug: string, patch: Partial<Row> = {}): Row {
  return {
    slug,
    name: slug,
    description: `${slug} description`,
    category: 'code',
    featured: false,
    installed: false,
    surfaces: [],
    packageId: `use/a3s/${slug}`,
    ...patch
  }
}

function wire(rows: Row[]) {
  catalog.failInstall = false
  catalog.failMaterialize = false
  catalog.rows = rows.map((item) => ({ ...item }))
  catalog.markdown = '---\ntitle: 领域\n---\n甲方付款条款\n'
  catalog.callable = {}
  catalog.callableDigest = DIGEST
  catalog.afterInstall = undefined
}

function payload(rowItem: Row) {
  return {
    slug: rowItem.slug,
    kind: 'plugin',
    category: rowItem.category,
    name: rowItem.name,
    description: rowItem.description,
    color: '#6E5CE6',
    featured: rowItem.featured,
    builtin: rowItem.builtin === true,
    installed: rowItem.installed,
    version: '1.0.0',
    surfaces: rowItem.surfaces,
    install: {
      type: 'plugin',
      packageType: rowItem.packageType,
      available: rowItem.available !== false,
      digest: DIGEST,
      size: 120,
      packageId: rowItem.packageId
    },
    config: rowItem.config ?? []
  }
}

function json(body: unknown, status = 200) {
  return new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json' } })
}

function installFetch() {
  vi.stubGlobal(
    'fetch',
    vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = new URL(typeof input === 'string' ? input : input instanceof URL ? input.href : input.url)
      const method = (init?.method ?? 'GET').toUpperCase()
      if (url.pathname === '/api/capabilities' && method === 'GET') {
        const q = url.searchParams.get('q')?.trim().toLocaleLowerCase() ?? ''
        const category = url.searchParams.get('category')
        const featured = url.searchParams.get('featured') === 'true'
        const installed = url.searchParams.get('installed') === 'true'
        const matched = catalog.rows.filter((item) => {
          if (installed && !item.installed) return false
          if (featured && !item.featured) return false
          if (category && item.category !== category) return false
          if (q && !`${item.name} ${item.description}`.toLocaleLowerCase().includes(q)) return false
          return true
        })
        const counts = new Map<string, number>()
        for (const item of catalog.rows) counts.set(item.category, (counts.get(item.category) ?? 0) + 1)
        return json({
          code: 0,
          data: {
            items: matched.map(payload),
            total: matched.length,
            page: 1,
            pageSize: 100,
            categories: [...counts.entries()].map(([id, count]) => ({ id, count }))
          }
        })
      }
      const installation = url.pathname.match(/^\/api\/capabilities\/installations\/([^/]+)$/)
      if (installation && method === 'PUT') {
        if (catalog.failInstall) throw new Error('disk')
        const slug = decodeURIComponent(installation[1])
        const item = catalog.rows.find((entry) => entry.slug === slug)
        if (item) item.installed = true
        catalog.afterInstall?.()
        return json({ code: 0, data: { slug, installed: true } })
      }
      if (installation && method === 'DELETE') {
        const slug = decodeURIComponent(installation[1])
        const item = catalog.rows.find((entry) => entry.slug === slug)
        if (item) item.installed = false
        return json({ code: 0, data: { slug, installed: false } })
      }
      const callable = url.pathname.match(/^\/api\/capabilities\/([^/]+)\/callable$/)
      if (callable && method === 'GET') {
        const slug = decodeURIComponent(callable[1])
        const surfaces = catalog.failMaterialize ? [] : (catalog.callable[slug] ?? [])
        return json({ code: 0, data: { packageDigest: catalog.callableDigest, surfaces } })
      }
      const okf = url.pathname.match(/^\/api\/capabilities\/([^/]+)\/okf$/)
      if (okf && method === 'GET') {
        return json({ code: 0, data: { pages: [{ path: 'okf/domain/index.md', title: '领域' }] } })
      }
      const page = url.pathname.match(/^\/api\/capabilities\/([^/]+)\/okf\/page$/)
      if (page && method === 'GET') {
        return json({
          code: 0,
          data: { path: url.searchParams.get('path'), title: '领域', markdown: catalog.markdown }
        })
      }
      if (url.pathname.startsWith('/api/capabilities/config') && (method === 'GET' || method === 'PUT')) {
        return json({ code: 0, data: {} })
      }
      if (page && method === 'PUT') {
        const body = JSON.parse(String(init?.body ?? '{}')) as {
          markdown?: string
          path?: string
          workspaceWiki?: string
        }
        catalog.markdown = body.markdown ?? catalog.markdown
        return json({ code: 0, data: { indexed: 1 } })
      }
      return json({ code: 404, message: 'missing', data: null }, 404)
    })
  )
}

const REVIEW = row('a3s-review', {
  name: '合同审查',
  category: 'other',
  description: '审查 Word 合同或粘贴文本',
  featured: true,
  surfaces: ['skill', 'agent'],
  packageType: 'aix',
  packageId: 'use/a3s/review'
})
const QUIET = row('quiet-tool', { name: 'Quiet', category: 'other', surfaces: ['tool'], packageId: 'use/a3s/quiet' })

function renderPage() {
  return render(
    <SWRConfig value={{ provider: () => new Map(), dedupingInterval: 0 }}>
      <PluginStorePage />
    </SWRConfig>
  )
}

function calls() {
  return (globalThis.fetch as ReturnType<typeof vi.fn>).mock.calls.map(([input, init]) => {
    const url = new URL(typeof input === 'string' ? input : input instanceof URL ? input.href : input.url)
    return {
      path: url.pathname,
      search: url.searchParams,
      method: (init?.method ?? 'GET').toUpperCase(),
      body: init?.body
    }
  })
}

describe('plugin store page', () => {
  beforeEach(async () => {
    await preferenceService.set('app.workspace.path', '/Users/roy/work')
    publishAixCatalogue({ protocolVersion: 1, revision: '', packages: [] })
    publishMemberProfile({ id: 'member-a', displayName: 'A', phone: '' })
    resetPluginInstallOverlayForTests()
    openWindow.mockClear()
    openWindow.mockResolvedValue(true)
    toastError.mockReset()
    toastSuccess.mockReset()
    wire([REVIEW, QUIET])
    catalog.callable['a3s-review'] = [{ kind: 'skill', id: 'review' }]
    installFetch()
  })

  afterEach(() => publishMemberProfile(null))

  it('hides built-in office plugins and still installs a free plugin', async () => {
    const user = userEvent.setup()
    wire([
      row('a3s-approval', { name: '审批', builtin: true, surfaces: ['ui'], packageId: 'use/a3s/approval' }),
      row('quiet-tool', { name: 'Quiet', surfaces: ['tool'], packageId: 'use/a3s/quiet' })
    ])
    renderPage()
    expect(document.querySelector('[data-ui="plugins.page"]')).toHaveClass('h-full', 'w-full', 'flex-1')
    expect(document.querySelector('[data-ui="plugins.page"] .grid')).toHaveClass(
      'w-full',
      'grid-cols-1',
      '@[520px]/catalog-grid:grid-cols-2'
    )
    expect(await screen.findByRole('heading', { name: 'Quiet' })).toBeInTheDocument()
    expect(screen.queryByRole('heading', { name: '审批' })).not.toBeInTheDocument()
    const quiet = screen.getByRole('heading', { name: 'Quiet' }).closest('article')!
    await user.click(within(quiet).getByRole('button', { name: 'plugins.install' }))
    await waitFor(() => expect(catalog.rows.find((item) => item.slug === 'quiet-tool')?.installed).toBe(true))
    expect(catalog.rows.find((item) => item.slug === 'a3s-approval')?.installed).not.toBe(true)
  })

  it('does not expose a retired VSIX row in the AIX plugin store', async () => {
    const user = userEvent.setup()
    wire([
      row(`vsix-${'b'.repeat(48)}`, {
        name: 'Retired extension',
        packageType: 'vsix',
        installed: true,
        available: false,
        packageId: 'vsx/sample/retired'
      })
    ])
    renderPage()
    await user.click(screen.getByRole('button', { name: 'plugins.tab.installed' }))
    expect(await screen.findByText('capabilities.no_results')).toBeInTheDocument()
    expect(screen.queryByRole('heading', { name: 'Retired extension' })).toBeNull()
    expect(calls().some((call) => call.path.includes('/retired-installations'))).toBe(false)
    expect(calls().some((call) => call.method !== 'GET')).toBe(false)
  })

  it('loads plugins from the member catalog and installs one', async () => {
    const user = userEvent.setup()
    renderPage()
    expect(screen.queryByText('capabilities.no_results')).toBeNull()
    expect(screen.queryByText('plugins.hero.title')).toBeNull()
    await screen.findByRole('heading', { level: 3, name: '合同审查' })
    expect(screen.getByRole('heading', { level: 3, name: 'Quiet' })).toBeInTheDocument()
    expect(screen.getByText('plugins.type.agent · 1.0.0')).toBeInTheDocument()
    expect(screen.getByText('plugins.type.tool · 1.0.0')).toBeInTheDocument()
    expect(calls().some((call) => call.path === '/api/capabilities' && call.search.get('kind') === 'plugin')).toBe(true)

    expect(screen.getByRole('button', { name: 'capabilities.category.other' })).toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: 'capabilities.category.featured' }))
    await waitFor(() =>
      expect(
        calls().some((call) => call.search.get('featured') === 'true' && call.search.get('kind') === 'plugin')
      ).toBe(true)
    )
    expect(screen.queryByRole('heading', { level: 3, name: 'Quiet' })).toBeNull()
    expect(screen.getByRole('heading', { level: 3, name: '合同审查' })).toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: 'capabilities.category.other' }))
    await waitFor(() => expect(calls().some((call) => call.search.get('category') === 'other')).toBe(true))

    const review = screen.getByRole('heading', { level: 3, name: '合同审查' }).closest('article')!
    const install = within(review).getByRole('button', { name: 'plugins.install' })
    await user.click(install)
    await waitFor(() =>
      expect(
        calls().some((call) => call.method === 'PUT' && call.path === '/api/capabilities/installations/a3s-review')
      ).toBe(true)
    )
    const uninstall = await screen.findByRole('button', { name: 'capabilities.uninstall' })
    expect(uninstall).toHaveAttribute('aria-pressed', 'true')
  })

  it('installs an AIX package through the member capability flow and shows its catalog version', async () => {
    const user = userEvent.setup()
    wire([row('aix-review', { name: 'AIX Review', packageType: 'aix', surfaces: ['ui'] })])
    renderPage()

    const heading = await screen.findByRole('heading', { level: 3, name: 'AIX Review' })
    const card = heading.closest('article')!
    expect(within(card).getByText('plugins.type.plugin · 1.0.0')).toBeInTheDocument()

    await user.click(within(card).getByRole('button', { name: 'plugins.install' }))
    await waitFor(() =>
      expect(
        calls().some((call) => call.method === 'PUT' && call.path === '/api/capabilities/installations/aix-review')
      ).toBe(true)
    )
    expect(calls().some((call) => call.path.includes('/vsix') || call.path.includes('/open-vsx'))).toBe(false)
  })

  it('asks before uninstalling from the installed shelf', async () => {
    const user = userEvent.setup()
    wire([REVIEW, { ...QUIET, installed: true }])
    renderPage()
    await screen.findByRole('heading', { level: 3, name: '合同审查' })

    await user.click(screen.getByRole('button', { name: 'plugins.tab.installed' }))
    await waitFor(() => expect(calls().some((call) => call.search.get('installed') === 'true')).toBe(true))
    const uninstall = screen.getByRole('button', { name: 'capabilities.uninstall' })
    await user.click(uninstall)
    expect(calls().some((call) => call.method === 'DELETE')).toBe(false)
    const confirm = await screen.findByRole('dialog')
    expect(confirm).toHaveTextContent('capabilities.uninstall_confirm')
    await user.click(within(confirm).getByRole('button', { name: 'common.cancel' }))
    expect(calls().some((call) => call.method === 'DELETE')).toBe(false)

    await user.click(screen.getByRole('button', { name: 'capabilities.uninstall' }))
    await user.click(within(await screen.findByRole('dialog')).getByText('capabilities.uninstall'))
    await waitFor(() =>
      expect(calls().some((call) => call.method === 'DELETE' && call.path.endsWith('/quiet-tool'))).toBe(true)
    )
  })

  it('reports a failed install and restores the install control', async () => {
    catalog.failInstall = true
    const user = userEvent.setup()
    renderPage()
    const review = (await screen.findByRole('heading', { level: 3, name: '合同审查' })).closest('article')
    if (!review) throw new Error('missing review card')
    await user.click(within(review).getByRole('button', { name: 'plugins.install' }))
    await waitFor(() => expect(toastError).toHaveBeenCalledWith('capabilities.install_failed · disk'))
    expect(within(review).getByRole('button', { name: 'plugins.install' })).toHaveAttribute('aria-pressed', 'false')
  })

  it('lists agent-extension cards on their catalog shelf', async () => {
    wire([
      row('coder', {
        name: 'Coder',
        category: 'code',
        surfaces: ['agent'],
        packageId: 'use/a3s/coder'
      }),
      row('quiet-tool', { name: 'Quiet', category: 'other', surfaces: ['tool'], packageId: 'use/a3s/quiet' })
    ])
    renderPage()
    expect(await screen.findByRole('heading', { name: 'Quiet' })).toBeInTheDocument()
    expect(screen.getByRole('heading', { name: 'Coder' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'capabilities.category.code' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'capabilities.category.other' })).toBeInTheDocument()
  })

  it('keeps the catalog categories when the featured shelf is empty', async () => {
    const user = userEvent.setup()
    wire([
      row('writer', { name: '写作', category: 'education', featured: false, surfaces: ['skill'] }),
      row('quiet-tool', { name: 'Quiet', category: 'code', featured: false, surfaces: ['tool'] })
    ])
    renderPage()
    await screen.findByRole('heading', { name: '写作' })
    await user.click(screen.getByRole('button', { name: 'capabilities.category.featured' }))
    expect(await screen.findByText('capabilities.empty.featured')).toBeInTheDocument()
    expect(screen.getByText('capabilities.empty.featured_hint')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'capabilities.category.education' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'capabilities.category.code' })).toBeInTheDocument()
  })

  it('keeps the empty state off the page until the catalog answers', async () => {
    let release: (() => void) | undefined
    vi.stubGlobal(
      'fetch',
      vi.fn(
        () =>
          new Promise<Response>((resolve) => {
            release = () =>
              resolve(
                json({
                  code: 0,
                  data: { items: [], total: 0, page: 1, pageSize: 100, categories: [] }
                })
              )
          })
      )
    )
    renderPage()
    expect(screen.queryByText('capabilities.no_results')).toBeNull()
    release?.()
    const empty = await screen.findByText('capabilities.no_results')
    expect(empty).toBeInTheDocument()
  })

  it('searches the catalog with one query', async () => {
    const user = userEvent.setup()
    renderPage()
    await screen.findByRole('heading', { level: 3, name: '合同审查' })
    await user.type(screen.getByPlaceholderText('capabilities.search_placeholder'), '审查')
    await waitFor(() => expect(calls().some((call) => call.search.get('q') === '审查')).toBe(true))
    expect(screen.queryByRole('heading', { level: 3, name: 'Quiet' })).toBeNull()
  })

  it('keeps an installed composer agent visible without a standalone open action', async () => {
    wire([{ ...REVIEW, installed: true }, QUIET])
    renderPage()
    const card = (await screen.findByRole('heading', { level: 3, name: '合同审查' })).closest('article')!
    expect(within(card).queryByRole('button', { name: 'common.open' })).toBeNull()
    expect(openWindow).not.toHaveBeenCalled()
  })

  for (const applies of [true, false]) {
    it(`updates an installed agent against the device archive (${applies ? 'applied' : 'stale device'})`, async () => {
      const user = userEvent.setup()
      wire([
        row('a3s-paperloom', {
          name: '论文精读',
          packageType: 'aix',
          installed: true,
          surfaces: ['agent', 'skill'],
          packageId: 'use/a3s/paperloom'
        })
      ])
      const oldDigest = `sha256:${'cd'.repeat(32)}`
      const old = {
        slug: 'a3s-paperloom',
        packageId: 'use/a3s/paperloom',
        packageDigest: oldDigest,
        generation: 1,
        revision: oldDigest,
        containers: [],
        views: []
      }
      publishAixCatalogue({ protocolVersion: 1, revision: oldDigest, packages: [old] })
      catalog.callableDigest = oldDigest
      catalog.callable['a3s-paperloom'] = [{ kind: 'skill', id: 'paperloom' }]
      if (applies)
        catalog.afterInstall = () => {
          catalog.callableDigest = DIGEST
          publishAixCatalogue({
            protocolVersion: 1,
            revision: DIGEST,
            packages: [{ ...old, packageDigest: DIGEST, generation: 2, revision: DIGEST }]
          })
        }
      renderPage()
      const card = (await screen.findByRole('heading', { name: '论文精读' })).closest('article')!
      expect(within(card).queryByRole('button', { name: 'common.open' })).toBeNull()
      await user.click(within(card).getByRole('button', { name: 'plugins.update' }))
      if (applies) {
        await waitFor(() => expect(within(card).queryByRole('button', { name: 'plugins.update' })).toBeNull())
        expect(toastError).not.toHaveBeenCalled()
      } else {
        await waitFor(() => expect(toastError).toHaveBeenCalledWith('plugins.update_failed · not_materialized'))
        expect(within(card).getByRole('button', { name: 'plugins.update' })).toBeEnabled()
        expect(within(card).queryByText('plugins.device_apply_failed')).toBeNull()
      }
      expect(calls().filter((call) => call.method === 'PUT' && call.path.endsWith('/a3s-paperloom'))).toHaveLength(1)
      expect(calls().filter((call) => call.method === 'DELETE')).toHaveLength(0)
      expect(catalog.rows[0].installed).toBe(true)
      expect(openWindow).not.toHaveBeenCalled()
    })
  }

  it('keeps a withdrawn installation visible for removal and prevents opening it', async () => {
    wire([row('a3s-review', { name: 'Retained review', surfaces: ['ui'], installed: true, available: false })])
    renderPage()
    const title = await screen.findByRole('heading', { name: 'Retained review' })
    const card = title.closest('article')!
    expect(within(card).queryByRole('button', { name: 'common.open' })).toBeNull()
    expect(within(card).getByText('plugins.unavailable')).toBeInTheDocument()
    expect(within(card).getByRole('button', { name: 'capabilities.uninstall' })).toBeInTheDocument()
  })

  it('hides 打开 when the shell cannot launch the ui plugin', async () => {
    wire([row('custom-board', { name: '看板', installed: true, surfaces: ['ui'], packageId: 'use/a3s/custom-board' })])
    renderPage()
    const card = (await screen.findByRole('heading', { level: 3, name: '看板' })).closest('article')
    if (!card) throw new Error('missing board card')
    expect(within(card).queryByRole('button', { name: 'common.open' })).toBeNull()
    expect(openWindow).not.toHaveBeenCalled()
  })

  it('preserves account selections when declared surfaces are not callable', async () => {
    const user = userEvent.setup()
    wire([
      row('notes-skill', { name: '笔记', surfaces: ['skill'], packageId: 'use/a3s/notes-skill' }),
      row('notes-mcp', { name: '笔记连接', surfaces: ['mcp'], packageId: 'use/a3s/notes-mcp' }),
      row('quiet-tool', { name: 'Quiet', surfaces: ['tool'], packageId: 'use/a3s/quiet-tool' }),
      row('payroll-flow', { name: '发薪', surfaces: ['flow'], packageId: 'use/a3s/payroll-flow' })
    ])
    renderPage()
    await screen.findByRole('heading', { level: 3, name: '笔记' })
    for (const [name, slug] of [
      ['笔记', 'notes-skill'],
      ['笔记连接', 'notes-mcp'],
      ['Quiet', 'quiet-tool'],
      ['发薪', 'payroll-flow']
    ] as const) {
      const card = screen.getByRole('heading', { level: 3, name }).closest('article')
      if (!card) throw new Error(`missing ${name}`)
      expect(within(card).queryByRole('button', { name: 'common.open' })).toBeNull()
      await user.click(within(card).getByRole('button', { name: 'plugins.install' }))
      await waitFor(() => {
        expect(
          calls().some((call) => call.method === 'GET' && call.path === `/api/capabilities/${slug}/callable`)
        ).toBe(true)
        expect(
          calls().some((call) => call.method === 'DELETE' && call.path === `/api/capabilities/installations/${slug}`)
        ).toBe(false)
      })
      expect(within(card).getByText('plugins.device_apply_failed')).toBeInTheDocument()
      expect(within(card).getByRole('button', { name: 'capabilities.uninstall' })).toHaveAttribute(
        'aria-pressed',
        'true'
      )
      expect(within(card).queryByRole('button', { name: 'plugins.apply_to_device' })).toBeNull()
    }
    expect(toastError).toHaveBeenCalledWith('capabilities.install_failed · not_materialized')
  })

  it('keeps a saved account selection after a device error and does not offer a device retry', async () => {
    const user = userEvent.setup()
    wire([QUIET])
    const originalFetch = globalThis.fetch
    vi.stubGlobal(
      'fetch',
      vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
        const result = await originalFetch(input, init)
        if (init?.method === 'PUT') {
          return json({ code: 502, message: 'saved to your account; device failed', data: null }, 502)
        }
        return result
      })
    )
    const view = renderPage()
    await user.click(await screen.findByRole('button', { name: 'plugins.install' }))
    await screen.findByText('plugins.device_apply_failed')
    expect(screen.getByRole('button', { name: 'capabilities.uninstall' })).toHaveAttribute('aria-pressed', 'true')
    expect(catalog.rows[0].installed).toBe(true)
    expect(calls().some((call) => call.method === 'DELETE')).toBe(false)
    view.unmount()
    renderPage()
    const uninstall = await screen.findByRole('button', { name: 'capabilities.uninstall' })
    expect(screen.queryByRole('button', { name: 'plugins.apply_to_device' })).toBeNull()
    expect(uninstall).toHaveAttribute('aria-pressed', 'true')
    expect(catalog.rows[0].installed).toBe(true)
    expect(calls().some((call) => call.method === 'DELETE')).toBe(false)
  })

  it('opens a parameter dialog after a plugin with config is installed', async () => {
    const user = userEvent.setup()
    wire([
      row('a3s-finance', {
        name: '金融分析',
        installed: true,
        surfaces: ['ui'],
        packageId: 'use/a3s/finance',
        config: [{ key: 'symbol', description: '默认标的：A3S、BTC 或 ETH。', required: false, secret: false }]
      })
    ])
    renderPage()
    publishAixCatalogue({
      protocolVersion: 1,
      revision: '',
      packages: [
        {
          slug: 'a3s-finance',
          packageId: 'use/a3s/finance',
          generation: 1,
          revision: DIGEST,
          containers: [],
          views: []
        }
      ]
    })
    const card = (await screen.findByRole('heading', { level: 3, name: '金融分析' })).closest('article')
    if (!card) throw new Error('missing finance card')
    expect(within(card).queryByRole('button', { name: 'plugins.install' })).toBeNull()
    expect(within(card).queryByRole('button', { name: 'workbench.home.pin.to_home' })).toBeNull()
    await user.click(within(card).getByRole('button', { name: 'plugins.okf.configure' }))
    const input = await screen.findByLabelText('symbol')
    await user.type(input, 'BTC')
    await user.click(screen.getByRole('button', { name: 'common.save' }))
    await waitFor(() => expect(toastSuccess).toHaveBeenCalledWith('common.saved'))
    const save = calls().find((call) => call.method === 'PUT' && call.path.includes('/config/'))
    expect(save?.path).toBe('/api/capabilities/config/a3s-finance')
    expect(JSON.parse(String(save?.body))).toMatchObject({ values: { symbol: 'BTC' } })
  })

  it('saves one okf page through the shell route', async () => {
    const user = userEvent.setup()
    wire([
      REVIEW,
      row('domain-knowledge', {
        name: '领域知识',
        installed: true,
        surfaces: ['skill', 'okf'],
        packageId: 'use/a3s/domain-knowledge'
      })
    ])
    renderPage()
    const card = (await screen.findByRole('heading', { level: 3, name: '领域知识' })).closest('article')
    if (!card) throw new Error('missing knowledge card')
    await user.click(within(card).getByRole('button', { name: 'plugins.okf.configure' }))
    await user.click(await screen.findByRole('button', { name: /plugins\.okf\.edit/ }))
    await user.click(await screen.findByRole('button', { name: '保存' }))
    await waitFor(() => expect(toastSuccess).toHaveBeenCalledWith('plugins.okf.saved'))
    const save = calls().find((call) => call.method === 'PUT' && call.path.endsWith('/okf/page'))
    expect(save?.path).toBe('/api/capabilities/domain-knowledge/okf/page')
    expect(JSON.parse(String(save?.body))).toMatchObject({
      path: 'okf/domain/index.md',
      markdown: '---\ntitle: 领域\n---\n甲方付款已修订\n',
      workspaceWiki: '/Users/roy/work/wiki'
    })
  })

  it('pins an installed plugin onto the home and removes that pin with the plugin', async () => {
    const user = userEvent.setup()
    wire([row('quiet-tool', { name: 'Quiet', installed: true, surfaces: ['tool'], packageId: 'use/a3s/quiet' })])
    publishAixCatalogue({
      protocolVersion: 1,
      revision: '',
      packages: [
        {
          slug: 'quiet-tool',
          packageId: 'use/a3s/quiet',
          generation: 1,
          revision: DIGEST,
          containers: [],
          views: [],
          home: [
            {
              id: 'notice',
              shelf: 'write',
              title: '通知',
              description: '事项',
              order: 10,
              action: 'writing',
              writingType: 'notice'
            }
          ]
        }
      ]
    })
    renderPage()
    const card = (await screen.findByRole('heading', { name: 'Quiet' })).closest('article')
    if (!card) throw new Error('missing plugin card')
    const pin = within(card).getByRole('button', { name: 'workbench.home.pin.to_home' })
    expect(pin.querySelector('svg')).toBeTruthy()
    expect(pin).toHaveClass('size-[30px]')
    expect(within(card).getByTestId('tooltip-content')).toHaveTextContent('workbench.home.pin.to_home')
    await user.click(pin)
    expect(readHomePluginPins('member-a')).toEqual(['quiet-tool'])
    const unpin = within(card).getByRole('button', { name: 'workbench.home.pin.remove' })
    expect(unpin.querySelector('svg')).toBeTruthy()
    expect(within(card).getByTestId('tooltip-content')).toHaveTextContent('workbench.home.pin.remove')
    pinHomePlugin('member-a', 'quiet-tool')
    await user.click(within(card).getByRole('button', { name: 'capabilities.uninstall' }))
    await user.click(within(await screen.findByRole('dialog')).getByRole('button', { name: 'capabilities.uninstall' }))
    await waitFor(() => expect(readHomePluginPins('member-a')).toEqual([]))
  })

  it('installs and uninstalls 数据分析, and adds it to the ActivityBar after install', async () => {
    const user = userEvent.setup()
    wire([
      row('a3s-data', {
        name: '数据分析',
        installed: false,
        surfaces: ['ui'],
        packageId: 'use/a3s/data'
      })
    ])
    publishAixCatalogue(dataWorkbenchCatalogue())
    renderPage()
    const card = (await screen.findByRole('heading', { name: '数据分析' })).closest('article')
    if (!card) throw new Error('missing data card')
    await user.click(within(card).getByRole('button', { name: 'plugins.install' }))
    await waitFor(() => expect(catalog.rows.find((item) => item.slug === 'a3s-data')?.installed).toBe(true))
    expect(within(card).getByRole('button', { name: 'capabilities.uninstall' })).toBeInTheDocument()
    expect(within(card).getByRole('button', { name: 'common.open' })).toBeInTheDocument()
    await user.click(within(card).getByRole('button', { name: 'plugins.okf.configure' }))
    const toggle = await screen.findByRole('switch', { name: 'plugins.activity_bar.label' })
    expect(toggle).not.toBeChecked()
    await user.click(toggle)
    await user.click(screen.getByRole('button', { name: 'common.save' }))
    await waitFor(() =>
      expect(preferenceService.getCachedValue('plugins.activity_bar')).toEqual({ 'aix.a3s-data': true })
    )
    // Save closes the dialog. Uninstall is on the card behind it.
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument())
    await user.click(within(card).getByRole('button', { name: 'capabilities.uninstall' }))
    await user.click(within(await screen.findByRole('dialog')).getByRole('button', { name: 'capabilities.uninstall' }))
    await waitFor(() => expect(catalog.rows.find((item) => item.slug === 'a3s-data')?.installed).toBe(false))
    expect(within(card).getByRole('button', { name: 'plugins.install' })).toBeInTheDocument()
  })
})
