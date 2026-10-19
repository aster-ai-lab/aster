import { MockUsePreferenceUtils } from '@test-mocks/renderer/usePreference'
import { fireEvent, render, screen } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import { popup } from '@renderer/services/popup'
import { toast } from '@renderer/services/toast'
import { type MenuPresentationMode, ThemeMode } from '@shared/data/preference/preferenceTypes'

import AppearanceSettings, { confirmMenuPresentationModeChange } from '../AppearanceSettings'

const t = (key: string) => key

const i18nMock = vi.hoisted(() => ({
  changeLanguage: vi.fn(),
  language: 'zh-CN',
  resolvedLanguage: 'zh-CN'
}))

vi.mock('@renderer/i18n/resolver', () => ({
  default: i18nMock
}))

const mocks = vi.hoisted(() => ({ request: vi.fn() }))
const themeMocks = vi.hoisted(() => ({
  setTheme: vi.fn(),
  theme: 'light' as 'light' | 'dark' | 'system',
  settedTheme: 'light' as 'light' | 'dark' | 'system'
}))
vi.mock('@renderer/ipc', () => ({ ipcApi: { request: mocks.request } }))

vi.mock('@a3s-lab/ui', async () => {
  const React = await import('react')
  const SelectContext = React.createContext<{ value?: string; onValueChange?: (value: string) => void }>({
    value: undefined,
    onValueChange: undefined
  })
  const passthrough =
    (tag: string) =>
    ({ children, ...props }: { children?: React.ReactNode }) =>
      React.createElement(tag, props, children)

  const Button = ({ children, onPress, ...props }: any) =>
    React.createElement('button', { ...props, onClick: onPress ?? props.onClick }, children)

  const PopoverContext = React.createContext({
    open: false,
    onOpenChange: undefined as undefined | ((open: boolean) => void)
  })

  return {
    Button,
    CodeEditor: ({ value, ...props }: any) =>
      React.createElement('textarea', { ...props, value: value ?? '', readOnly: true }),
    Combobox: ({ options = [], popoverClassName, renderOption, value, ...props }: any) => {
      const cleanProps = { ...props }
      delete cleanProps.emptyText
      delete cleanProps.searchPlacement
      delete cleanProps.triggerStyle

      return React.createElement(
        'div',
        { 'data-popover-class-name': popoverClassName },
        React.createElement(
          'select',
          { ...cleanProps, value: value ?? '', readOnly: true },
          options.map((option: any) =>
            React.createElement('option', { key: option.value, value: option.value }, option.label)
          )
        ),
        renderOption
          ? React.createElement(
              'div',
              { 'data-testid': 'combobox-options' },
              options.map((option: any) => React.createElement('div', { key: option.value }, renderOption(option)))
            )
          : null
      )
    },
    CustomTag: passthrough('span'),
    Flex: passthrough('div'),
    InfoTooltip: ({ children }: { children?: React.ReactNode }) => React.createElement(React.Fragment, null, children),
    Input: (props: any) => React.createElement('input', props),
    MenuItem: ({ active, icon, label, onClick, ...props }: any) => {
      const cleanProps = { ...props }
      delete cleanProps.labelClassName

      return React.createElement(
        'button',
        { ...cleanProps, 'aria-pressed': active, onClick, type: 'button' },
        icon,
        label
      )
    },
    MenuList: passthrough('div'),
    PageHeader: ({ title }: { title: string }) => React.createElement('h1', null, title),
    Popover: ({ children, open = false, onOpenChange }: any) =>
      React.createElement(PopoverContext.Provider, { value: { open, onOpenChange } }, children),
    PopoverContent: ({ children }: { children?: React.ReactNode }) => React.createElement('div', null, children),
    PopoverTrigger: ({ children, asChild }: any) =>
      asChild && React.isValidElement(children) ? children : React.createElement('div', null, children),
    RowFlex: passthrough('div'),
    // Mirrors the real control's radiogroup/radio semantics so tests address it the way
    // assistive technology does, rather than through whatever DOM the mock happens to emit.
    SegmentedControl: ({ options = [], value, onValueChange, ...props }: any) =>
      React.createElement(
        'div',
        { 'aria-label': props['aria-label'], role: 'radiogroup' },
        options.map((option: any) =>
          React.createElement(
            'button',
            {
              'aria-checked': value === option.value,
              key: option.value,
              onClick: () => onValueChange?.(option.value),
              role: 'radio',
              type: 'button'
            },
            option.label
          )
        )
      ),
    Select: ({ children, onValueChange, value }: any) =>
      React.createElement(SelectContext.Provider, { value: { value, onValueChange } }, children),
    SelectContent: ({ children, portalContainer: _portalContainer, align: _align, ...props }: any) =>
      React.createElement('div', props, children),
    SelectItem: ({ children, value, ...props }: any) => {
      const current = React.use(SelectContext)
      return React.createElement(
        'button',
        {
          ...props,
          'aria-selected': current.value === value,
          onClick: () => current.onValueChange?.(value),
          role: 'option',
          type: 'button'
        },
        children
      )
    },
    SelectTrigger: ({ children, size, ...props }: any) =>
      React.createElement('button', { ...props, 'data-size': size, role: 'combobox', type: 'button' }, children),
    SelectValue: () => {
      const current = React.use(SelectContext)
      return React.createElement('span', null, current.value)
    },
    Switch: ({ checked, onCheckedChange, ...props }: any) =>
      React.createElement('input', {
        ...props,
        checked: Boolean(checked),
        onChange: (event: React.ChangeEvent<HTMLInputElement>) => onCheckedChange?.(event.target.checked),
        type: 'checkbox'
      }),
    Tooltip: ({ children, className, classNames, content, title }: any) =>
      React.createElement(
        'div',
        {
          className: [className, classNames?.placeholder].filter(Boolean).join(' ') || undefined,
          ...(content || title ? { 'data-title': content || title } : {})
        },
        children
      )
  }
})

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t
  })
}))

