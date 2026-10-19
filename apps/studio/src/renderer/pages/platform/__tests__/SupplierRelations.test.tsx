import { act, fireEvent, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import i18n from 'i18next'
import { SWRConfig } from 'swr'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { SupplierRelations } from '../SupplierRelations'

vi.mock('@a3s-lab/ui', async () => vi.importActual('@a3s-lab/ui'))

const id = '11111111-1111-4111-8111-111111111111'
const say = (key: string, values?: Record<string, unknown>) =>
  String(i18n.getFixedT(null, null, 'platform.supplierRelations')(key, values))
let queries: URL[]
let models: string[]
let invalid: boolean
let barrier: Promise<void> | undefined
function tree(scope = 'account-a/team', authorized = true, cache = new Map()) {
  return (
    <SWRConfig value={{ provider: () => cache, dedupingInterval: 0, shouldRetryOnError: false }}>
      <SupplierRelations supplierId={id} scope={scope} authorized={authorized} />
    </SWRConfig>
  )
}
beforeEach(async () => {
  await i18n.changeLanguage('en-US')
  queries = []
  models = ['exact/model%_', ...Array.from({ length: 42 }, (_, n) => `model-${String(n).padStart(2, '0')}`)]
  invalid = false
  barrier = undefined
  Object.defineProperty(globalThis, '__A3S_STUDIO_RUNTIME__', {
    value: { apiBaseUrl: 'http://127.0.0.1:9', token: 'ab'.repeat(32) },
    configurable: true
  })
  vi.stubGlobal(
    'fetch',
    vi.fn<typeof fetch>(async (input) => {
      const url = new URL(String(input))
      queries.push(url)
      const kind = url.pathname.endsWith('/models/page') ? 'models' : 'dependencies'
      const limit = Number(url.searchParams.get('limit'))
      const source =
        kind === 'models'
          ? models.map((model) => ({
              id: model,
              name: model,
              state: null,
              upstreamModels: [] as string[],
              dependencyCount: model === 'exact/model%_' ? 32 : 0
            }))
          : Array.from({ length: 33 }, (_, n) => ({
              id: `dependent-${n}`,
              name: `Dependent ${n}`,
              state: 'draft',
              upstreamModels: [n ? 'exact/model%_' : 'undeclared/upstream'],
              dependencyCount: 0
            }))
      const exact = url.searchParams.get('model') ?? ''
      const search = url.searchParams.get('search')?.toLowerCase() ?? ''
      const matching = source.filter(
        (item) =>
          (!exact || item.upstreamModels.includes(exact)) &&
          [item.id, item.name, ...item.upstreamModels].join(' ').toLowerCase().includes(search)
      )
      const page = Math.min(Number(url.searchParams.get('page')), Math.max(1, Math.ceil(matching.length / limit)))
      const data = {
        supplierId: id,
        kind,
        items: invalid ? [matching[0], matching[0]] : matching.slice((page - 1) * limit, page * limit),
        total: matching.length,
        page,
        limit,
        revision: 101,
        dependencyTotal: 33
      }
      if (barrier && Number(url.searchParams.get('page')) === 2) await barrier
      return new Response(JSON.stringify({ code: 0, data }))
    })
  )
})
afterEach(() => {
  vi.unstubAllGlobals()
  Reflect.deleteProperty(globalThis, '__A3S_STUDIO_RUNTIME__')
})

describe('supplier relation workflows with actual controls and member client', () => {
  it('reads later owner pages, changes page size and drills into exact stored model dependencies', async () => {
    render(tree())
    const user = userEvent.setup()
    await screen.findByText('model-00')
    expect(screen.queryByText('model-41')).not.toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: String(i18n.t('collection.page', { page: 3 })) }))
    expect(await screen.findByText('model-41')).toBeVisible()
    expect(screen.queryByText('model-00')).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: String(i18n.t('common.next')) })).toBeDisabled()
    await user.selectOptions(screen.getByLabelText(String(i18n.t('collection.rowsPerPage'))), '50')
    expect(await screen.findByText('model-00')).toBeVisible()
    await user.click(screen.getByRole('button', { name: say('viewDependencies', { model: 'exact/model%_' }) }))
    expect(await screen.findByText('Dependent 1')).toBeVisible()
    expect(screen.queryByText('Dependent 0')).not.toBeInTheDocument()
    expect(queries.at(-1)?.searchParams.get('model')).toBe('exact/model%_')
    expect(screen.getByText(say('dependencySummary', { count: 33 }))).toBeVisible()
    await user.click(screen.getByRole('button', { name: say('clearModel', { model: 'exact/model%_' }) }))
    expect(await screen.findByText('undeclared/upstream')).toBeVisible()
    expect(screen.getByText('Dependent 0')).toBeVisible()
  })
  it('waits for IME completion, searches literal wildcard text and resets to the first page', async () => {
    render(tree())
    const user = userEvent.setup()
    await screen.findByText('model-00')
    await user.click(screen.getByRole('button', { name: String(i18n.t('collection.page', { page: 3 })) }))
    await screen.findByText('model-41')
    const input = screen.getByRole('searchbox', { name: say('search') })
    fireEvent.compositionStart(input)
    fireEvent.change(input, { target: { value: '%_' } })
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 350))
    })
    expect(screen.getByText('model-41')).toBeVisible()
    expect(queries.some((query) => query.searchParams.get('search') === '%_')).toBe(false)
    fireEvent.compositionEnd(input, { data: '%_' })
    expect(await screen.findByText('exact/model%_')).toBeVisible()
    expect(screen.queryByText('model-41')).not.toBeInTheDocument()
    expect(queries.at(-1)?.searchParams.get('page')).toBe('1')
    expect(queries.at(-1)?.searchParams.get('search')).toBe('%_')
  })
  it('withdraws prior rows after an invalid refresh and retries only the read', async () => {
    render(tree())
    const user = userEvent.setup()
    await screen.findByText('model-00')
    invalid = true
    await user.click(screen.getByRole('button', { name: String(i18n.t('common.refresh')) }))
    expect(await screen.findByRole('alert')).toHaveTextContent(say('failed'))
    expect(screen.queryByText('model-00')).not.toBeInTheDocument()
    invalid = false
    await user.click(screen.getByRole('button', { name: String(i18n.t('common.retry')) }))
    expect(await screen.findByText('model-00')).toBeVisible()
    expect(queries.every((query) => query.pathname.endsWith('/models/page'))).toBe(true)
  })
  it('hides protected rows when authority is withdrawn and ignores a late page after changing account scope', async () => {
    const cache = new Map()
    const view = render(tree('account-a/team', true, cache))
    const user = userEvent.setup()
    await screen.findByText('model-00')
    let finish!: () => void
    barrier = new Promise((resolve) => {
      finish = resolve
    })
    await user.click(screen.getByRole('button', { name: String(i18n.t('common.next')) }))
    await waitFor(() => expect(queries.at(-1)?.searchParams.get('page')).toBe('2'))
    view.rerender(tree('account-a/team', false, cache))
    expect(screen.queryByRole('region', { name: say('title') })).not.toBeInTheDocument()
    await act(async () => {
      finish()
      await barrier
    })
    models = ['new-account-model']
    barrier = undefined
    view.rerender(tree('account-b/team', true, cache))
    expect(await screen.findByText('new-account-model')).toBeVisible()
    expect(screen.queryByText('model-19')).not.toBeInTheDocument()
    expect(screen.queryByText('model-00')).not.toBeInTheDocument()
  })
})
