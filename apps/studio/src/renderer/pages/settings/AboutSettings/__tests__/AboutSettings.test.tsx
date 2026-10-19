import '@testing-library/jest-dom/vitest'
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

const mocks = vi.hoisted(() => ({
  navigate: vi.fn(),
  request: vi.fn(),
  search: {} as Record<string, unknown>,
  showDoctor: vi.fn(),
  showReleases: vi.fn(),
  toastError: vi.fn(),
  toastInfo: vi.fn(),
  updateState: {
    available: false,
    checking: false,
    downloaded: false,
    downloading: false,
    downloadProgress: 0,
    info: null as { version?: string; releaseNotes?: string } | null
  }
}))

vi.mock('@renderer/components/doctor', () => ({
  DoctorPopup: { show: (...args: unknown[]) => mocks.showDoctor(...args) }
}))

vi.mock('@renderer/ipc', () => ({
  ipcApi: { request: mocks.request }
}))

vi.mock('@tanstack/react-router', () => ({
  useLocation: () => ({ pathname: '/settings/about' }),
  useNavigate: () => mocks.navigate,
  useSearch: () => mocks.search
}))

vi.mock('@renderer/hooks/useAppUpdateState', () => ({
  useAppUpdateState: () => ({
    appUpdateState: mocks.updateState,
    updateAppUpdateState: vi.fn()
  })
}))

vi.mock('@renderer/hooks/useMiniAppPopup', () => ({
  useMiniAppPopup: () => ({ openSmartMiniApp: vi.fn() })
}))

vi.mock('@renderer/hooks/useOpenReleaseNotes', () => ({
  useOpenReleaseNotes: () => mocks.showReleases
}))

vi.mock('@renderer/hooks/useTheme', () => ({
  useTheme: () => ({ theme: 'light' })
}))

vi.mock('@renderer/services/toast', () => ({
  toast: {
    info: (...args: unknown[]) => mocks.toastInfo(...args),
    error: (...args: unknown[]) => mocks.toastError(...args)
  }
}))

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    i18n: { language: 'zh-CN', resolvedLanguage: 'zh-CN' },
    t: (key: string) => key
  })
}))

import { AboutSettings, aboutChangelogBullet, aboutFeedFromRelease, releaseIsNewer, studioVersionLabel } from '..'

async function renderAboutSettings() {
  render(<AboutSettings />)
  await waitFor(() => expect(mocks.request).toHaveBeenCalledWith('app.get_info'))
  await waitFor(() => expect(mocks.request).toHaveBeenCalledWith('app.updater.release_notes.get'))
}

describe('studioVersionLabel', () => {
  it('uses the studio version without a second v prefix', () => {
    expect(studioVersionLabel('2.1.3')).toBe('2.1.3')
    expect(studioVersionLabel('v2.1.3')).toBe('2.1.3')
    expect(studioVersionLabel('  V2.1.3  ')).toBe('2.1.3')
    expect(studioVersionLabel('')).toBe('')
  })
})

describe('aboutChangelogBullet', () => {
  it('keeps a product-neutral bullet and drops notes that name another product', () => {
    expect(
      aboutChangelogBullet(
        '<!--LANG:en-->Cherry Studio 1\n- item<!--LANG:zh-CN-->Cherry Studio\n- 一项<!--LANG:END-->',
        'zh-CN'
      )
    ).toBe('')
    expect(
      aboutChangelogBullet('<!--LANG:en-->\n- Faster startup<!--LANG:zh-CN-->\n- 启动更快<!--LANG:END-->', 'zh-CN')
    ).toBe('启动更快')
  })

  it('treats a published release as newer only when its version is greater', () => {
    expect(releaseIsNewer('2.1.4', '2.1.3')).toBe(true)
    expect(releaseIsNewer('2.1.3', '2.1.3')).toBe(false)
    expect(releaseIsNewer('2.1.3', '2.1.3-rc.1')).toBe(true)
    expect(releaseIsNewer('2.1.3-rc.1', '2.1.3')).toBe(false)
    expect(releaseIsNewer('soon', '2.1.3')).toBeNull()
    expect(aboutFeedFromRelease('2.1.3', { version: '2.1.4', releaseNotes: '- 新版本' })).toEqual({
      kind: 'newer',
      version: '2.1.4',
      notes: '- 新版本'
    })
    expect(aboutFeedFromRelease('2.1.3', { version: '2.1.3', releaseNotes: '- 启动更快' })).toEqual({
      kind: 'current',
      notes: '- 启动更快'
    })
    expect(aboutFeedFromRelease('2.1.3', null)).toEqual({ kind: 'unpublished' })
  })
})