vi.mock('@renderer/hooks/useTheme', () => ({
  useTheme: () => ({
    settedTheme: themeMocks.settedTheme,
    setTheme: themeMocks.setTheme,
    theme: themeMocks.theme
  })
}))

vi.mock('@renderer/hooks/useCodeStyle', () => ({
  useCodeStyle: () => ({
    activeCmTheme: 'light'
  }),
  useCmTheme: () => 'light'
}))

vi.mock('@renderer/hooks/useUserTheme', () => ({
  default: () => ({
    setUserTheme: vi.fn(),
    userTheme: { colorPrimary: '#1677ff', userCodeFontFamily: '', userFontFamily: '' }
  })
}))

vi.mock('@renderer/hooks/useTimer', () => ({
  useTimer: () => ({
    setTimeoutTimer: vi.fn()
  })
}))

vi.mock('@renderer/components/chat/settings/ChatPreferenceSections', () => ({
  default: () => <div data-testid="chat-preference-sections" />
}))

vi.mock('@renderer/components/SettingsPrimitives', async () => {
  const React = await import('react')
  const passthrough =
    (tag: string) =>
    ({ children, ...props }: { children?: React.ReactNode }) =>
      React.createElement(tag, props, children)

  return {
    SettingDescription: passthrough('p'),
    SettingDivider: passthrough('hr'),
    SettingGroup: passthrough('section'),
    SettingRow: passthrough('div'),
    SettingRowTitle: passthrough('div'),
    SettingsContentBody: passthrough('main'),
    SettingsContentColumn: passthrough('main'),
    SettingTitle: passthrough('h2')
  }
})

vi.mock('@renderer/components/Scrollbar', () => ({
  default: ({ children, ...props }: { children?: React.ReactNode }) => <div {...props}>{children}</div>
}))

