// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { createInstance } from 'i18next'
import { I18nextProvider } from 'react-i18next'
import { SWRConfig } from 'swr'
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest'

import { dataApiService } from '@data/DataApiService'
import type { WebviewTag } from '@renderer/host/webviewTypes'
import en from '@renderer/i18n/locales/en-us.json'
import { agentBrowserRuntimeService as runtime } from '@renderer/services/AgentBrowserRuntimeService'
import { WebviewSecurityProfile } from '@shared/utils/webviewSecurity'

import { SessionBrowserView } from '../SessionBrowserView'
import { WebviewNavigation } from '../WebviewNavigation'

vi.unmock('@a3s-lab/ui')
vi.unmock('@data/hooks/useDataApi')
vi.unmock('react-i18next')
vi.mock('@renderer/components/WebviewAnnotationControls', () => ({ WebviewAnnotationControls: () => null }))

const i18n = createInstance()
const visit = (url: string, title: string) => ({ id: title, title, url, visitedAt: 1, source: 'local' })
const report = visit('http://internal.test/reports?lang=zh#latest', 'Monthly report')
const recent = [report, { ...report, title: 'Old report title' }, visit('https://example.com/', 'Example')]
const result = (items = recent) => ({ items, hasMore: false })

beforeAll(async () => {
  await i18n.init({ lng: 'en', resources: { en: { translation: en } }, keySeparator: false })
})
beforeEach(() => vi.mocked(dataApiService.get).mockReset().mockResolvedValue(result()))
afterEach(cleanup)

function setup(historyEnabled = true, url = 'https://current.test/') {
  const loadURL = vi.fn().mockResolvedValue(undefined)
  const guest = {
    getURL: () => url,
    canGoBack: () => false,
    canGoForward: () => false,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    loadURL
  } as unknown as WebviewTag
  render(
    <SWRConfig value={{ provider: () => new Map() }}>
      <I18nextProvider i18n={i18n}>
        <WebviewNavigation
          initialUrl={url}
          webviewRef={{ current: guest }}
          webviewRevision={1}
          historyEnabled={historyEnabled}
          isWebviewReady
          isHostActive
          target={{ id: 'test-browser', label: 'Browser' }}
        />
      </I18nextProvider>
    </SWRConfig>
  )
  return { loadURL, address: screen.getByRole(historyEnabled ? 'combobox' : 'textbox', { name: 'Web address' }) }
}

