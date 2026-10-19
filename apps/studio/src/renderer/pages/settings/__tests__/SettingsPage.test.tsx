import { MockUsePreferenceUtils } from '@test-mocks/renderer/usePreference'
import { fireEvent, render, screen } from '@testing-library/react'
import type { ReactNode } from 'react'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import zhCN from '@renderer/i18n/locales/zh-cn.json'
import { ipcApi } from '@renderer/ipc'

import { SettingsLeaveContext } from '../settingsLeave'
import SettingsPage from '../SettingsPage'

const { isMacTransparentWindowMock, navigateMock } = vi.hoisted(() => ({
  isMacTransparentWindowMock: vi.fn(),
  navigateMock: vi.fn()
}))

vi.mock('@renderer/ipc', () => ({
  ipcApi: { request: vi.fn().mockResolvedValue(undefined) },
  useIpcOn: vi.fn()
}))

vi.mock('@a3s-lab/ui', () => ({
  MenuDivider: () => <hr data-testid="menu-divider" />,
  MenuItem: ({
    icon,
    label,
    className,
    onClick,
    active
  }: {
    icon?: ReactNode
    label: string
    className?: string
    onClick?: () => void
    active?: boolean
  }) => (
    <button
      type="button"
      data-testid="menu-item"
      data-slot="menu-item"
      data-active={active ? 'true' : undefined}
      className={className}
      onClick={onClick}>
      {icon}
      {label}
    </button>
  ),
  MenuList: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  PageHeader: ({ title, action }: { title: string; action?: ReactNode }) => (
    <header>
      {title}
      {action}
    </header>
  ),
  SearchInput: (props: {
    value: string
    placeholder?: string
    containerClassName?: string
    onChange: (e: { target: { value: string } }) => void
    onKeyDown?: (e: { key: string; preventDefault: () => void }) => void
  }) => (
    <input
      data-testid="settings-search-input"
      className={props.containerClassName}
      value={props.value}
      placeholder={props.placeholder}
      onChange={props.onChange}
      onKeyDown={props.onKeyDown}
    />
  )
}))

vi.mock('@renderer/components/Scrollbar', () => ({
  default: ({ children }: { children: ReactNode }) => <div>{children}</div>
}))

vi.mock('@renderer/hooks/useMacTransparentWindow', () => ({
  default: () => isMacTransparentWindowMock()
}))

vi.mock('@tanstack/react-router', () => ({
  Outlet: () => (
    <>
      <a href="http://127.0.0.1:8080" target="_blank" rel="noreferrer">
        <span>Provider website</span>
      </a>
      <a href="#provider">Internal settings</a>
    </>
  ),
  useLocation: () => ({ pathname: '/settings/general', hash: '' }),
  useNavigate: () => navigateMock,
  useRouter: () => ({ history: { canGoBack: () => false, back: vi.fn() } }),
  useSearch: () => ({})
}))

vi.mock('react-i18next', () => ({
  initReactI18next: { type: '3rdParty', init: vi.fn() },
  useTranslation: () => ({
    t: (key: string) =>
      ({
        'agent.settings.toolsMcp.mcp.tab': 'MCP',
        'settings.aligned.about.title': '关于',
        'settings.appearance.title': '外观',
        'settings.backToApp': '返回应用',
        'settings.browser.title': '浏览器',
        'settings.general.common.title': zhCN['settings.general.common.title'],
        'settings.search.cancel': '取消搜索',
        'settings.search.placeholder': '搜索设置...',
        'agent.channels.discord.title': 'Discord',
        'agent.channels.feishu.title': '飞书',
        'agent.channels.qq.title': 'QQ',
        'agent.channels.slack.title': 'Slack',
        'agent.channels.telegram.title': 'Telegram',
        'agent.channels.wechat.title': '微信',
        'settings.menuGroups.archive': '归档',
        'settings.menuGroups.coding': '编码',
        'settings.menuGroups.connections': '连接',
        'settings.menuGroups.personal': '偏好',
        'settings.menuGroups.system': '系统',
        'settings.nav.archivedTasks': '归档',
        'settings.nav.codeReview': '代码审查',
        'settings.nav.language': '语言',
        'settings.nav.personalization': '个性化',
        'settings.nav.worktrees': '工作树',
        'settings.shortcuts.title': '快捷键',
        'settings.dependencies.title': '环境依赖',
        'settings.shortcuts.categories.general': '全局与窗口',
        'settings.shortcuts.categories.chat': '消息交互',
        'settings.shortcuts.categories.topic': '会话与对话',
        'settings.shortcuts.categories.assistant': 'AI 助手工具'
      })[key] ??
      (zhCN as Record<string, string>)[key] ??
      key
  })
}))