vi.mock('@renderer/components/icons/ResetIcon', () => ({
  default: (props: any) => <span data-testid="reset-icon" {...props} />
}))

vi.mock('../components/ThemeColorPicker', () => ({
  default: ({ ariaLabel, value }: { ariaLabel?: string; value?: string }) => (
    <button aria-label={ariaLabel} type="button">
      {value ?? 'theme-color'}
    </button>
  )
}))
vi.mock('@renderer/utils/error', () => ({
  formatErrorMessage: (error: unknown) => (error instanceof Error ? error.message : String(error))
}))

describe('AppearanceSettings menu presentation mode', () => {
  const setMenuPresentationMode = vi.fn<(mode: MenuPresentationMode) => Promise<void>>()
  const setTimeoutTimer = vi.fn<(key: string, callback: () => void, delay: number) => void>()

  beforeEach(() => {
    vi.clearAllMocks()
    setMenuPresentationMode.mockResolvedValue(undefined)
    // Confirm resolves true so the confirmed branch runs; a test that needs the decline
    // path overrides with mockResolvedValueOnce(false).
    vi.mocked(popup.confirm).mockImplementation(async () => true)
  })

  it('does nothing when the selected mode is already active', () => {
    void confirmMenuPresentationModeChange({
      currentMode: 'cherry',
      mode: 'cherry',
      setMenuPresentationMode,
      setTimeoutTimer,
      t
    })

    expect(popup.confirm).not.toHaveBeenCalled()
  })

  it('saves the selected mode and schedules relaunch after confirmation', async () => {
    await confirmMenuPresentationModeChange({
      currentMode: 'cherry',
      mode: 'native',
      setMenuPresentationMode,
      setTimeoutTimer,
      t
    })

    expect(popup.confirm).toHaveBeenCalledWith(
      expect.objectContaining({
        title: 'settings.general.common.menu.presentation_mode.restart.title',
        content: 'settings.general.common.menu.presentation_mode.restart.content',
        okText: 'common.confirm',
        cancelText: 'common.cancel',
        centered: true
      })
    )

    expect(setMenuPresentationMode).toHaveBeenCalledWith('native')
    expect(setTimeoutTimer).toHaveBeenCalledWith('handleMenuPresentationModeChange', expect.any(Function), 500)

    setTimeoutTimer.mock.calls[0][1]()
    expect(window.api.application.relaunch).toHaveBeenCalled()
  })

  it('surfaces save failures without scheduling relaunch', async () => {
    const error = new Error('save failed')
    setMenuPresentationMode.mockRejectedValue(error)

    await expect(
      confirmMenuPresentationModeChange({
        currentMode: 'cherry',
        mode: 'native',
        setMenuPresentationMode,
        setTimeoutTimer,
        t
      })
    ).rejects.toThrow('save failed')

    expect(toast.error).toHaveBeenCalledWith('save failed')
    expect(setTimeoutTimer).not.toHaveBeenCalled()
    expect(window.api.application.relaunch).not.toHaveBeenCalled()
  })
})

