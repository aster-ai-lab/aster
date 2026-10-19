// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { act, fireEvent, render, screen, within } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import { PERSONAL_CENTER, writeCenterId } from '@renderer/data/memberOrgs'
import { createSidebarShortcutId, type SidebarShortcutItem } from '@shared/data/preference/preferenceTypes'

import { createSidebarShortcutTarget } from '../../../utils/sidebar'

const mocks = vi.hoisted(() => ({
  activate: vi.fn(),
  registryResolve: vi.fn(),
  resolutions: [] as any[],
  shortcuts: [] as any[]
}))

vi.mock('@data/hooks/useCache', () => ({ usePersistCache: () => [170, vi.fn()] }))
vi.mock('@data/hooks/usePreference', () => ({ usePreference: () => ['User', vi.fn()] }))
vi.mock('@renderer/hooks/tab', () => ({ useTabs: () => ({ activeTab: { url: '/app/chat' } }) }))
vi.mock('@renderer/hooks/useAvatar', () => ({ default: () => null }))
vi.mock('@renderer/hooks/useSidebarShortcuts', () => ({
  useSidebarShortcuts: () => ({ shortcuts: mocks.shortcuts })
}))
vi.mock('@renderer/services/mainWindowNavigation', () => ({ openSettingsTab: vi.fn() }))
vi.mock('../sidebarShortcuts', () => ({
  useSidebarNavigationSnapshot: () => ({ url: '/' }),
  useResolvedSidebarShortcuts: () => mocks.resolutions,
  useSidebarShortcutActivation: () => mocks.activate,
  useSidebarShortcutRegistry: () => ({ resolve: mocks.registryResolve })
}))
vi.mock('../../layout/ShellTabBarActions', () => ({ SidebarShellActions: () => null }))
vi.mock('../../Sidebar', () => ({
  getSidebarDisplayWidth: (width: number) => width,
  getSidebarLayout: () => 'full',
  normalizeSidebarWidth: (width: number) => width,
  UserAvatar: () => <span />,
  Sidebar: ({
    entries,
    onHeaderClick,
    headerExpanded
  }: {
    entries: Array<{
      key: string
      label: string
      disabled?: boolean
      onOpen: () => void
    }>
    onHeaderClick?: (event: { currentTarget: HTMLElement }) => void
    headerExpanded?: boolean
  }) => (
    <div>
      <button
        type="button"
        aria-label="用户"
        aria-expanded={headerExpanded || undefined}
        onClick={(event) => onHeaderClick?.(event)}>
        avatar
      </button>
      <ol aria-label="shortcuts">
        {entries.map((entry) => (
          <li key={entry.key} aria-label={entry.label}>
            <button
              type="button"
              aria-disabled={entry.disabled || undefined}
              onClick={() => !entry.disabled && entry.onOpen()}>
              {entry.label}
            </button>
          </li>
        ))}
      </ol>
    </div>
  )
}))
vi.mock('react-i18next', () => ({ useTranslation: () => ({ t: (key: string) => key }) }))

import Sidebar from '../Sidebar'

function shortcut(providerId: string, resourceId: string, fallbackLabel?: string): SidebarShortcutItem {
  const target = createSidebarShortcutTarget(providerId, resourceId)
  return { type: 'shortcut', id: createSidebarShortcutId(target), target, fallbackLabel }
}

function renderedShortcutLabels(): Array<string | null> {
  return within(screen.getByRole('list', { name: 'shortcuts' }))
    .getAllByRole('listitem')
    .map((item) => item.getAttribute('aria-label'))
}

describe('app Sidebar shortcuts', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    localStorage.clear()
    mocks.shortcuts = []
    mocks.resolutions = []
    mocks.registryResolve.mockReturnValue({ activate: mocks.activate })
  })

  it('keeps a missing resource in place and disables activation', () => {
    const missing = shortcut('core.knowledge-base', 'missing', 'Lost Knowledge Base')
    mocks.shortcuts = [missing]
    mocks.resolutions = [{ status: 'missing', shortcut: missing }]

    render(<Sidebar />)

    expect(screen.getByRole('button', { name: 'Lost Knowledge Base' })).toHaveAttribute('aria-disabled', 'true')
    fireEvent.click(screen.getByRole('button', { name: 'Lost Knowledge Base' }))
    expect(mocks.activate).not.toHaveBeenCalled()
  })

  it('keeps messages in the personal center and hides org until a team is selected', () => {
    const agents = shortcut('core.app', 'agents', 'Agents')
    const messages = shortcut('core.app', 'messages', 'Messages')
    const org = shortcut('core.app', 'org', 'Org')
    mocks.shortcuts = [agents, messages, org]
    mocks.resolutions = mocks.shortcuts.map((item) => ({ status: 'missing', shortcut: item }))

    render(<Sidebar />)
    expect(renderedShortcutLabels()).toEqual(['Agents', 'Messages'])

    act(() => writeCenterId('team-1'))
    expect(renderedShortcutLabels()).toEqual(['Agents', 'Messages', 'Org'])

    act(() => writeCenterId(PERSONAL_CENTER))
    expect(renderedShortcutLabels()).toEqual(['Agents', 'Messages'])
  })

  it('opens the team and personal center switcher from the activity avatar', async () => {
    render(<Sidebar />)

    fireEvent.click(screen.getByRole('button', { name: '用户' }))

    expect(await screen.findByRole('dialog', { name: 'sidebar.center.switch' })).toBeInTheDocument()
    expect(screen.getByRole('tab', { name: 'sidebar.center.teams' })).toBeInTheDocument()
    expect(screen.getByRole('tab', { name: 'sidebar.center.personal' })).toBeInTheDocument()
  })
})
