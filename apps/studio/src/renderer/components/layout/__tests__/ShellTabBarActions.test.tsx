// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

const { cacheState, mocks, updateState } = vi.hoisted(() => ({
  cacheState: { sidebarWidth: 50 },
  mocks: {
    openSettingsTab: vi.fn(),
    openTab: vi.fn(),
    showDoctorPopup: vi.fn(),
    showSearchPopup: vi.fn(),
    showUpdatePopup: vi.fn()
  },
  updateState: {
    available: false,
    downloaded: false,
    info: null as { version: string } | null
  }
}))

vi.mock('@logger', () => ({
  loggerService: {
    withContext: () => ({
      error: vi.fn()
    })
  }
}))

vi.mock('@a3s-lab/ui', () => ({
  Button: ({
    children,
    type = 'button',
    ...props
  }: React.ComponentProps<'button'> & { variant?: string; size?: string }) => {
    const { variant, size, ...buttonProps } = props
    void variant
    void size

    return (
      <button data-slot="button" type={type} {...buttonProps}>
        {children}
      </button>
    )
  },
  Tooltip: ({ children, content }: { children: React.ReactNode; content?: React.ReactNode }) => (
    <div data-tooltip={typeof content === 'string' ? content : undefined}>{children}</div>
  ),
  Kbd: ({ children }: { children?: React.ReactNode }) => children
}))

vi.mock('@data/hooks/useCache', () => ({
  usePersistCache: () => [cacheState.sidebarWidth, vi.fn()]
}))

vi.mock('@renderer/hooks/useAppUpdateState', () => ({
  useAppUpdateState: () => ({ appUpdateState: updateState, updateAppUpdateState: vi.fn() })
}))

vi.mock('@renderer/services/mainWindowNavigation', () => ({
  openSettingsTab: mocks.openSettingsTab
}))

vi.mock('@renderer/components/GlobalSearch/GlobalSearchPopup', () => ({
  default: {
    show: mocks.showSearchPopup
  }
}))

vi.mock('@renderer/components/doctor', () => ({
  DoctorPopup: {
    show: mocks.showDoctorPopup
  }
}))

vi.mock('@renderer/components/UpdateDialogPopup', () => ({
  default: {
    show: mocks.showUpdatePopup
  }
}))

vi.mock('@renderer/components/command', () => ({
  CommandTooltip: ({ children }: { children: React.ReactNode }) => children
}))

vi.mock('@renderer/hooks/tab', () => ({
  useTabs: () => ({ openTab: mocks.openTab })
}))

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) =>
      ({
        'globalSearch.open': 'Open global search',
        'settings.about.updateAvailable': 'Found new version',
        'settings.doctor.entry.title': 'System diagnostics',
        'settings.title': 'Settings',
        'title.launchpad': 'Launchpad'
      })[key] ?? key
  })
}))

vi.mock('../../WindowControls', () => ({
  WindowControls: () => null
}))

import { ShellTabBarActions, SidebarShellActions } from '../ShellTabBarActions'

afterEach(() => {
  cleanup()
  vi.clearAllMocks()
  cacheState.sidebarWidth = 50
  updateState.available = false
  updateState.downloaded = false
  updateState.info = null
})