describe('Browser address history', () => {
  it('loads history only on focus, deduplicates URLs and opens a searched result with the keyboard', async () => {
    const user = userEvent.setup()
    const { address, loadURL } = setup()
    expect(dataApiService.get).not.toHaveBeenCalled()
    await user.click(address)
    expect(await screen.findAllByRole('option')).toHaveLength(2)
    expect(screen.getByRole('option', { name: /Monthly report/ })).toBeVisible()
    expect(screen.queryByText('Old report title')).not.toBeInTheDocument()
    vi.mocked(dataApiService.get).mockResolvedValue(result([report]))
    await user.clear(address)
    await user.type(address, 'report')
    await waitFor(() =>
      expect(dataApiService.get).toHaveBeenLastCalledWith(
        '/browser-visits',
        expect.objectContaining({ query: { search: 'report', offset: 0, limit: 20 } })
      )
    )
    await screen.findByRole('option', { name: /Monthly report/ })
    await user.keyboard('{ArrowDown}{Enter}')
    expect(loadURL).toHaveBeenCalledWith(report.url)
    expect(screen.queryByRole('listbox')).not.toBeInTheDocument()
  })

  it('opens a clicked history item without blur replacing its URL', async () => {
    const user = userEvent.setup()
    const { address, loadURL } = setup()
    await user.click(address)
    await user.click(await screen.findByRole('option', { name: /Monthly report/ }))
    expect(loadURL).toHaveBeenCalledWith(report.url)
    expect(address).toHaveValue('internal.test/reports?lang=zh#latest')
  })

  it('delivers an address to the host when the page is not an Electron guest', async () => {
    const user = userEvent.setup()
    const onNavigate = vi.fn()
    render(
      <SWRConfig value={{ provider: () => new Map() }}>
        <I18nextProvider i18n={i18n}>
          <WebviewNavigation
            initialUrl="about:blank"
            webviewRef={{ current: null }}
            webviewRevision={0}
            historyEnabled={false}
            isWebviewReady
            isHostActive
            onNavigate={onNavigate}
            target={{ id: 'test-browser', label: 'Browser' }}
          />
        </I18nextProvider>
      </SWRConfig>
    )
    const address = screen.getByRole('textbox', { name: 'Web address' })
    await user.click(address)
    await user.clear(address)
    await user.type(address, 'https://example.com/docs')
    expect(onNavigate).not.toHaveBeenCalled()
    await user.keyboard('{Enter}')
    expect(onNavigate).toHaveBeenCalledWith('https://example.com/docs')
  })

  it('opens a workspace search on Quark', async () => {
    const user = userEvent.setup()
    const onNavigate = vi.fn()
    render(
      <SWRConfig value={{ provider: () => new Map() }}>
        <I18nextProvider i18n={i18n}>
          <WebviewNavigation
            initialUrl="about:blank"
            webviewRef={{ current: null }}
            webviewRevision={0}
            historyEnabled
            isWebviewReady
            isHostActive
            onNavigate={onNavigate}
            target={{ id: 'test-browser', label: 'Browser' }}
          />
        </I18nextProvider>
      </SWRConfig>
    )
    const address = screen.getByRole('combobox', { name: 'Web address' })
    await user.click(address)
    fireEvent.change(address, { target: { value: '天气预报' } })
    expect(onNavigate).not.toHaveBeenCalled()
    await user.keyboard('{Enter}')
    expect(onNavigate).toHaveBeenCalledOnce()
    expect(new URL(String(onNavigate.mock.calls[0]?.[0])).searchParams.get('q')).toBe('天气预报')
    expect(String(onNavigate.mock.calls[0]?.[0])).toMatch(/^https:\/\/quark\.sm\.cn\/s\?/)
  })

  it('shows Quark suggestions and opens the chosen word', async () => {
    const invoke = vi.fn(async (command: string, args?: { query?: string }) => {
      if (command === 'quark_search_suggestions' && args?.query === '天气') return ['天气预报', '天气之子']
      return []
    })
    Object.assign(globalThis, { __TAURI_INTERNALS__: { invoke } })
    try {
      const user = userEvent.setup()
      const onNavigate = vi.fn()
      render(
        <SWRConfig value={{ provider: () => new Map() }}>
          <I18nextProvider i18n={i18n}>
            <WebviewNavigation
              initialUrl="about:blank"
              webviewRef={{ current: null }}
              webviewRevision={0}
              historyEnabled
              isWebviewReady
              isHostActive
              onNavigate={onNavigate}
              target={{ id: 'test-browser', label: 'Browser' }}
            />
          </I18nextProvider>
        </SWRConfig>
      )
      const address = screen.getByRole('combobox', { name: 'Web address' })
      await user.click(address)
      fireEvent.change(address, { target: { value: '天气' } })
      expect(invoke).not.toHaveBeenCalled()
      const suggestion = await screen.findByRole('option', { name: '天气预报' })
      expect(screen.getByRole('option', { name: '天气之子' })).toBeVisible()
      expect(invoke).toHaveBeenCalledWith('quark_search_suggestions', { query: '天气' })
      await user.click(suggestion)
      expect(onNavigate).toHaveBeenCalledOnce()
      expect(new URL(String(onNavigate.mock.calls[0]?.[0])).searchParams.get('q')).toBe('天气预报')
    } finally {
      Reflect.deleteProperty(globalThis, '__TAURI_INTERNALS__')
    }
  })

  it('does not ask Quark for a web address', async () => {
    const invoke = vi.fn(async () => ['天气预报'])
    Object.assign(globalThis, { __TAURI_INTERNALS__: { invoke } })
    try {
      const user = userEvent.setup()
      const { address } = setup()
      await user.click(address)
      fireEvent.change(address, { target: { value: 'example.com' } })
      await act(async () => {
        await new Promise((resolve) => setTimeout(resolve, 350))
      })
      expect(invoke).not.toHaveBeenCalled()
      expect(screen.queryByRole('option', { name: '天气预报' })).not.toBeInTheDocument()
    } finally {
      Reflect.deleteProperty(globalThis, '__TAURI_INTERNALS__')
    }
  })

  it('searches history 300ms after the last address change', async () => {
    vi.useFakeTimers()
    try {
      const { address } = setup()
      fireEvent.focus(address)
      fireEvent.change(address, { target: { value: 'r' } })
      await act(async () => {
        await vi.advanceTimersByTimeAsync(299)
      })
      fireEvent.change(address, { target: { value: 're' } })
      await act(async () => {
        await vi.advanceTimersByTimeAsync(299)
      })
      const searches = () =>
        vi.mocked(dataApiService.get).mock.calls.map((call) => {
          const query = call[1] as { query?: { search?: string } } | undefined
          return query?.query?.search
        })
      expect(searches()).not.toContain('r')
      expect(searches()).not.toContain('re')
      await act(async () => {
        await vi.advanceTimersByTimeAsync(1)
      })
      expect(searches()).toContain('re')
      expect(searches()).not.toContain('r')
    } finally {
      vi.useRealTimers()
    }
  })

  it('keeps free-form URL submission and ignores Enter while composing', async () => {
    const user = userEvent.setup()
    const { address, loadURL } = setup()
    await user.click(address)
    await screen.findAllByRole('option')
    await user.clear(address)
    await user.type(address, 'http://localhost:3000/new')
    fireEvent.keyDown(address, { key: 'Enter', isComposing: true })
    expect(loadURL).not.toHaveBeenCalled()
    await user.keyboard('{Enter}')
    expect(loadURL).toHaveBeenCalledWith('http://localhost:3000/new')
  })

  it('dismisses suggestions with Escape and restores the current address without navigation', async () => {
    const user = userEvent.setup()
    const { address, loadURL } = setup()
    await user.click(address)
    await screen.findAllByRole('option')
    await user.clear(address)
    await user.type(address, 'report')
    await user.keyboard('{Escape}')
    expect(screen.queryByRole('listbox')).not.toBeInTheDocument()
    expect(address).toHaveValue('current.test')
    expect(loadURL).not.toHaveBeenCalled()
  })

  it('cannot select an earlier search while a newer search is loading or after its late response', async () => {
    const user = userEvent.setup()
    const { address, loadURL } = setup()
    await user.click(address)
    await screen.findAllByRole('option')
    let finish!: (value: ReturnType<typeof result>) => void
    vi.mocked(dataApiService.get).mockReturnValueOnce(
      new Promise<ReturnType<typeof result>>((resolve) => {
        finish = resolve
      })
    )
    await user.clear(address)
    await user.type(address, 'old')
    await waitFor(() =>
      expect(dataApiService.get).toHaveBeenLastCalledWith(
        '/browser-visits',
        expect.objectContaining({ query: { search: 'old', offset: 0, limit: 20 } })
      )
    )
    vi.mocked(dataApiService.get).mockResolvedValue(result([visit('https://new.test/', 'New result')]))
    await user.clear(address)
    await user.type(address, 'new')
    expect(screen.queryByRole('option')).not.toBeInTheDocument()
    await screen.findByRole('option', { name: /New result/ })
    await act(async () => finish(result([report])))
    expect(screen.queryByRole('option', { name: /Monthly report/ })).not.toBeInTheDocument()
    await user.keyboard('{ArrowDown}{Enter}')
    expect(loadURL).toHaveBeenCalledWith('https://new.test/')
  })

  it('does not expose ordinary browser history in preview surfaces', async () => {
    const user = userEvent.setup()
    const { address } = setup(false)
    await user.click(address)
    await user.clear(address)
    await user.type(address, 'report')
    expect(dataApiService.get).not.toHaveBeenCalled()
    expect(screen.queryByRole('listbox')).not.toBeInTheDocument()
  })
})