describe('AppearanceSettings selectors', () => {
  beforeEach(() => {
    MockUsePreferenceUtils.resetMocks()
    i18nMock.language = 'zh-CN'
    i18nMock.resolvedLanguage = 'zh-CN'
    mocks.request.mockReset()
    themeMocks.setTheme.mockReset()
    themeMocks.theme = 'light'
    themeMocks.settedTheme = 'light'
    document.documentElement.classList.remove('dark')
  })

  it('defaults an unset language to Chinese and offers only Chinese and English', () => {
    MockUsePreferenceUtils.setPreferenceValue('app.language', null)

    render(<AppearanceSettings />)

    expect(screen.getByRole('combobox', { name: 'common.language' })).toBeInTheDocument()
    expect(screen.queryByRole('radiogroup', { name: 'common.language' })).not.toBeInTheDocument()
    expect(screen.getByRole('option', { name: '中文' })).toHaveAttribute('aria-selected', 'true')
    expect(screen.getByRole('option', { name: 'English' })).toHaveAttribute('aria-selected', 'false')
    expect(screen.queryByText('日本語')).not.toBeInTheDocument()
    expect(screen.queryByText('中文（繁体）')).not.toBeInTheDocument()
  })

  it('shows English when that language is saved', () => {
    MockUsePreferenceUtils.setPreferenceValue('app.language', 'en-US')

    render(<AppearanceSettings />)

    expect(screen.getByRole('option', { name: 'English' })).toHaveAttribute('aria-selected', 'true')
  })

  it('falls back to Chinese when a stored language is outside the product pair', () => {
    MockUsePreferenceUtils.setPreferenceValue('app.language', 'ja-JP')

    render(<AppearanceSettings />)

    expect(screen.getByRole('option', { name: '中文' })).toHaveAttribute('aria-selected', 'true')
  })

  it('applies a language choice immediately', () => {
    MockUsePreferenceUtils.setPreferenceValue('app.language', 'zh-CN')

    render(<AppearanceSettings />)
    fireEvent.click(screen.getByRole('option', { name: 'English' }))

    expect(i18nMock.changeLanguage).toHaveBeenCalledWith('en-US')
    expect(MockUsePreferenceUtils.getPreferenceValue('app.language')).toBe('en-US')
  })

  it('shows window theme and language, and not wallpaper or the retired layout rows', () => {
    render(<AppearanceSettings />)

    expect(screen.queryByText('settings.display.font.title')).not.toBeInTheDocument()
    expect(screen.queryByText('settings.aligned.appearance.theme_pack')).not.toBeInTheDocument()
    expect(screen.queryByText('settings.aligned.appearance.density')).not.toBeInTheDocument()
    expect(screen.queryByText('settings.zoom.title')).not.toBeInTheDocument()
    expect(screen.queryByText('settings.display.custom.css.label')).not.toBeInTheDocument()
    expect(screen.queryByTestId('chat-preference-sections')).not.toBeInTheDocument()
    expect(screen.queryByText('settings.appearance.wallpaper')).not.toBeInTheDocument()
    expect(document.getElementById('setting-appearance-theme-mode')).toBeInTheDocument()
    expect(document.getElementById('setting-appearance-wallpaper')).not.toBeInTheDocument()
    expect(document.getElementById('setting-appearance-display-language')).toBeInTheDocument()
  })

  it('shows every theme as a preview card and switches from the card', () => {
    render(<AppearanceSettings />)

    const theme = screen.getByRole('radiogroup', { name: 'settings.appearance.window' })
    expect(theme).toBeInTheDocument()
    const light = screen.getByRole('radio', { name: 'settings.theme.light' })
    const dark = screen.getByRole('radio', { name: 'settings.theme.dark' })
    const system = screen.getByRole('radio', { name: 'settings.appearance.follow_system' })
    expect(light).toHaveAttribute('aria-checked', 'true')
    expect(dark).toHaveAttribute('aria-checked', 'false')
    expect(system).toHaveAttribute('aria-checked', 'false')
    expect(light.querySelector('[data-theme-preview]')).toHaveClass('border-[#171717]')
    expect(dark.querySelector('[data-theme-preview]')).toHaveClass('border-transparent')
    expect(system.querySelector('[data-theme-preview]')).toHaveClass('border-transparent')

    fireEvent.click(dark)

    expect(themeMocks.setTheme).toHaveBeenCalledWith(ThemeMode.dark)
  })

  it('places window theme, then language', () => {
    const { container } = render(<AppearanceSettings />)
    const groupTitles = Array.from(container.querySelectorAll('h2')).map((heading) => heading.textContent)

    expect(groupTitles).toEqual(['settings.appearance.window', 'settings.general.common.sections.display_language'])
  })
})