describe('ShellTabBarActions', () => {
  beforeEach(() => {
    Object.defineProperty(window, 'toast', {
      configurable: true,
      value: { error: vi.fn() }
    })
  })

  it('opens global search from the action area', async () => {
    const user = userEvent.setup()

    render(<ShellTabBarActions />)

    await user.click(screen.getByRole('button', { name: 'Open global search' }))

    expect(screen.getByRole('button', { name: 'Open global search' })).toHaveAttribute('data-slot', 'button')
    expect(screen.getByRole('button', { name: 'Open global search' })).toHaveClass(
      'text-muted-foreground',
      'dark:text-muted-foreground'
    )
    expect(mocks.showSearchPopup).toHaveBeenCalledTimes(1)
  })

  it('shows a ready update and opens its dialog directly', async () => {
    const user = userEvent.setup()
    updateState.available = true
    updateState.downloaded = true
    updateState.info = { version: '2.0.0' }

    render(<ShellTabBarActions />)

    const updateButton = screen.getByRole('button', { name: 'Found new version' })
    expect(updateButton.querySelector('svg')).toHaveClass('text-success')

    await user.click(updateButton)

    await waitFor(() => {
      expect(mocks.showUpdatePopup).toHaveBeenCalledWith({ releaseInfo: updateState.info })
    })
  })

  it('keeps the update action hidden until the update is ready to install', () => {
    updateState.available = true
    updateState.info = { version: '2.0.0' }

    render(<ShellTabBarActions />)

    expect(screen.queryByRole('button', { name: 'Found new version' })).not.toBeInTheDocument()
  })

  it('keeps the update action at the left of the action group', () => {
    cacheState.sidebarWidth = 0
    updateState.available = true
    updateState.downloaded = true
    updateState.info = { version: '2.0.0' }

    render(<ShellTabBarActions />)

    expect(screen.getAllByRole('button').map((button) => button.getAttribute('aria-label'))).toEqual([
      'Found new version',
      'System diagnostics',
      'Settings',
      'Open global search'
    ])
  })

  it('uses its natural width in the header flex layout with one right padding', () => {
    const { container } = render(<ShellTabBarActions />)
    const actionArea = container.firstElementChild

    expect(actionArea).toHaveClass('shrink-0')
    expect(actionArea).not.toHaveClass('absolute')
    expect(actionArea?.firstElementChild).toHaveClass('pr-2')
  })

  it('keeps theme and settings actions out of the tab bar while the sidebar is visible', () => {
    render(<ShellTabBarActions />)

    expect(screen.queryByRole('button', { name: 'Light' })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: /settings/i })).not.toBeInTheDocument()
  })

  it('opens settings from the tab bar when the sidebar is hidden', async () => {
    const user = userEvent.setup()
    cacheState.sidebarWidth = 0

    render(<ShellTabBarActions />)

    const settingsButton = screen.getByRole('button', { name: /settings/i })
    expect(settingsButton.parentElement).toHaveAttribute('data-tooltip', 'Settings')

    await user.click(settingsButton)

    expect(mocks.openSettingsTab).toHaveBeenCalledWith()
  })

  it('opens system diagnostics from the keyboard when the sidebar is hidden', async () => {
    const user = userEvent.setup()
    cacheState.sidebarWidth = 0

    render(<ShellTabBarActions />)

    await user.tab()
    expect(screen.getByRole('button', { name: 'System diagnostics' })).toHaveFocus()
    await user.keyboard('{Enter}')

    expect(mocks.showDoctorPopup).toHaveBeenCalledWith({ initialPanel: 'checks' })
  })

  it('opens the fullscreen applet board from the sidebar footer', async () => {
    const user = userEvent.setup()
    localStorage.setItem('a3s.org.center', 'org-1')
    localStorage.setItem('org.workbench.favorites.v1:org-1', JSON.stringify(['inbox']))

    render(<SidebarShellActions layout="icon" onSettingsClick={mocks.openSettingsTab} />)

    const button = screen.getByRole('button', { name: 'org.nav.workbench' })
    expect(button.querySelector('svg')).toHaveAttribute('stroke-width', '1.6')
    await user.click(button)

    const board = document.querySelector('[data-ui="workbench.applet.launcher"]') as HTMLElement
    expect(board).toHaveClass('workbench-launchpad')
    expect(board).toHaveAttribute('data-state', 'open')
    expect(board).toHaveClass('fixed', 'inset-0')
    expect(mocks.openTab).not.toHaveBeenCalled()
    fireEvent.click(board.querySelector('[data-applet="approval"] [data-pin]')!)
    expect(JSON.parse(localStorage.getItem('org.workbench.favorites.v1:org-1') ?? '[]')).toEqual(['inbox', 'approval'])
    localStorage.removeItem('a3s.org.center')
    localStorage.removeItem('org.workbench.favorites.v1:org-1')
  })

  it('does not show the launchpad rocket on the activity bar', () => {
    render(<SidebarShellActions layout="icon" onSettingsClick={mocks.openSettingsTab} />)

    expect(screen.queryByRole('button', { name: 'Launchpad' })).not.toBeInTheDocument()
    expect(document.querySelector('[data-launchpad-button]')).toBeNull()
  })

  it('shows the launchpad name in the expanded sidebar footer', () => {
    render(<SidebarShellActions layout="full" onSettingsClick={mocks.openSettingsTab} />)

    expect(screen.getByRole('button', { name: 'Launchpad' })).toHaveTextContent('Launchpad')
  })

  it('does not render the theme toggle in the sidebar footer action', () => {
    render(<SidebarShellActions layout="icon" onSettingsClick={mocks.openSettingsTab} />)

    expect(screen.queryByRole('button', { name: 'Light' })).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: /settings/i })).toHaveAttribute('data-slot', 'button')
    expect(screen.getByRole('button', { name: /settings/i }).parentElement).toHaveAttribute('data-tooltip', 'Settings')
    expect(screen.getByRole('button', { name: /settings/i })).toHaveClass(
      'text-muted-foreground',
      'dark:text-muted-foreground'
    )
    expect(screen.queryByRole('button', { name: 'Help & Feedback' })).not.toBeInTheDocument()
  })

  it('opens the settings tab from the sidebar footer action', async () => {
    const user = userEvent.setup()

    render(<SidebarShellActions layout="icon" onSettingsClick={mocks.openSettingsTab} />)

    await user.click(screen.getByRole('button', { name: /settings/i }))

    expect(mocks.openSettingsTab).toHaveBeenCalledTimes(1)
  })

  it('renders sidebar full footer actions with visible labels', () => {
    render(<SidebarShellActions layout="full" onSettingsClick={mocks.openSettingsTab} />)

    expect(screen.queryByRole('button', { name: 'Light' })).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: /settings/i })).toHaveAttribute('data-slot', 'button')
    expect(screen.getByRole('button', { name: /settings/i })).toHaveClass('justify-start', 'text-foreground')
    expect(screen.getByRole('button', { name: /settings/i })).not.toHaveClass('text-muted-foreground')
    expect(screen.getByRole('button', { name: /settings/i })).toHaveTextContent('Settings')
    expect(screen.queryByRole('button', { name: 'Help & Feedback' })).not.toBeInTheDocument()
  })
})