describe('AboutSettings', () => {
  afterEach(() => {
    vi.unstubAllGlobals()
    vi.restoreAllMocks()
    delete (globalThis as { __A3S_STUDIO_RUNTIME__?: unknown }).__A3S_STUDIO_RUNTIME__
    delete (globalThis as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__
  })

  beforeEach(() => {
    vi.clearAllMocks()
    mocks.search = {}
    mocks.updateState.available = false
    mocks.updateState.checking = false
    mocks.updateState.downloaded = false
    mocks.updateState.downloading = false
    mocks.updateState.downloadProgress = 0
    mocks.updateState.info = null
    delete document.documentElement.dataset.a3sProduct
    document.documentElement.dataset.a3sCompanyZh = '通友科技'
    document.documentElement.dataset.a3sCompanyEn = 'Toyou AI Labs'
    mocks.request.mockImplementation(async (route: string) => {
      if (route === 'app.get_info') return { isPortable: false, version: '2.1.3' }
      if (route === 'app.updater.release_notes.get') {
        return [{ releaseNotes: '- 启动更快', version: '2.1.3' }]
      }
      return undefined
    })
  })

  it('lists the about card links', async () => {
    await renderAboutSettings()
    expect(await screen.findByText('settings.about.page.up_to_date')).toBeInTheDocument()

    expect(screen.getByRole('heading', { name: 'Cherry Studio' }).closest('[data-theme-mode]')).toHaveClass('pt-16')
    expect(screen.getByText('settings.about.page.up_to_date')).toBeInTheDocument()
    expect(screen.queryByText('启动更快')).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'settings.about.releases.title' })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'settings.about.feedback.title' })).not.toBeInTheDocument()
    expect(screen.getByText('settings.about.page.version')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'settings.about.checkUpdate.label' })).toHaveClass(
      'h-[30px]',
      'border-[#e5e5e5]',
      'text-[#171717]'
    )
    expect(screen.getByRole('button', { name: 'settings.about.page.terms' })).toHaveClass('h-9', 'text-[13px]')
    expect(document.querySelector('[data-ui="settings.about"]')).toHaveClass('rounded-[14px]', 'border-[#ececec]')
    expect(screen.getByRole('button', { name: 'settings.about.page.privacy' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'settings.about.page.copyright' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'settings.about.page.support' })).toBeInTheDocument()
  })

  it('shows no copyright footer even when the tenant has a company name', async () => {
    document.documentElement.dataset.a3sCompanyZh = '通友科技'
    document.documentElement.dataset.a3sCompanyEn = 'Toyou AI Labs'
    await renderAboutSettings()
    expect(screen.queryByText(/©|版权所有|settings\.about\.page\.credits/)).not.toBeInTheDocument()
    expect(screen.queryByText(/通友科技|Toyou AI Labs/)).not.toBeInTheDocument()
  })

  it('draws the ToWork wordmark when the tenant product is ToWork', async () => {
    document.documentElement.dataset.a3sProduct = 'ToWork'
    await renderAboutSettings()
    expect(screen.getByRole('heading', { name: 'ToWork' }).querySelector('svg')).toBeInTheDocument()
    expect(screen.queryByText('ToWork')).not.toBeInTheDocument()
  })

  it('restarts to finish a downloaded update and otherwise checks for one', async () => {
    const user = userEvent.setup()
    mocks.updateState.downloaded = true
    await renderAboutSettings()

    await user.click(screen.getByRole('button', { name: 'settings.about.page.restart' }))
    expect(mocks.request).toHaveBeenCalledWith('app.updater.quit_and_install')

    mocks.updateState.downloaded = false
    render(<AboutSettings />)
    const check = await screen.findByRole('button', { name: 'settings.about.checkUpdate.label' })
    await waitFor(() => expect(check).toBeEnabled())
    await user.click(check)
    expect(mocks.request).toHaveBeenCalledWith('app.updater.check_for_update')
  })

  it('shows a determinate progress bar while an update is downloading', async () => {
    mocks.updateState.downloading = true
    mocks.updateState.downloadProgress = 42
    await renderAboutSettings()

    const bar = screen.getByRole('progressbar', { name: 'settings.about.downloading' })
    expect(bar).toHaveAttribute('aria-valuenow', '42')
    expect(bar.querySelector('div')).toHaveStyle({ width: '42%' })
    expect(screen.getByText('42%')).toBeInTheDocument()
  })

  it('toasts when a notice is unpublished', async () => {
    const user = userEvent.setup()
    delete (globalThis as { __A3S_STUDIO_RUNTIME__?: unknown }).__A3S_STUDIO_RUNTIME__
    await renderAboutSettings()

    await user.click(screen.getByRole('button', { name: 'settings.about.page.terms' }))
    await waitFor(() => expect(mocks.toastInfo).toHaveBeenCalledWith('settings.about.page.link_unavailable'))

    await user.click(screen.getByRole('button', { name: 'settings.about.page.support' }))
    await waitFor(() => expect(mocks.toastInfo).toHaveBeenCalledTimes(2))
  })

  it('offers the published update and does not call the build current before the feed answers', async () => {
    const user = userEvent.setup()
    mocks.request.mockImplementation(async (route: string) => {
      if (route === 'app.get_info') return { isPortable: false, version: '2.1.3' }
      if (route === 'app.updater.release_notes.get') {
        return [{ releaseNotes: '- 新版本说明', version: '2.1.4' }]
      }
      return undefined
    })
    render(<AboutSettings />)

    expect(screen.queryByText('settings.about.page.up_to_date')).not.toBeInTheDocument()
    const update = await screen.findByRole('button', { name: 'settings.about.checkUpdate.available' })
    expect(screen.getByText('settings.about.updateAvailable')).toBeInTheDocument()
    expect(screen.queryByText('新版本说明')).not.toBeInTheDocument()
    expect(mocks.request).toHaveBeenCalledWith('app.updater.release_notes.get')
    expect(mocks.request).not.toHaveBeenCalledWith('app.updater.check_for_update')

    await user.click(update)
    expect(mocks.request).toHaveBeenCalledWith('app.updater.check_for_update')
  })

  it('says there is no published update when the release feed is empty', async () => {
    mocks.request.mockImplementation(async (route: string) => {
      if (route === 'app.get_info') return { isPortable: false, version: '2.1.3' }
      if (route === 'app.updater.release_notes.get') return null
      return undefined
    })
    render(<AboutSettings />)

    expect(await screen.findByText('settings.about.updateNotPublished')).toBeInTheDocument()
    expect(screen.queryByText('settings.about.page.up_to_date')).not.toBeInTheDocument()
  })

  it('reports a failed release read without a toast', async () => {
    mocks.request.mockImplementation(async (route: string) => {
      if (route === 'app.get_info') return { isPortable: false, version: '2.1.3' }
      if (route === 'app.updater.release_notes.get') throw new Error('offline')
      return undefined
    })
    render(<AboutSettings />)

    expect(await screen.findByText('settings.about.updateError')).toBeInTheDocument()
    expect(mocks.toastError).not.toHaveBeenCalled()
    expect(screen.queryByText('settings.about.page.up_to_date')).not.toBeInTheDocument()
  })

  it('opens a published notice in its own window', async () => {
    const user = userEvent.setup()
    const invoke = vi.fn(async () => undefined)
    ;(globalThis as { __TAURI_INTERNALS__?: { invoke: typeof invoke } }).__TAURI_INTERNALS__ = { invoke }
    Object.defineProperty(globalThis, '__A3S_STUDIO_RUNTIME__', {
      configurable: true,
      value: { apiBaseUrl: 'http://127.0.0.1:9', token: 'cd'.repeat(32) }
    })
    const fetchMock = vi.fn(async () => ({
      ok: true,
      json: async () => ({ code: 0, data: { url: 'http://127.0.0.1:3000/v1/api/legal/terms' } })
    }))
    vi.stubGlobal('fetch', fetchMock)
    await renderAboutSettings()

    await user.click(screen.getByRole('button', { name: 'settings.about.page.terms' }))

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith('open_notice_window', {
        slug: 'terms',
        title: 'settings.about.page.terms',
        url: 'http://127.0.0.1:3000/v1/api/legal/terms'
      })
    )
    expect(invoke).not.toHaveBeenCalledWith('launch_notice_browser', expect.anything())
    expect(screen.queryByRole('heading', { name: 'settings.about.page.terms' })).not.toBeInTheDocument()
    expect(document.querySelector('[data-notice-page]')).not.toBeInTheDocument()
    expect(fetchMock).toHaveBeenCalledWith('http://127.0.0.1:9/api/publications/terms', {
      headers: { authorization: `Bearer ${'cd'.repeat(32)}` }
    })
    expect(mocks.request).not.toHaveBeenCalledWith('system.shell.open_external_website', expect.anything())
    expect(mocks.toastInfo).not.toHaveBeenCalled()
    expect(mocks.toastError).not.toHaveBeenCalled()
  })

  it('toasts when the notice window does not open', async () => {
    const user = userEvent.setup()
    const invoke = vi.fn(async () => {
      throw new Error('closed')
    })
    ;(globalThis as { __TAURI_INTERNALS__?: { invoke: typeof invoke } }).__TAURI_INTERNALS__ = { invoke }
    Object.defineProperty(globalThis, '__A3S_STUDIO_RUNTIME__', {
      configurable: true,
      value: { apiBaseUrl: 'http://127.0.0.1:9', token: 'cd'.repeat(32) }
    })
    vi.stubGlobal(
      'fetch',
      vi.fn(async () => ({
        ok: true,
        json: async () => ({ code: 0, data: { url: 'http://127.0.0.1:3000/v1/api/legal/privacy' } })
      }))
    )
    await renderAboutSettings()

    await user.click(screen.getByRole('button', { name: 'settings.about.page.privacy' }))

    await waitFor(() => expect(mocks.toastError).toHaveBeenCalledWith('settings.about.page.open_failed'))
    expect(screen.queryByRole('heading', { name: 'settings.about.page.privacy' })).not.toBeInTheDocument()
    expect(document.querySelector('[data-notice-page]')).not.toBeInTheDocument()
  })

  it('indexes the visible about rows', async () => {
    const { entries } = await import('../about.search')
    expect(entries.map((entry) => entry.anchorId)).toEqual(['update', 'terms', 'privacy', 'copyright', 'support'])
    expect(entries.find((entry) => entry.anchorId === 'debug-tools')).toBeUndefined()
  })

  it('opens an externally requested Doctor panel and consumes only that query', async () => {
    mocks.search = { doctor: 'report', focusId: 'support' }

    await renderAboutSettings()

    await waitFor(() => expect(mocks.showDoctor).toHaveBeenCalledWith({ initialPanel: 'report' }))
    expect(mocks.navigate).toHaveBeenCalledWith({
      to: '/settings/about',
      search: expect.any(Function),
      replace: true
    })

    const updateSearch = mocks.navigate.mock.calls[0][0].search as (
      previous: Record<string, unknown>
    ) => Record<string, unknown>
    expect(updateSearch({ doctor: 'report', focusId: 'support' })).toEqual({ focusId: 'support' })
  })
})