describe('Browser address display', () => {
  it.each([
    ['https://github.com/maizzle/framework?tab=readme#start', 'github.com/maizzle/framework?tab=readme#start'],
    ['http://localhost:3000/reports?lang=zh#latest', 'localhost:3000/reports?lang=zh#latest'],
    ['https://docs.example.com/project', 'docs.example.com/project'],
    ['file:///tmp/preview.html', 'file:///tmp/preview.html']
  ])('expands %s only when focused and restores it after cancelling an edit', async (url, compact) => {
    const user = userEvent.setup()
    const { address, loadURL } = setup(false, url)
    expect(address).toHaveValue(compact)
    await user.hover(address)
    expect(address).toHaveValue(compact)
    await user.click(address)
    expect(address).toHaveValue(url)
    const input = address as HTMLInputElement
    expect(input.selectionStart).toBe(0)
    expect(input.selectionEnd).toBe(url.length)
    await user.keyboard('unsaved draft')
    await user.tab()
    expect(address).toHaveValue(compact)
    fireEvent.focus(address)
    expect(address).toHaveValue(url)
    expect(loadURL).not.toHaveBeenCalled()
  })
})

describe('Embedded page history', () => {
  afterEach(() => {
    runtime.dispose()
    delete (globalThis as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__
  })

  it('enables back from the embedded page and places the tip above that page', async () => {
    const user = userEvent.setup()
    const onBack = vi.fn()
    render(
      <SWRConfig value={{ provider: () => new Map() }}>
        <I18nextProvider i18n={i18n}>
          <WebviewNavigation
            initialUrl="https://current.test/"
            webviewRef={{ current: null }}
            webviewRevision={0}
            historyEnabled
            isWebviewReady
            isHostActive
            tooltipPlacement="top"
            navigationControls={{ canGoBack: true, canGoForward: false, onBack, onForward: vi.fn() }}
            target={{ id: 'test-browser', label: 'Browser' }}
          />
        </I18nextProvider>
      </SWRConfig>
    )
    const back = screen.getByRole('button', { name: 'Back' })
    expect(back).toBeEnabled()
    expect(screen.getByRole('button', { name: 'Forward' })).toBeDisabled()
    await user.hover(back)
    const tip = await screen.findByRole('tooltip', { name: /Back/ })
    const sideCarrier = tip.hasAttribute('data-side') ? tip : tip.parentElement
    expect(sideCarrier).toHaveAttribute('data-side', 'top')
    await user.click(back)
    expect(onBack).toHaveBeenCalledOnce()
  })

  it('follows the embedded page address and sends back to that page', async () => {
    const user = userEvent.setup()
    const invoke = vi.fn(async (command: string, args?: Record<string, unknown>) => {
      if (command === 'workspace_browser_navigate') {
        return { url: 'https://live.test/page', canGoBack: true, canGoForward: false, action: args?.action }
      }
      return undefined
    })
    ;(globalThis as { __TAURI_INTERNALS__?: { invoke: typeof invoke } }).__TAURI_INTERNALS__ = { invoke }
    runtime.declare('session-a', 'tab-a')
    runtime.ensure('session-a', 'https://example.com/docs')
    runtime.update('session-a', { ready: true })
    render(
      <SWRConfig value={{ provider: () => new Map() }}>
        <I18nextProvider i18n={i18n}>
          <SessionBrowserView
            sessionId="session-a"
            securityProfile={WebviewSecurityProfile.AgentBrowser}
            onNavigate={vi.fn()}
            target={{ id: 'session-a', label: 'Browser' }}
            isHostActive
          />
        </I18nextProvider>
      </SWRConfig>
    )
    const address = await screen.findByRole('combobox', { name: 'Web address' })
    await waitFor(() => expect(address).toHaveValue('live.test/page'))
    expect(runtime.get('session-a')?.sourceUrl).toBe('https://example.com/docs')
    expect(runtime.get('session-a')?.url).toBe('https://live.test/page')
    expect(invoke).toHaveBeenCalledWith('workspace_browser_navigate', { action: 'state' })
    expect(invoke).not.toHaveBeenCalledWith('launch_workspace_browser', expect.anything())
    await user.click(screen.getByRole('button', { name: 'Back' }))
    expect(invoke).toHaveBeenCalledWith('workspace_browser_navigate', { action: 'back' })
    expect(screen.getByRole('button', { name: 'Forward' })).toBeDisabled()
  })

  it('stops a loading page, clears an edit, and focuses the address from the keyboard', async () => {
    const user = userEvent.setup()
    const onStop = vi.fn()
    const onNavigate = vi.fn()
    render(
      <SWRConfig value={{ provider: () => new Map() }}>
        <I18nextProvider i18n={i18n}>
          <WebviewNavigation
            initialUrl="https://current.test/docs"
            webviewRef={{ current: null }}
            webviewRevision={0}
            historyEnabled
            isWebviewReady
            isHostActive
            loading
            onStop={onStop}
            onNavigate={onNavigate}
            target={{ id: 'test-browser', label: 'Browser' }}
          />
        </I18nextProvider>
      </SWRConfig>
    )
    expect(document.querySelector('[data-address-security="secure"]')).not.toBeNull()
    expect(screen.getByRole('progressbar', { name: 'Loading...' })).toBeInTheDocument()
    act(() => {
      screen
        .getByRole('button', { name: 'Back' })
        .dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }))
    })
    expect(onStop).toHaveBeenCalledOnce()
    await user.click(screen.getByRole('button', { name: 'Stop' }))
    expect(onStop).toHaveBeenCalledTimes(2)
    const address = screen.getByRole('combobox', { name: 'Web address' })
    act(() => {
      window.dispatchEvent(new KeyboardEvent('keydown', { key: 'l', metaKey: true, bubbles: true, cancelable: true }))
    })
    expect(address).toHaveFocus()
    await user.click(screen.getByRole('button', { name: 'Clear' }))
    expect(address).toHaveValue('')
    await user.keyboard('{Enter}')
    expect(onNavigate).not.toHaveBeenCalled()
  })
})
