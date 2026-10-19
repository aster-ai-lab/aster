import { act, render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import i18n from 'i18next'
import { SWRConfig } from 'swr'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { adoptEditorCenter, bindCenterMember, releaseCenterMember } from '@renderer/data/memberOrgs'
import type { PlatformItem } from '@renderer/data/platformCatalog'

import { writePlatformFacet, writePlatformMenu } from '../platformNav'
import PlatformPage from '../PlatformPage'

vi.mock('@a3s-lab/ui', async () => vi.importActual('@a3s-lab/ui'))
vi.mock('@tanstack/react-router', () => ({ useNavigate: () => vi.fn() }))

const say = (key: string, values?: Record<string, unknown>) => String(i18n.t(key, values))
let rows: PlatformItem[]
let failed: boolean
const checkbox = (n: number) =>
  screen.getByRole('checkbox', {
    name: say('collection.selectRow', { name: `Plugin ${String(n).padStart(2, '0')}`, id: `plugin-${n}` })
  })
function renderPage() {
  return render(
    <SWRConfig value={{ provider: () => new Map(), dedupingInterval: 0, shouldRetryOnError: false }}>
      <PlatformPage />
    </SWRConfig>
  )
}
beforeEach(async () => {
  await i18n.changeLanguage('en-US')
  localStorage.clear()
  releaseCenterMember()
  adoptEditorCenter('platform-team')
  bindCenterMember('11111111-1111-4111-8111-111111111111')
  writePlatformMenu('services')
  writePlatformFacet('services', 'plugins')
  failed = false
  rows = Array.from({ length: 23 }, (_, n) => ({
    id: `plugin-${n}`,
    name: `Plugin ${String(n).padStart(2, '0')}`,
    meta: '',
    fields: [
      { key: 'category', value: n % 2 ? 'general' : 'developer' },
      { key: 'status', value: 'enabled' }
    ]
  }))
  Object.defineProperty(globalThis, '__A3S_STUDIO_RUNTIME__', {
    value: { apiBaseUrl: 'http://127.0.0.1:9', token: 'cd'.repeat(32) },
    configurable: true
  })
  vi.spyOn(globalThis, 'fetch').mockImplementation(async (input) => {
    const url = new URL(String(input))
    if (url.pathname === '/api/member-orgs')
      return new Response(
        JSON.stringify({
          code: 0,
          data: [
            {
              id: 'platform-team',
              name: 'Platform',
              membership: 'admin',
              creator: true,
              is_official: true
            }
          ]
        })
      )
    if (url.pathname.endsWith('/suppliers/access'))
      return new Response(
        JSON.stringify({
          code: 0,
          data: {
            read: true,
            create: false,
            update: false,
            delete: false
          }
        })
      )
    if (failed && url.pathname.endsWith('/services')) return new Response('unavailable', { status: 503 })
    const data = url.pathname.endsWith('/services') && url.searchParams.get('kind') === 'plugins' ? rows : []
    return new Response(JSON.stringify({ code: 0, data }))
  })
})
afterEach(() => {
  vi.restoreAllMocks()
  releaseCenterMember()
  Reflect.deleteProperty(globalThis, '__A3S_STUDIO_RUNTIME__')
})

describe('platform collection controls with real shared primitives and member reads', () => {
  // Catches controls stranded in the page header, and redundant navigation for a single page.
  it('keeps search, category, resource switching and refresh in the named table controls', async () => {
    const user = userEvent.setup()
    rows = rows.slice(0, 4)
    renderPage()
    await screen.findByRole('checkbox', { name: say('collection.selectPage') })
    const toolbar = screen.getByRole('region', {
      name: say('collection.controls', { name: say('platform.nav.services') })
    })
    expect(within(toolbar).getByRole('searchbox', { name: say('common.search') })).toBeVisible()
    expect(within(toolbar).getByRole('button', { name: say('platform.col.category') })).toBeVisible()
    expect(within(toolbar).getByRole('radio', { name: say('platform.services.plugins') })).toBeChecked()
    expect(within(toolbar).getByRole('button', { name: say('common.refresh') })).toBeVisible()
    expect(screen.getByRole('navigation', { name: say('collection.pagination') })).toHaveTextContent(
      say('collection.range', { start: 1, end: 4, total: 4 })
    )
    expect(screen.queryByRole('group', { name: say('collection.pageNumbers') })).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: say('collection.rowsPerPage') })).toHaveTextContent('20')
    await user.type(within(toolbar).getByRole('searchbox', { name: say('common.search') }), 'no matching plugin')
    expect(screen.getByRole('navigation', { name: say('collection.pagination') })).toHaveTextContent(
      say('collection.emptyRange')
    )
    expect(screen.getByLabelText(say('collection.rowsPerPage'))).toBeEnabled()
    expect(screen.queryByRole('checkbox')).not.toBeInTheDocument()
  })

  it('clears selection when category changes and never restores hidden selected records', async () => {
    const user = userEvent.setup()
    rows = rows.slice(0, 4)
    renderPage()
    await screen.findByRole('checkbox', { name: say('collection.selectPage') })
    await user.click(checkbox(0))
    await user.click(
      within(
        screen.getByRole('region', { name: say('collection.controls', { name: say('platform.nav.services') }) })
      ).getByRole('button', { name: say('platform.col.category') })
    )
    await user.click(screen.getByRole('option', { name: 'general 2' }))
    expect(checkbox(1)).not.toBeChecked()
    expect(screen.queryByRole('region', { name: say('collection.selectedActions') })).not.toBeInTheDocument()
    await user.click(
      within(
        screen.getByRole('region', { name: say('collection.controls', { name: say('platform.nav.services') }) })
      ).getByRole('button', { name: say('platform.col.category') })
    )
    await user.click(screen.getByRole('option', { name: 'All 4' }))
    expect(checkbox(0)).not.toBeChecked()
  })

  // Catches detail opening on checkbox input, all-dataset selection, lost mixed state and stale page selections.
  it('selects through keyboard and check-all, copies only the current page and clears on pagination', async () => {
    const user = userEvent.setup()
    renderPage()
    await screen.findByRole('checkbox', { name: say('collection.selectPage') })
    checkbox(0).focus()
    await user.keyboard('[Space]')
    expect(checkbox(0)).toBeChecked()
    expect(screen.getByRole('checkbox', { name: say('collection.selectPage') })).toBePartiallyChecked()
    expect(screen.getByRole('button', { name: /Plugin 00/ })).toHaveAttribute('aria-expanded', 'false')
    await user.click(screen.getByRole('checkbox', { name: say('collection.selectPage') }))
    const bar = screen.getByRole('region', { name: say('collection.selectedActions') })
    expect(bar).toHaveTextContent(say('collection.selected', { count: 20 }))
    const write = vi.spyOn(navigator.clipboard, 'writeText').mockResolvedValue()
    await user.click(within(bar).getByRole('button', { name: say('collection.copyIds') }))
    await waitFor(() => expect(bar).toHaveTextContent(say('collection.copiedIds', { count: 20 })))
    expect(write).toHaveBeenCalledWith(Array.from({ length: 20 }, (_, n) => `plugin-${n}`).join('\n'))
    await user.click(within(bar).getByRole('button', { name: say('collection.clearSelection') }))
    expect(screen.getByRole('checkbox', { name: say('collection.selectPage') })).toHaveFocus()
    await user.click(checkbox(0))
    await user.click(screen.getByRole('button', { name: say('common.next') }))
    expect(checkbox(20)).not.toBeChecked()
    expect(screen.queryByRole('region', { name: say('collection.selectedActions') })).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: say('common.next') })).toBeDisabled()
    await user.click(screen.getByRole('button', { name: say('common.previous') }))
    expect(checkbox(0)).not.toBeChecked()
  })

  // Catches a click beside the 16px box falling through to the row and opening detail instead of selecting.
  it('selects from anywhere in the checkbox cell without expanding the row', async () => {
    const user = userEvent.setup()
    rows = rows.slice(0, 2)
    renderPage()
    await screen.findByRole('checkbox', { name: say('collection.selectPage') })
    const cell = checkbox(0).closest('td')!
    await user.click(cell)
    expect(checkbox(0)).toBeChecked()
    expect(screen.getByRole('button', { name: /Plugin 00/ })).toHaveAttribute('aria-expanded', 'false')
    await user.click(cell)
    expect(checkbox(0)).not.toBeChecked()
  })

  // Catches every background refresh disabling (and visibly fading) selection on rows of the same query.
  it('keeps selection usable while the same query refreshes', async () => {
    const user = userEvent.setup()
    rows = rows.slice(0, 2)
    renderPage()
    await screen.findByRole('checkbox', { name: say('collection.selectPage') })
    await user.click(checkbox(0))
    let release = () => {}
    const original = vi.mocked(globalThis.fetch).getMockImplementation()!
    vi.mocked(globalThis.fetch).mockImplementation(async (input, init) => {
      if (String(input).includes('/services')) await new Promise<void>((resolve) => (release = resolve))
      return original(input, init)
    })
    await user.click(screen.getByRole('button', { name: say('common.refresh') }))
    expect(screen.getByRole('button', { name: say('common.refresh') })).toBeDisabled()
    expect(checkbox(0)).toBeEnabled()
    expect(checkbox(0)).toBeChecked()
    expect(screen.getByRole('button', { name: say('collection.copyIds') })).toBeEnabled()
    await act(async () => release())
    await waitFor(() => expect(screen.getByRole('button', { name: say('common.refresh') })).toBeEnabled())
    expect(checkbox(0)).toBeChecked()
  })

  // Catches a failed clipboard clearing selection, or refresh/query/view changes reviving hidden selected IDs.
  it('retains failed copy selections, reconciles refresh removals and resets search, size and resource scope', async () => {
    const user = userEvent.setup()
    renderPage()
    await screen.findByRole('checkbox', { name: say('collection.selectPage') })
    await user.click(checkbox(0))
    vi.spyOn(navigator.clipboard, 'writeText').mockRejectedValue(new Error('clipboard denied'))
    await user.click(screen.getByRole('button', { name: say('collection.copyIds') }))
    expect(await screen.findByRole('alert')).toHaveTextContent(say('common.copy_failed'))
    expect(checkbox(0)).toBeChecked()
    rows = rows.slice(1)
    await user.click(screen.getByRole('button', { name: say('common.refresh') }))
    await waitFor(() => expect(screen.queryByRole('checkbox', { name: /plugin-0\)/ })).not.toBeInTheDocument())
    expect(screen.queryByRole('region', { name: say('collection.selectedActions') })).not.toBeInTheDocument()
    await user.click(checkbox(1))
    await user.click(screen.getByRole('button', { name: say('collection.rowsPerPage') }))
    await user.click(screen.getByRole('option', { name: '10' }))
    expect(checkbox(1)).not.toBeChecked()
    await user.click(checkbox(1))
    await user.type(screen.getByRole('searchbox', { name: say('common.search') }), 'Plugin 01')
    expect(checkbox(1)).not.toBeChecked()
    await user.click(checkbox(1))
    await user.click(screen.getByRole('radio', { name: say('platform.services.skills') }))
    expect(screen.queryByRole('region', { name: say('collection.selectedActions') })).not.toBeInTheDocument()
    await user.click(screen.getByRole('radio', { name: say('platform.services.plugins') }))
    expect(checkbox(1)).not.toBeChecked()
    await user.click(checkbox(1))
    failed = true
    await user.click(screen.getByRole('button', { name: say('common.refresh') }))
    await screen.findByRole('button', { name: say('common.retry') })
    expect(screen.queryByRole('checkbox')).not.toBeInTheDocument()
    expect(screen.queryByRole('region', { name: say('collection.selectedActions') })).not.toBeInTheDocument()
    failed = false
    await user.click(screen.getByRole('button', { name: say('common.retry') }))
    await waitFor(() => expect(checkbox(1)).not.toBeChecked())
    await act(async () => {
      adoptEditorCenter('other-team')
    })
    expect(screen.queryByRole('checkbox')).not.toBeInTheDocument()
  })
})