describe('SettingsPage', () => {
  beforeEach(() => {
    MockUsePreferenceUtils.resetMocks()
    isMacTransparentWindowMock.mockReturnValue(false)
    navigateMock.mockReset()
    vi.mocked(ipcApi.request).mockClear()
  })

  it.each(['click', 'auxclick'])(
    'opens settings websites externally on %s even when internal browsing is enabled',
    (type) => {
      MockUsePreferenceUtils.setPreferenceValue('app.browser.open_links_in_browser', true)
      render(<SettingsPage />)
      const event = new MouseEvent(type, { bubbles: true, cancelable: true, button: type === 'auxclick' ? 1 : 0 })
      fireEvent(screen.getByText('Provider website'), event)
      expect(event.defaultPrevented).toBe(true)
      expect(ipcApi.request).toHaveBeenCalledWith('system.shell.open_external_website', 'http://127.0.0.1:8080/')
    }
  )

  it('leaves internal settings navigation and right-click menus alone', () => {
    render(<SettingsPage />)
    const internalClick = new MouseEvent('click', { bubbles: true, cancelable: true })
    fireEvent(screen.getByText('Internal settings'), internalClick)
    const rightClick = new MouseEvent('auxclick', { bubbles: true, cancelable: true, button: 2 })
    fireEvent(screen.getByText('Provider website'), rightClick)
    expect(internalClick.defaultPrevented).toBe(false)
    expect(rightClick.defaultPrevented).toBe(false)
    expect(ipcApi.request).not.toHaveBeenCalled()
  })

  it('puts a search icon to the right of the back button and opens the field from it', () => {
    render(<SettingsPage />)
    const back = screen.getByRole('button', { name: '返回应用' })
    const search = screen.getByRole('button', { name: '搜索设置...' })
    expect(search.parentElement).toBe(back.parentElement)
    expect(screen.queryByTestId('settings-search-input')).not.toBeInTheDocument()
    expect(screen.queryByText('title.settings')).not.toBeInTheDocument()
    expect(document.querySelector('[data-ui="settings.section-title"]')).toHaveTextContent('通用')

    expect(search).toHaveClass('size-9')

    fireEvent.click(search)
    const field = screen.getByTestId('settings-search-input')
    const cancel = screen.getByRole('button', { name: '取消搜索' })
    expect(field).toHaveClass('h-9')
    expect(cancel).toHaveClass('size-9')
    expect(screen.queryByRole('button', { name: '搜索设置...' })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: '返回应用' })).not.toBeInTheDocument()

    fireEvent.click(cancel)
    expect(screen.queryByTestId('settings-search-input')).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: '返回应用' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: '搜索设置...' })).toBeInTheDocument()
  })

  it('lists the reference menu in group order', () => {
    const { container } = render(<SettingsPage />)

    expect(container.querySelector('[data-ui="settings.view"]')).toBeInTheDocument()
    expect(container.querySelector('[data-ui="settings.navigation"]')).toBeInTheDocument()
    expect(container.querySelector('[data-ui="settings.content"]')).toBeInTheDocument()
    expect(screen.queryByTestId('menu-divider')).not.toBeInTheDocument()

    const labels = ['通用', '外观', '快捷键', '用量', '渠道', '数据', '执行轨迹', '环境依赖', '归档', '关于']
    const items = labels.map((name) => screen.getByRole('button', { name }))
    expect(screen.getAllByTestId('menu-item')).toEqual(items)
    expect(screen.getByRole('button', { name: '环境依赖' })).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: '模型服务' })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: '语言' })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: '个性化' })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: '浏览器' })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: '代码审查' })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: '工作树' })).not.toBeInTheDocument()
    expect(items[0].querySelector('svg')).toHaveClass('lucide-app-window')
    expect(items[1].querySelector('svg')).toHaveClass('lucide-contrast')
    expect(items[2].querySelector('svg')).toHaveClass('lucide-keyboard')
    expect(items[3].querySelector('svg')).toHaveClass('lucide-activity')
    expect(items[4].querySelector('svg')).toHaveClass('lucide-radio')
    expect(items[5].querySelector('svg')).toHaveClass('lucide-hard-drive')
    expect(items[6].querySelector('svg')).toHaveClass('lucide-route')
    expect(items[7].querySelector('svg')).toHaveClass('lucide-terminal')
    expect(items[8].querySelector('svg')).toHaveClass('lucide-archive')
    expect(items[9].querySelector('svg')).toHaveClass('lucide-info')
    expect(screen.queryByRole('button', { name: '自主进化' })).not.toBeInTheDocument()
    for (const item of items) expect(item).not.toHaveClass('mt-3')
    expect(screen.queryByRole('button', { name: '插件' })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: '技能' })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'MCP' })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'Agents' })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: '用量与模型' })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: '连接' })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: '账户' })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: '声音' })).not.toBeInTheDocument()
    expect(screen.queryByText('管理')).not.toBeInTheDocument()
    expect(screen.queryByText('编码')).not.toBeInTheDocument()
    expect(screen.getByText('偏好').nextElementSibling).toBe(items[0])
    expect(screen.getByText('连接').nextElementSibling).toBe(items[4])
    expect(screen.getByText('开发者').nextElementSibling).toBe(items[6])
    expect(screen.getByText('系统').nextElementSibling).toBe(items[8])
    expect(screen.queryByRole('button', { name: '已归档任务' })).not.toBeInTheDocument()

    fireEvent.click(items[0])
    expect(navigateMock).toHaveBeenCalledWith({ to: '/settings/general', search: {} })
    fireEvent.click(items[1])
    expect(navigateMock).toHaveBeenCalledWith({ to: '/settings/appearance', search: {} })
    fireEvent.click(items[2])
    expect(navigateMock).toHaveBeenCalledWith({ to: '/settings/shortcut', search: {} })
    fireEvent.click(items[3])
    expect(navigateMock).toHaveBeenCalledWith({ to: '/settings/usage', search: {} })
    fireEvent.click(items[4])
    expect(navigateMock).toHaveBeenCalledWith({ to: '/settings/channels', search: {} })
    fireEvent.click(items[5])
    expect(navigateMock).toHaveBeenCalledWith({ to: '/settings/data', search: {} })
    fireEvent.click(items[6])
    expect(navigateMock).toHaveBeenCalledWith({ to: '/settings/traces', search: {} })
    fireEvent.click(items[7])
    expect(navigateMock).toHaveBeenCalledWith({ to: '/settings/dependencies', search: {} })
    fireEvent.click(items[8])
    expect(navigateMock).toHaveBeenCalledWith({ to: '/settings/archive', search: {} })
    fireEvent.click(items[9])
    expect(navigateMock).toHaveBeenCalledWith({ to: '/settings/about', search: {} })
  })

  it('searches, moves between sections, and leaves from the keyboard', () => {
    const leave = vi.fn()
    render(
      <SettingsLeaveContext value={leave}>
        <SettingsPage />
      </SettingsLeaveContext>
    )

    const items = screen.getAllByTestId('menu-item')
    fireEvent.keyDown(items[0], { key: 'ArrowDown' })
    expect(navigateMock).toHaveBeenCalledWith({ to: '/settings/appearance', search: {} })

    fireEvent.keyDown(window, { key: 'f', ctrlKey: true })
    const field = screen.getByTestId('settings-search-input')
    fireEvent.keyDown(field, { key: 'Escape' })
    expect(leave).not.toHaveBeenCalled()
    expect(screen.queryByTestId('settings-search-input')).not.toBeInTheDocument()

    fireEvent.keyDown(window, { key: 'Escape' })
    expect(leave).toHaveBeenCalledOnce()
  })
})
