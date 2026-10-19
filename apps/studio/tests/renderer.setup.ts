import '@testing-library/jest-dom/vitest'
import { createRequire } from 'node:module'

import { beforeAll, beforeEach, expect, vi } from 'vitest'

import { MockCherrystudioUI } from './__mocks__/renderer/CherrystudioUI'
import { resetPopupMocks } from './__mocks__/renderer/popup'
import { resetToastMocks } from './__mocks__/renderer/toast'

vi.stubGlobal('__APP_EDITION__', 'global')

// Radix Select scrolls its highlighted option into view on open; happy-dom
// lacks Element.prototype.scrollIntoView, which crashes the effect and kills
// the host dialog's render mid-interaction.
if (typeof Element !== 'undefined' && !Element.prototype.scrollIntoView) {
  Element.prototype.scrollIntoView = function scrollIntoView() {}
}

// Radix Avatar/Image gates its <img> on load; the loader never fires for
// file:// / data-less sources in the DOM environment, so URL-logged avatars
// stay on the fallback forever in tests. Fire the loader synchronously —
// production loads asynchronously and is unaffected (the stub only completes
// loads that would otherwise hang).
if (typeof Image !== 'undefined') {
  const descriptor = Object.getOwnPropertyDescriptor(Image.prototype, 'src')
  if (descriptor?.set) {
    Object.defineProperty(Image.prototype, 'src', {
      set(value: string) {
        descriptor.set!.call(this, value)
        // Radix useImage uses addEventListener('load'), not the onload
        // property; mark complete and dispatch so both paths observe the load.
        try {
          Object.defineProperty(this, 'complete', { value: true, configurable: true })
          Object.defineProperty(this, 'naturalWidth', { value: 1, configurable: true })
          Object.defineProperty(this, 'naturalHeight', { value: 1, configurable: true })
        } catch {
          /* frozen prototype — event dispatch still notifies listeners */
        }
        queueMicrotask(() => {
          this.dispatchEvent(new Event('load'))
          this.onload?.call(this, new Event('load'))
        })
      },
      get: descriptor.get,
      configurable: true
    })
  }
}

const require = createRequire(import.meta.url)

// Repo-root bun nests React 19.3. Radix dialog's CJS scroll lock requires that
// copy, while the test renderer uses Studio's React 19.2.3, so useRef is null.
// Pin every require('react') to the Studio copy.
const reactEntries: Record<string, string> = {
  react: require.resolve('react'),
  'react-dom': require.resolve('react-dom'),
  'react/jsx-runtime': require.resolve('react/jsx-runtime'),
  'react/jsx-dev-runtime': require.resolve('react/jsx-dev-runtime'),
  'react-dom/client': require.resolve('react-dom/client')
}
const moduleApi = require('module') as {
  _resolveFilename: (request: string, parent: unknown, isMain: boolean, options?: unknown) => string
}
const resolveFilename = moduleApi._resolveFilename
moduleApi._resolveFilename = function (request, parent, isMain, options) {
  return reactEntries[request] ?? resolveFilename.call(this, request, parent, isMain, options)
}

const bufferModule = require('buffer')
if (!bufferModule.SlowBuffer) {
  bufferModule.SlowBuffer = bufferModule.Buffer
}

// i18n now initializes lazily via initI18n() instead of a module-level side effect,
// so seed it once per test file for components that render real translations.
// The whole body is guarded: files that vi.mock('@renderer/i18n/resolver') make the dynamic
// import invoke their mock factory (which may throw), and files that
// vi.mock('react-i18next') without initReactI18next make initI18n() reject. Neither
// needs real initialization, so swallow both the import error and the init rejection.
beforeAll(async () => {
  try {
    const mod = await import('@renderer/i18n/resolver')
    await mod.initI18n?.()
  } catch {
    // Intentionally ignored — mocked-i18n test files don't need real init.
  }
})

// Mock LoggerService globally for renderer tests
vi.mock('@logger', async () => {
  const { MockRendererLoggerService, mockRendererLoggerService } = await import('./__mocks__/RendererLoggerService')
  return {
    LoggerService: MockRendererLoggerService,
    loggerService: mockRendererLoggerService
  }
})

// Mock PreferenceService globally for renderer tests
vi.mock('@data/PreferenceService', async () => {
  const { MockPreferenceService } = await import('./__mocks__/renderer/PreferenceService')
  return MockPreferenceService
})

// Mock DataApiService globally for renderer tests
vi.mock('@data/DataApiService', async () => {
  const { MockDataApiService } = await import('./__mocks__/renderer/DataApiService')
  return MockDataApiService
})

// Mock CacheService globally for renderer tests
vi.mock('@data/CacheService', async () => {
  const { MockCacheService } = await import('./__mocks__/renderer/CacheService')
  return MockCacheService
})

// Mock useDataApi hooks globally for renderer tests
vi.mock('@data/hooks/useDataApi', async () => {
  const { MockUseDataApi } = await import('./__mocks__/renderer/useDataApi')
  return MockUseDataApi
})

// Mock usePreference hooks globally for renderer tests
vi.mock('@data/hooks/usePreference', async () => {
  const { MockUsePreference } = await import('./__mocks__/renderer/usePreference')
  return MockUsePreference
})

// Mock useCache hooks globally for renderer tests
vi.mock('@data/hooks/useCache', async () => {
  const { MockUseCache } = await import('./__mocks__/renderer/useCache')
  return MockUseCache
})

// Mock the toast notification surface globally for renderer tests
vi.mock('@renderer/services/toast', async () => {
  const { MockToast } = await import('./__mocks__/renderer/toast')
  return MockToast
})

// Mock the popup (dialog) surface globally for renderer tests. Infra unit tests that
// need the real store opt out with vi.mock('@renderer/services/popup', importOriginal).
vi.mock('@renderer/services/popup', async () => {
  const { MockPopup } = await import('./__mocks__/renderer/popup')
  return MockPopup
})

// Reset the toast/popup spies (and restore the confirm-family default) before each
// test so suites don't need to manage that shared mock state themselves.
beforeEach(() => {
  resetToastMocks()
  resetPopupMocks()
})

// Mock uuid globally for renderer tests
let uuidCounter = 0
vi.mock('uuid', async (importOriginal) => ({
  ...(await importOriginal<typeof import('uuid')>()),
  v4: () => 'test-uuid-' + ++uuidCounter
}))

vi.mock('@iconify/react', () => {
  const React = require('react')
  return {
    Icon: ({ icon, ...props }: { icon?: string }) =>
      React.createElement('span', { ...props, 'data-icon': icon, 'data-testid': 'iconify-icon' })
  }
})

vi.mock('axios', () => {
  const defaultAxiosMock = {
    get: vi.fn().mockResolvedValue({ data: {} }), // Mocking axios GET request
    post: vi.fn().mockResolvedValue({ data: {} }) // Mocking axios POST request
    // You can add other axios methods like put, delete etc. as needed
  }

  const isAxiosError = (error: unknown): error is { isAxiosError?: boolean } =>
    Boolean((error as { isAxiosError?: boolean } | undefined)?.isAxiosError)

  return {
    default: defaultAxiosMock,
    isAxiosError
  }
})

// Mock ResizeObserver for jsdom environment
vi.stubGlobal(
  'ResizeObserver',
  class ResizeObserver {
    observe() {}
    unobserve() {}
    disconnect() {}
  }
)

vi.stubGlobal('electron', {
  ipcRenderer: {
    on: vi.fn(),
    send: vi.fn(),
    invoke: vi.fn().mockResolvedValue(undefined)
  }
})
vi.stubGlobal('api', {
  file: {
    read: vi.fn().mockResolvedValue('[]'),
    writeWithId: vi.fn().mockResolvedValue(undefined)
  },
  // Legacy `window.api.application.*` bridge — `relaunch` stays on legacy IPC, so tests that
  // trigger a restart flow reach this stub. Stubbed to a no-op resolving spy.
  application: {
    relaunch: vi.fn().mockResolvedValue(undefined)
  },
  // Low-level IpcApi bridge — the typed `ipcApi` facade calls through this. Stubbed so
  // module-level `ipcApi.on(...)` side effects (e.g. the renderer notification singleton)
  // don't crash tests that transitively import them without a local `@renderer/ipc` mock.
  ipcApi: {
    request: vi.fn().mockResolvedValue(undefined),
    on: vi.fn(() => () => {})
  }
})

// Markdown stylesheet import is a side-effect no-op in tests
vi.mock('@a3s-lab/ui/components/composites/markdown/styles', () => ({}))

// Mock @a3s-lab/ui globally for renderer tests
vi.mock('@a3s-lab/ui', async () => {
  const React = require('react')
  const { createPortal } = require('react-dom')
  // Real implementation: its filtering/normalization contract is what callers
  // are tested against, and a stub would drift from it.
  const { InputNumber } = await import('@a3s-lab/ui/components/primitives/input-number')
  const { InputGroupInputNumber } = await import('@a3s-lab/ui/components/primitives/input-group')
  const { Pagination: ConsolePagination } = await import('@a3s-lab/ui/components/console/Pagination')
  const rowMotion = await import('@a3s-lab/ui/lib/rowMotion')
  const SelectContext = React.createContext({ value: undefined, onValueChange: undefined, items: new Map() })
  const PopoverContext = React.createContext({ open: false, onOpenChange: undefined })
  const DialogOpenContext = React.createContext(null)
  const DialogLabelContext = React.createContext(null)
  const DialogPortalContainerContext = React.createContext(null)
  const DialogDepthContext = React.createContext(0)
  const ContextMenuContext = React.createContext({ open: false, onOpenChange: undefined })
  const DropdownMenuOpenContext = React.createContext(null)
  const AccordionContext = React.createContext({
    collapsible: false,
    onValueChange: undefined,
    type: 'single',
    value: ''
  })
  const AccordionItemContext = React.createContext({ disabled: false, value: '' })
  const FormFieldNameContext = React.createContext(undefined)
  return {
    ConsolePagination,
    ROW_LEAVE_MS: rowMotion.ROW_LEAVE_MS,
    rowMotionClassName: rowMotion.rowMotionClassName,
    rowMotionStyle: rowMotion.rowMotionStyle,
    // Markdown — `@a3s-lab/ui` barrel re-exports composites/markdown (#16228).
    // Lightweight stand-ins so tests mounting real ChatMarkdown still surface text.
    Markdown: ({ children }) => React.createElement('div', null, children),
    StreamingMarkdown: ({ children }) => React.createElement('div', null, children),
    withChatPlugins: () => [],
    withMath: (plugins) => plugins ?? [],
    withMermaid: (plugins) => plugins ?? [],
    withFullMarkdown: (plugins) => plugins ?? [],
    defaultMarkdownPlugins: [],
    useMarkdownBlockContext: () => ({ content: '' }),
    usePortalContainer: () => null,
    createSlugger: () => ({ slug: (value) => String(value ?? '') }),
    extractTextFromNode: () => '',
    BlurCancelPointerSensor: class BlurCancelPointerSensor {},
    ReorderableList: ({ items, renderItem, getId, dragHandle }) =>
      React.createElement(
        React.Fragment,
        null,
        items.map((item, index) =>
          React.createElement(
            'div',
            { key: getId(item) },
            renderItem(item, index, {
              dragging: false,
              dragHandleProps: dragHandle
                ? {
                    ref: () => {},
                    attributes: { role: 'button', 'aria-roledescription': 'sortable', tabIndex: 0 },
                    listeners: {}
                  }
                : undefined
            })
          )
        )
      ),
    Sortable: ({ items, itemKey, renderItem, className }) => {
      const getKey = typeof itemKey === 'function' ? itemKey : (item) => item[itemKey]
      return React.createElement(
        'div',
        { className },
        items.map((item) =>
          React.createElement('div', { key: getKey(item) }, renderItem(item, { dragging: false, overlay: false }))
        )
      )
    },
    useDndReorder: () => ({ onSortEnd: () => {} }),
    NormalTooltip: ({ children }) => children,
    Button: ({
      children,
      onPress,
      disabled,
      isDisabled,
      loading,
      startContent,
      asChild,
      variant,
      size,
      className,
      ...props
    }) => {
      const buttonProps = {
        ...props,
        'data-slot': 'button',
        'data-variant': variant ?? 'default',
        className:
          ['inline-flex rounded-md', size === 'md' ? 'min-h-9 text-[13px]' : undefined, className]
            .filter(Boolean)
            .join(' ') || undefined,
        onClick: onPress ?? props.onClick,
        disabled: disabled || isDisabled || loading
      }
      if (asChild && React.isValidElement(children)) {
        const childProps = children.props || {}
        return React.cloneElement(children, {
          ...buttonProps,
          ...childProps,
          className: [buttonProps.className, childProps.className].filter(Boolean).join(' ') || undefined,
          style: { ...buttonProps.style, ...childProps.style },
          onClick: (...args) => {
            buttonProps.onClick?.(...args)
            childProps.onClick?.(...args)
          },
          onKeyDown: (...args) => {
            buttonProps.onKeyDown?.(...args)
            childProps.onKeyDown?.(...args)
          }
        })
      }
      return React.createElement('button', buttonProps, startContent, children)
    },
    ConfirmDialog: ({ cancelText, confirmText, content, description, onConfirm, onOpenChange, open, title }) =>
      open
        ? React.createElement(
            'div',
            { role: 'dialog', 'aria-label': title },
            React.createElement('h2', null, title),
            description ? React.createElement('p', null, description) : null,
            content,
            React.createElement('button', { type: 'button', onClick: () => onOpenChange?.(false) }, cancelText),
            React.createElement(
              'button',
              {
                type: 'button',
                onClick: async () => {
                  await onConfirm?.()
                  onOpenChange?.(false)
                }
              },
              confirmText
            )
          )
        : null,
    InputNumber,
    InputGroupInputNumber,
    Input: ({
      hasError,
      'aria-invalid': ariaInvalid,
      className,
      list,
      size: _size,
      allowClear: _allowClear,
      onClear: _onClear,
      ...props
    }) =>
      React.createElement('input', {
        ...props,
        list,
        'aria-invalid': ariaInvalid,
        className: [
          'h-9 w-full min-w-0 rounded-[8px] bg-[#f5f5f5] px-3 text-[13px]',
          className,
          hasError || ariaInvalid ? 'ant-input-status-error' : undefined,
          list && ariaInvalid ? 'ant-select-status-error' : undefined
        ]
          .filter(Boolean)
          .join(' ')
      }),
    Calendar: ({ mode, defaultMonth, onSelect, onDayKeyDown, labels, formatters, weekStartsOn = 1, components }) => {
      const anchor = defaultMonth instanceof Date && !Number.isNaN(defaultMonth.getTime()) ? defaultMonth : new Date()
      const [cursor, setCursor] = React.useState(() => new Date(anchor.getFullYear(), anchor.getMonth(), 1))
      const pending = React.useRef(null)
      const year = cursor.getFullYear()
      const month = cursor.getMonth()
      const count = new Date(year, month + 1, 0).getDate()
      const lead = (new Date(year, month, 1).getDay() - weekStartsOn + 7) % 7
      const origin = new Date(2026, 9, 4 + weekStartsOn)
      const weekday = (index) => {
        const date = new Date(origin.getFullYear(), origin.getMonth(), origin.getDate() + index)
        return typeof formatters?.formatWeekdayName === 'function'
          ? formatters.formatWeekdayName(date)
          : date.toLocaleDateString('zh-CN', { weekday: 'narrow' })
      }
      const iso = (date) => {
        const monthText = String(date.getMonth() + 1).padStart(2, '0')
        const dayText = String(date.getDate()).padStart(2, '0')
        return `${date.getFullYear()}-${monthText}-${dayText}`
      }
      const pick = (date) => {
        if (mode !== 'range') {
          onSelect?.(date)
          return
        }
        const current = pending.current
        if (!current) {
          pending.current = date
          onSelect?.({ from: date, to: date })
          return
        }
        pending.current = null
        const from = current.getTime() <= date.getTime() ? current : date
        const to = current.getTime() <= date.getTime() ? date : current
        onSelect?.({ from, to })
      }
      const cells = []
      for (let index = 0; index < lead; index += 1) cells.push(null)
      for (let day = 1; day <= count; day += 1) cells.push(new Date(year, month, day))
      while (cells.length % 7 !== 0) cells.push(null)
      const rows = []
      for (let index = 0; index < cells.length; index += 7) rows.push(cells.slice(index, index + 7))
      return React.createElement(
        'div',
        null,
        React.createElement('button', {
          type: 'button',
          'aria-label': typeof labels?.labelPrevious === 'function' ? labels.labelPrevious() : 'previous',
          onClick: () => setCursor(new Date(year, month - 1, 1))
        }),
        React.createElement('button', {
          type: 'button',
          'aria-label': typeof labels?.labelNext === 'function' ? labels.labelNext() : 'next',
          onClick: () => setCursor(new Date(year, month + 1, 1))
        }),
        React.createElement(
          'table',
          null,
          React.createElement(
            'thead',
            null,
            React.createElement(
              'tr',
              null,
              Array.from({ length: 7 }, (_, index) =>
                React.createElement('th', { key: index, scope: 'col' }, weekday(index))
              )
            )
          ),
          React.createElement(
            'tbody',
            null,
            rows.map((row, rowIndex) =>
              React.createElement(
                'tr',
                { key: rowIndex },
                row.map((date, column) =>
                  date
                    ? React.createElement(
                        'td',
                        { key: column, 'data-day': iso(date) },
                        React.createElement(
                          components?.DayButton ?? 'button',
                          {
                            type: 'button',
                            day: { date },
                            modifiers: {},
                            'aria-label':
                              typeof labels?.labelDayButton === 'function' ? labels.labelDayButton(date) : undefined,
                            onClick: () => pick(date),
                            onKeyDown: (event) => onDayKeyDown?.(date, {}, event)
                          },
                          String(date.getDate())
                        )
                      )
                    : React.createElement('td', { key: column })
                )
              )
            )
          )
        )
      )
    },
    SearchInput: ({ containerClassName, clearLabel, onClear, value, ...props }) =>
      React.createElement(
        'div',
        { className: containerClassName },
        React.createElement('input', { ...props, type: 'search', value }),
        onClear && value
          ? React.createElement('button', { type: 'button', 'aria-label': clearLabel, onClick: onClear })
          : null
      ),
    Textarea: {
      Input: ({ hasError, 'aria-invalid': ariaInvalid, className, onValueChange, onChange, ...props }) =>
        React.createElement('textarea', {
          ...props,
          'aria-invalid': ariaInvalid,
          className: [
            'min-h-16 w-full rounded-[8px] bg-[#f5f5f5] px-3 py-2 text-[13px]',
            className,
            hasError || ariaInvalid ? 'ant-input-status-error' : undefined
          ]
            .filter(Boolean)
            .join(' '),
          onChange: (event: React.ChangeEvent<HTMLTextAreaElement>) => {
            onChange?.(event)
            onValueChange?.(event.target.value)
          }
        })
    },
    Accordion: ({ children, collapsible = false, defaultValue, onValueChange, type = 'single', value, ...props }) => {
      const [internalValue, setInternalValue] = React.useState(defaultValue ?? (type === 'multiple' ? [] : ''))
      const currentValue = value ?? internalValue
      const handleValueChange = (nextValue) => {
        if (value === undefined) setInternalValue(nextValue)
        onValueChange?.(nextValue)
      }
      return React.createElement(
        AccordionContext.Provider,
        { value: { collapsible, onValueChange: handleValueChange, type, value: currentValue } },
        React.createElement('div', { ...props, 'data-testid': props['data-testid'] ?? 'accordion' }, children)
      )
    },
    AccordionItem: ({ children, disabled = false, value, ...props }) =>
      React.createElement(
        AccordionItemContext.Provider,
        { value: { disabled, value } },
        React.createElement('div', { ...props, 'data-testid': props['data-testid'] ?? 'accordion-item' }, children)
      ),
    AccordionTrigger: ({ children, disabled, onClick, ...props }) => {
      const accordion = React.use(AccordionContext)
      const item = React.use(AccordionItemContext)
      const isOpen =
        accordion.type === 'multiple' ? accordion.value.includes(item.value) : accordion.value === item.value
      const handleClick = (event) => {
        onClick?.(event)
        if (event.defaultPrevented || disabled || item.disabled) return

        if (accordion.type === 'multiple') {
          accordion.onValueChange?.(
            isOpen ? accordion.value.filter((value) => value !== item.value) : [...accordion.value, item.value]
          )
        } else if (!isOpen || accordion.collapsible) {
          accordion.onValueChange?.(isOpen ? '' : item.value)
        }
      }
      return React.createElement(
        'button',
        {
          ...props,
          type: 'button',
          disabled: disabled || item.disabled,
          'aria-expanded': isOpen,
          'data-testid': props['data-testid'] ?? 'accordion-trigger',
          onClick: handleClick
        },
        children
      )
    },
    AccordionContent: ({ children, className, contentClassName, forceMount = false, ...props }) => {
      const accordion = React.use(AccordionContext)
      const item = React.use(AccordionItemContext)
      const isOpen =
        accordion.type === 'multiple' ? accordion.value.includes(item.value) : accordion.value === item.value
      if (!isOpen && !forceMount) return null

      return React.createElement(
        'div',
        {
          ...props,
          className: [contentClassName, className].filter(Boolean).join(' '),
          'data-state': isOpen ? 'open' : 'closed',
          'data-testid': props['data-testid'] ?? 'accordion-content'
        },
        children
      )
    },
    DropdownMenu: ({ children, onOpenChange }) =>
      React.createElement(
        DropdownMenuOpenContext.Provider,
        { value: onOpenChange ?? null },
        React.createElement('div', null, children)
      ),
    DropdownMenuTrigger: ({ children }) => {
      const onOpenChange = React.use(DropdownMenuOpenContext)
      return React.createElement('span', { onClick: () => onOpenChange?.(true) }, children)
    },
    DropdownMenuContent: ({ children }) => React.createElement('div', null, children),
    DropdownMenuSeparator: () => React.createElement('hr'),
    DropdownMenuSub: ({ children }) => React.createElement('div', null, children),
    DropdownMenuSubContent: ({ children }) => React.createElement('div', null, children),
    DropdownMenuSubTrigger: ({ children }) => React.createElement('div', null, children),
    DropdownMenuCheckboxItem: ({ children, disabled, onCheckedChange }) =>
      React.createElement('button', { type: 'button', disabled, onClick: onCheckedChange }, children),
    DropdownMenuItem: ({ children, disabled, onSelect }) =>
      React.createElement('button', { type: 'button', disabled, onClick: onSelect }, children),
    ContextMenu: ({ children, defaultOpen = false, open: controlledOpen, onOpenChange, ...props }) => {
      const [uncontrolledOpen, setUncontrolledOpen] = React.useState(defaultOpen)
      const open = controlledOpen ?? uncontrolledOpen
      const closeAutoFocus = React.useRef<((event: { preventDefault: () => void }) => void) | null>(null)
      const handleOpenChange = (nextOpen: boolean) => {
        if (controlledOpen === undefined) {
          setUncontrolledOpen(nextOpen)
        }
        onOpenChange?.(nextOpen)
      }
      return React.createElement(
        ContextMenuContext.Provider,
        { value: { open, onOpenChange: handleOpenChange, closeAutoFocus } },
        React.createElement('div', { ...props, 'data-testid': 'context-menu' }, children)
      )
    },
    ContextMenuTrigger: ({ children, asChild, ...props }) => {
      const context = React.useContext(ContextMenuContext)
      const triggerProps = {
        ...props,
        'data-testid': 'context-menu-trigger',
        onContextMenu: (event: React.MouseEvent) => {
          props.onContextMenu?.(event)
          if (!event.defaultPrevented && !props.disabled) {
            context.onOpenChange?.(true)
            event.preventDefault()
          }
        }
      }
      if (asChild && React.isValidElement(children)) {
        const childProps = children.props || {}
        return React.cloneElement(children, {
          ...triggerProps,
          ...childProps,
          onContextMenu: (event: React.MouseEvent) => {
            childProps.onContextMenu?.(event)
            if (!event.defaultPrevented) {
              triggerProps.onContextMenu(event)
            }
          }
        })
      }
      return React.createElement('div', triggerProps, children)
    },
    ContextMenuContent: ({ children, onCloseAutoFocus, ...props }) => {
      const context = React.useContext(ContextMenuContext)
      if (context.closeAutoFocus) context.closeAutoFocus.current = context.open ? (onCloseAutoFocus ?? null) : null
      return context.open
        ? React.createElement('div', { ...props, 'data-testid': 'context-menu-content' }, children)
        : null
    },
    ContextMenuItem: ({ children, onSelect, ...props }) => {
      const context = React.useContext(ContextMenuContext)
      return React.createElement(
        'button',
        {
          ...props,
          type: 'button',
          onClick: (event: React.MouseEvent) => {
            onSelect?.(event)
            context.closeAutoFocus?.current?.({ preventDefault() {} })
            context.onOpenChange?.(false)
          },
          'data-testid': 'context-menu-item'
        },
        children
      )
    },
    ContextMenuItemContent: ({ badge, children, icon, shortcut, ...props }) =>
      React.createElement(
        React.Fragment,
        null,
        React.createElement('span', { ...props }, icon, children),
        badge,
        shortcut ? React.createElement('span', null, shortcut) : null
      ),
    ContextMenuSeparator: (props) => React.createElement('div', { ...props, 'data-testid': 'context-menu-separator' }),
    ContextMenuSub: ({ children, ...props }) =>
      React.createElement('div', { ...props, 'data-testid': 'context-menu-sub' }, children),
    ContextMenuSubTrigger: ({ children, ...props }) =>
      React.createElement('button', { ...props, type: 'button', 'data-testid': 'context-menu-sub-trigger' }, children),
    ContextMenuSubContent: ({ children, ...props }) =>
      React.createElement('div', { ...props, 'data-testid': 'context-menu-sub-content' }, children),
    ImagePreviewContextMenu: ({ actions = [], children, context, item }) =>
      React.createElement(
        'div',
        { 'data-testid': 'image-preview-context-menu' },
        children,
        actions.map((action) =>
          React.createElement(
            'button',
            {
              disabled: action.disabled,
              key: action.id,
              onClick: () => action.onSelect?.(item, context),
              type: 'button'
            },
            action.icon,
            action.label
          )
        )
      ),
    ImagePreviewDialog: ({
      activeIndex = 0,
      className,
      items = [],
      labels = {},
      onOpenChange,
      open,
      overlayClassName,
      toolbarActions = []
    }) =>
      open
        ? React.createElement(
            React.Fragment,
            null,
            overlayClassName
              ? React.createElement('div', { className: overlayClassName, 'data-slot': 'dialog-overlay' })
              : null,
            React.createElement(
              'div',
              { className, 'data-testid': 'image-preview-dialog' },
              items[activeIndex]
                ? React.createElement('img', {
                    alt: items[activeIndex].alt,
                    src: items[activeIndex].src
                  })
                : null,
              toolbarActions.map((action) =>
                React.createElement(
                  'button',
                  {
                    disabled: action.disabled,
                    key: action.id,
                    onClick: () =>
                      action.onSelect?.(items[activeIndex], {
                        close: () => onOpenChange?.(false),
                        index: activeIndex,
                        items,
                        resetTransform: vi.fn(),
                        transform: {
                          flipX: false,
                          flipY: false,
                          offsetX: 0,
                          offsetY: 0,
                          rotation: 0,
                          zoom: 1
                        }
                      }),
                    type: 'button'
                  },
                  action.icon,
                  action.label
                )
              ),
              React.createElement(
                'button',
                { 'aria-label': labels.close, onClick: () => onOpenChange?.(false), type: 'button' },
                labels.close
              )
            )
          )
        : null,
    ImagePreviewImage: ({ item, ...props }) =>
      React.createElement('img', { ...props, alt: item?.alt, src: item?.src, 'data-testid': 'image-preview-image' }),
    ImagePreviewToolbar: ({ labels = {}, transformControls }) =>
      React.createElement(
        'div',
        { 'data-testid': 'image-preview-toolbar' },
        [
          ['zoomOut', labels.zoomOut],
          ['zoomIn', labels.zoomIn],
          ['rotateLeft', labels.rotateLeft],
          ['rotateRight', labels.rotateRight],
          ['flipHorizontal', labels.flipHorizontal],
          ['flipVertical', labels.flipVertical],
          ['reset', labels.reset]
        ].map(([control, label]) =>
          React.createElement(
            'button',
            {
              key: control,
              onClick: () => transformControls?.[control]?.(),
              type: 'button'
            },
            label
          )
        )
      ),
    ImagePreviewViewport: ({ item, onError, onLoad }) =>
      React.createElement('img', {
        alt: item?.alt,
        onError,
        onLoad,
        src: item?.src,
        'data-testid': 'image-preview-viewport'
      }),
    ImagePreviewTrigger: ({ alt, dialogProps: _dialogProps, item, items: _items, ...props }) =>
      React.createElement('img', { ...props, alt: alt ?? item?.alt, src: item?.src }),
    PopConfirm: ({
      cancelLabel = 'Cancel',
      children,
      confirmLabel = 'Confirm',
      defaultOpen = false,
      description,
      onConfirm,
      title
    }) => {
      const [open, setOpen] = React.useState(defaultOpen)
      return React.createElement(
        React.Fragment,
        null,
        React.createElement(
          'span',
          {
            className: 'inline-flex',
            onClick: (event) => {
              event.preventDefault()
              setOpen((current) => !current)
            }
          },
          children
        ),
        open
          ? React.createElement(
              'div',
              { role: 'dialog', 'aria-label': title, className: 'ui-popconfirm' },
              React.createElement('p', null, title),
              description ? React.createElement('p', null, description) : null,
              React.createElement('button', { type: 'button', onClick: () => setOpen(false) }, cancelLabel),
              React.createElement(
                'button',
                {
                  type: 'button',
                  onClick: () => {
                    setOpen(false)
                    onConfirm?.()
                  }
                },
                confirmLabel
              )
            )
          : null
      )
    },
    DialogPortalContainerProvider: ({ children, container }) =>
      React.createElement(DialogPortalContainerContext.Provider, { value: container ?? null }, children),
    Dialog: ({ children, onOpenChange, open, ...props }) => {
      const depth = React.useContext(DialogDepthContext)
      return open
        ? React.createElement(
            DialogOpenContext.Provider,
            { value: onOpenChange },
            React.createElement(
              DialogDepthContext.Provider,
              { value: depth + 1 },
              React.createElement('div', { ...props, 'data-testid': 'dialog' }, children)
            )
          )
        : null
    },
    DialogContent: ({
      children,
      closeLabel,
      closeOnOverlayClick: _closeOnOverlayClick,
      motion: _motion,
      onEscapeKeyDown,
      overlayClassName: _overlayClassName,
      showCloseButton = true,
      size,
      ...props
    }) => {
      const onOpenChange = React.useContext(DialogOpenContext)
      const portalContainer = React.useContext(DialogPortalContainerContext)
      const depth = React.useContext(DialogDepthContext)
      const [label, setLabel] = React.useState('')
      React.useEffect(() => {
        const onKey = (event) => {
          if (event.key !== 'Escape') return
          onEscapeKeyDown?.(event)
          if (event.defaultPrevented) return
          event.preventDefault()
          onOpenChange?.(false)
        }
        document.addEventListener('keydown', onKey)
        return () => document.removeEventListener('keydown', onKey)
      }, [onEscapeKeyDown, onOpenChange])
      const content = React.createElement(
        DialogLabelContext.Provider,
        { value: setLabel },
        React.createElement(
          'div',
          {
            ...props,
            role: 'dialog',
            'aria-label': label || undefined,
            'data-size': size,
            'data-testid': 'dialog-content'
          },
          children,
          showCloseButton !== false && closeLabel
            ? React.createElement('button', {
                type: 'button',
                'aria-label': closeLabel,
                onClick: () => onOpenChange?.(false)
              })
            : null
        )
      )
      // A dialog opened from another dialog uses the page portal, so it is not
      // trapped inside the parent dialog content.
      return depth > 1 && portalContainer ? createPortal(content, portalContainer) : content
    },
    DialogHeader: ({ children, ...props }) =>
      React.createElement('div', { ...props, 'data-testid': 'dialog-header' }, children),
    DialogTitle: ({ children, ...props }) => {
      const setLabel = React.useContext(DialogLabelContext)
      React.useLayoutEffect(() => {
        if (typeof children === 'string' || typeof children === 'number') setLabel?.(String(children))
      }, [children, setLabel])
      return React.createElement('h2', { ...props, 'data-testid': 'dialog-title' }, children)
    },
    DialogDescription: ({ children, ...props }) =>
      React.createElement('p', { ...props, 'data-testid': 'dialog-description' }, children),
    DialogFooter: ({ children, ...props }) =>
      React.createElement('div', { ...props, 'data-testid': 'dialog-footer' }, children),
    Item: ({ asChild, children, className, size, variant, ...props }) => {
      const itemProps = {
        ...props,
        className,
        'data-size': size,
        'data-slot': 'item',
        'data-variant': variant
      }
      if (asChild && React.isValidElement(children)) {
        const childProps = children.props || {}
        return React.cloneElement(children, {
          ...itemProps,
          ...childProps,
          className: [className, childProps.className].filter(Boolean).join(' ') || undefined
        })
      }
      return React.createElement('div', itemProps, children)
    },
    ItemActions: ({ children, ...props }) =>
      React.createElement('div', { ...props, 'data-slot': 'item-actions' }, children),
    ItemContent: ({ children, ...props }) =>
      React.createElement('div', { ...props, 'data-slot': 'item-content' }, children),
    ItemDescription: ({ children, ...props }) =>
      React.createElement('p', { ...props, 'data-slot': 'item-description' }, children),
    ItemGroup: ({ children, ...props }) =>
      React.createElement('div', { ...props, role: 'list', 'data-slot': 'item-group' }, children),
    ItemMedia: ({ children, variant, ...props }) =>
      React.createElement('div', { ...props, 'data-slot': 'item-media', 'data-variant': variant }, children),
    ItemTitle: ({ children, ...props }) =>
      React.createElement('div', { ...props, 'data-slot': 'item-title' }, children),
    // Passthrough unless real react-hook-form methods are supplied, in which case the
    // provider is needed so FormField/FormMessage can read field state.
    Form: (props) => {
      const { FormProvider } = require('react-hook-form')
      return props?.control && props?.formState
        ? React.createElement(FormProvider, props)
        : React.createElement(React.Fragment, null, props.children)
    },
    // react-hook-form bridge: FormField delegates to the real Controller so form tests
    // exercise validation instead of a stub.
    FormField: ({ control, name, render: renderField }) => {
      const { Controller } = require('react-hook-form')
      return React.createElement(
        FormFieldNameContext.Provider,
        { value: name },
        React.createElement(Controller, { control, name, render: renderField })
      )
    },
    FormItem: ({ children, ...props }) => React.createElement('div', { ...props, 'data-slot': 'form-item' }, children),
    FormLabel: ({ children, ...props }) => {
      const name = React.useContext(FormFieldNameContext)
      return React.createElement('label', { ...props, htmlFor: name }, children)
    },
    FormControl: ({ children }) => {
      const name = React.useContext(FormFieldNameContext)
      return React.isValidElement(children) ? React.cloneElement(children, { id: name }) : children
    },
    FormMessage: (props) => {
      const { useFormContext } = require('react-hook-form')
      const name = React.useContext(FormFieldNameContext)
      const context = useFormContext?.()
      const message = name ? context?.formState?.errors?.[name]?.message : undefined
      return message ? React.createElement('div', { ...props, role: 'alert' }, String(message)) : null
    },
    Label: ({ children, ...props }) => React.createElement('label', props, children),
    FieldError: ({ children, errors, ...props }) => {
      const errorMessage = children ?? errors?.find((error) => error?.message)?.message
      return errorMessage ? React.createElement('div', { ...props, role: 'alert' }, errorMessage) : null
    },
    Popover: ({ children, open = false, onOpenChange, ...props }) =>
      React.createElement(
        PopoverContext.Provider,
        { value: { open, onOpenChange } },
        React.createElement('div', { ...props, 'data-testid': 'popover' }, children)
      ),
    PopoverAnchor: ({ children, asChild, ...props }) => {
      if (asChild && React.isValidElement(children)) {
        return React.cloneElement(children, { ...props, ...children.props })
      }
      return React.createElement('div', props, children)
    },
    PopoverTrigger: ({ children, asChild, ...props }) => {
      const context = React.useContext(PopoverContext)
      const triggerProps = {
        ...props,
        'data-testid': 'popover-trigger',
        onClick: (event: React.MouseEvent) => {
          props.onClick?.(event)
          context.onOpenChange?.(!context.open)
        }
      }
      if (asChild && React.isValidElement(children)) {
        return React.cloneElement(children, { ...triggerProps, ...children.props })
      }
      return React.createElement('div', triggerProps, children)
    },
    PopoverContent: ({ children, align, side, sideOffset, ...props }) => {
      const context = React.useContext(PopoverContext)
      return context.open ? React.createElement('div', { ...props, 'data-testid': 'popover-content' }, children) : null
    },
    MenuList: ({ children, ...props }) =>
      React.createElement('div', { ...props, 'data-testid': 'menu-list' }, children),
    MenuDivider: (props) => React.createElement('div', { ...props, 'data-testid': 'menu-divider' }),
    Divider: (props) => React.createElement('div', { ...props, 'data-testid': 'divider' }),
    FieldGroup: ({ children, ...props }) =>
      React.createElement('div', { ...props, 'data-slot': 'field-group' }, children),
    Field: ({ children, ...props }) => React.createElement('div', { ...props, 'data-slot': 'field' }, children),
    FieldLabel: ({ children, ...props }) => React.createElement('label', { ...props }, children),
    MenuItem: ({ active, children, icon, label, labelClassName, onClick, suffix, ...props }) =>
      React.createElement(
        'button',
        {
          ...props,
          type: 'button',
          onClick,
          'data-active': active ? 'true' : undefined,
          'data-testid': 'menu-item'
        },
        icon,
        React.createElement('span', { className: labelClassName }, label),
        suffix,
        children
      ),
    PageHeader: ({ action, bordered, title, ...props }) =>
      React.createElement(
        'div',
        { ...props, 'data-bordered': bordered ? 'true' : undefined, 'data-testid': 'page-header' },
        React.createElement('h2', null, title),
        action
      ),
    Badge: ({ children, ...props }) => React.createElement('span', { ...props, 'data-testid': 'badge' }, children),
    Separator: (props) => React.createElement('hr', { ...props, 'data-testid': 'separator' }),
    // ToastHost mounts one viewport per window. The imperative toast service is mocked separately.
    ToastViewport: () => null,
    Scrollbar: ({ children, autoHideScrollbar: _autoHideScrollbar, showOnHover: _showOnHover, ...props }) =>
      React.createElement('div', { 'data-testid': 'scrollbar', ...props }, children),
    Dropzone: ({
      children,
      getFilesFromEvent: _getFilesFromEvent,
      maxFiles: _maxFiles,
      multiple: _multiple,
      noClick: _noClick,
      noKeyboard: _noKeyboard,
      onDrop: _onDrop,
      onError: _onError,
      validator: _validator,
      ...props
    }) => React.createElement('div', { ...props, 'data-testid': 'dropzone' }, children),
    DropzoneEmptyState: ({ children }) => React.createElement(React.Fragment, null, children),
    Kbd: ({ children, ...props }) => React.createElement('kbd', { ...props }, children),
    Checkbox: ({ checked, onCheckedChange, ...props }) =>
      React.createElement('input', {
        ...props,
        checked,
        type: 'checkbox',
        'data-slot': 'checkbox',
        onChange: (event: React.ChangeEvent<HTMLInputElement>) => onCheckedChange?.(event.target.checked)
      }),
    RadioGroup: ({ children, value, onValueChange, ...props }) =>
      React.createElement(
        'div',
        {
          ...props,
          'data-testid': 'radio-group',
          'data-value': value,
          onChange: (event: React.ChangeEvent<HTMLInputElement>) => onValueChange?.(event.target.value)
        },
        children
      ),
    RadioGroupItem: ({ value, ...props }) =>
      React.createElement('input', { ...props, type: 'radio', value, 'data-testid': 'radio-group-item' }),
    Slider: ({ value, defaultValue, onValueChange, onValueCommit, ...props }) =>
      React.createElement('input', {
        ...props,
        type: 'range',
        value: value?.[0] ?? defaultValue?.[0] ?? 0,
        'data-testid': 'slider',
        onChange: (event: React.ChangeEvent<HTMLInputElement>) => onValueChange?.([Number(event.target.value)]),
        onMouseUp: (event: React.MouseEvent<HTMLInputElement>) =>
          onValueCommit?.([Number((event.target as HTMLInputElement).value)])
      }),
    SegmentedControl: ({ options = [], value, onValueChange, ...props }) =>
      React.createElement(
        'div',
        { ...props, 'data-testid': 'segmented-control', 'data-value': value },
        options.map((option) =>
          React.createElement(
            'button',
            {
              key: option.value,
              type: 'button',
              disabled: option.disabled,
              onClick: () => onValueChange?.(option.value)
            },
            option.label
          )
        )
      ),
    SearchSelect: ({
      id,
      'aria-label': ariaLabel,
      value,
      placeholder,
      options = [],
      onChange,
      emptyText,
      className,
      disabled,
      'data-ui': dataUi
    }) => {
      const [open, setOpen] = React.useState(false)
      const selected = options.find((option) => option.value === value)
      return React.createElement(
        React.Fragment,
        null,
        React.createElement(
          'button',
          {
            id,
            type: 'button',
            role: 'combobox',
            'aria-label': ariaLabel,
            'aria-expanded': open ? 'true' : 'false',
            'data-ui': dataUi,
            disabled,
            className: ['inline-flex h-9 items-center rounded-[8px] bg-[#f5f5f5] px-3 text-[13px]', className]
              .filter(Boolean)
              .join(' '),
            onClick: () => setOpen((current) => !current)
          },
          selected ? React.createElement(React.Fragment, null, selected.icon, selected.label) : placeholder
        ),
        open
          ? createPortal(
              React.createElement(
                'div',
                null,
                options.length === 0
                  ? React.createElement('div', null, emptyText)
                  : options.map((option) =>
                      React.createElement(
                        'button',
                        {
                          key: option.value,
                          type: 'button',
                          role: 'option',
                          'aria-selected': option.value === value ? 'true' : 'false',
                          onClick: () => {
                            onChange?.(option.value)
                            setOpen(false)
                          }
                        },
                        option.icon,
                        option.label
                      )
                    )
              ),
              document.body
            )
          : null
      )
    },
    Select: ({ children, value, onValueChange, ...props }) => {
      const items = React.useRef(new Map())
      const [open, setOpen] = React.useState(false)
      const choose = (next) => {
        onValueChange?.(next)
        setOpen(false)
      }
      return React.createElement(
        SelectContext.Provider,
        { value: { value, onValueChange: choose, items: items.current, open, setOpen } },
        React.createElement('div', { ...props, 'data-testid': 'select', 'data-value': value }, children)
      )
    },
    SelectTrigger: ({ children, className, onClick, ...props }) => {
      const context = React.useContext(SelectContext)
      return React.createElement(
        'button',
        {
          ...props,
          type: 'button',
          role: 'combobox',
          'aria-expanded': context.open ? 'true' : 'false',
          'data-testid': 'select-trigger',
          className: ['inline-flex h-9 items-center rounded-[8px] bg-[#f5f5f5] px-3 text-[13px]', className]
            .filter(Boolean)
            .join(' '),
          onClick: (event) => {
            onClick?.(event)
            context.setOpen?.(!context.open)
          }
        },
        children
      )
    },
    SelectValue: ({ children, placeholder, ...props }) => {
      const context = React.useContext(SelectContext)
      const selected = context.value != null && context.value !== '' ? context.items.get(context.value) : undefined
      return React.createElement(
        'span',
        { ...props, 'data-testid': 'select-value' },
        selected ?? children ?? placeholder
      )
    },
    SelectContent: ({ children, ...props }) => {
      const context = React.useContext(SelectContext)
      if (!context.open) return null
      return createPortal(
        React.createElement('div', { ...props, 'data-testid': 'select-content' }, children),
        document.body
      )
    },
    SelectGroup: ({ children, ...props }) =>
      React.createElement('div', { ...props, 'data-testid': 'select-group' }, children),
    SelectItem: ({ children, value, ...props }) => {
      const context = React.useContext(SelectContext)
      context.items.set(value, children)
      return React.createElement(
        'button',
        {
          ...props,
          type: 'button',
          role: 'option',
          'data-testid': 'select-item',
          'data-value': value,
          onClick: () => context.onValueChange?.(value)
        },
        children
      )
    },
    Combobox: ({ options = [], value, onChange, onValueChange, placeholder, disabled, ...props }) =>
      React.createElement(
        'select',
        {
          ...props,
          disabled,
          value: value ?? '',
          'data-testid': 'combobox',
          onChange: (event: React.ChangeEvent<HTMLSelectElement>) => {
            onChange?.(event.target.value)
            onValueChange?.(event.target.value)
          }
        },
        React.createElement('option', { value: '' }, placeholder),
        options.map((option) =>
          React.createElement(
            'option',
            { key: option.value, value: option.value, disabled: option.disabled },
            option.label
          )
        )
      ),
    Tooltip: ({
      children,
      title,
      content,
      mouseEnterDelay,
      classNames,
      className,
      sideOffset,
      fullWidthTrigger,
      ...props
    }) => {
      // Support both old (title) and new (content) API
      const tooltipText = content || title
      // Mirror the real Tooltip: the trigger wrapper carries classNames.placeholder.
      const wrapperClassName =
        [className, classNames?.placeholder, fullWidthTrigger && 'block w-full min-w-0 max-w-full']
          .filter(Boolean)
          .join(' ') || undefined
      return React.createElement(
        'div',
        {
          ...props,
          ...(wrapperClassName && { className: wrapperClassName }),
          ...(tooltipText && { 'data-slot': 'tooltip-trigger' }),
          'data-testid': 'tooltip',
          ...(tooltipText && { 'data-title': tooltipText }),
          'data-mouse-enter-delay': mouseEnterDelay,
          'data-side-offset': sideOffset
        },
        children,
        tooltipText ? React.createElement('div', { 'data-testid': 'tooltip-content' }, tooltipText) : null
      )
    },
    CircularProgress: ({ value, renderLabel, showLabel, ...props }) =>
      React.createElement(
        'div',
        { ...props, 'data-testid': 'circular-progress', 'data-value': value },
        showLabel ? (renderLabel ? renderLabel(value) : value) : null
      ),
    CustomTag: ({
      children,
      icon,
      color,
      size = 12,
      style,
      tooltip,
      closable,
      onClose,
      onClick,
      onContextMenu,
      disabled,
      inactive,
      className,
      ...props
    }) => {
      const actualColor = inactive ? '#aaaaaa' : color
      const tag = React.createElement(
        'div',
        {
          ...props,
          className,
          style: {
            padding: `${size / 3}px ${closable ? size * 1.8 : size * 0.8}px ${size / 3}px ${size * 0.8}px`,
            color: actualColor,
            backgroundColor: actualColor + '20',
            fontSize: `${size}px`,
            lineHeight: 1,
            cursor: disabled ? 'not-allowed' : onClick ? 'pointer' : 'auto',
            ...style
          },
          onClick: disabled ? undefined : onClick,
          onContextMenu: disabled ? undefined : onContextMenu
        },
        icon,
        children,
        closable
          ? React.createElement(
              'button',
              {
                type: 'button',
                onClick: (event) => {
                  event.stopPropagation()
                  onClose?.()
                }
              },
              'x'
            )
          : null
      )

      return tooltip
        ? React.createElement(
            'div',
            { 'data-testid': 'tooltip', 'data-title': tooltip },
            tag,
            React.createElement('div', { 'data-testid': 'tooltip-content' }, tooltip)
          )
        : tag
    },
    Spinner: ({ text, ...props }) => React.createElement('div', { ...props, 'data-testid': 'spinner' }, text),
    CodeEditor: ({ children, ...props }) =>
      React.createElement('div', { ...props, 'data-testid': 'code-editor' }, children),
    Flex: ({ children, ...props }) => React.createElement('div', { ...props, 'data-testid': 'flex' }, children),
    ExpandableText: ({ children, ...props }) =>
      React.createElement('div', { ...props, 'data-testid': 'expandable-text' }, children),
    // Add other commonly used UI components
    Box: ({ children, ...props }) => React.createElement('div', { ...props, 'data-testid': 'box' }, children),
    Center: ({ children, ...props }) => React.createElement('div', { ...props, 'data-testid': 'center' }, children),
    ColFlex: ({ children, ...props }) => React.createElement('div', { ...props, 'data-testid': 'col-flex' }, children),
    RowFlex: ({ children, ...props }) => React.createElement('div', { ...props, 'data-testid': 'row-flex' }, children),
    SpaceBetweenRowFlex: ({ children, ...props }) =>
      React.createElement('div', { ...props, 'data-testid': 'space-between-row-flex' }, children),
    Ellipsis: ({ children, ...props }) => React.createElement('div', { ...props, 'data-testid': 'ellipsis' }, children),
    TextBadge: ({ children, ...props }) =>
      React.createElement('div', { ...props, 'data-testid': 'text-badge' }, children),
    Badge: ({ children, ...props }) => React.createElement('span', { ...props, 'data-testid': 'badge' }, children),
    EmptyState: ({
      children,
      title,
      description,
      actionLabel,
      onAction,
      secondaryLabel,
      onSecondary,
      preset,
      compact: _compact,
      ...props
    }) =>
      React.createElement(
        'div',
        { ...props, 'data-testid': 'empty-state', 'data-preset': preset },
        title ? React.createElement('div', {}, title) : null,
        description ? React.createElement('div', {}, description) : null,
        children,
        actionLabel && onAction
          ? React.createElement('button', { type: 'button', onClick: onAction }, actionLabel)
          : null,
        secondaryLabel && onSecondary
          ? React.createElement('button', { type: 'button', onClick: onSecondary }, secondaryLabel)
          : null
      ),
    Alert: ({ children, message, description, action, icon, showIcon, type, ...props }) =>
      React.createElement(
        'div',
        { ...props, role: 'alert', 'data-testid': 'alert', 'data-type': type },
        showIcon ? icon : null,
        message,
        description,
        children,
        action
      ),
    EditableNumber: ({ value, onChange, disabled, ...props }) =>
      React.createElement('input', {
        ...props,
        type: 'number',
        value: value ?? '',
        disabled,
        'data-testid': 'editable-number',
        onChange: (event: React.ChangeEvent<HTMLInputElement>) =>
          onChange?.(event.target.value === '' ? null : event.target.valueAsNumber)
      }),
    Skeleton: ({ children, ...props }) => React.createElement('div', { ...props, 'data-testid': 'skeleton' }, children),
    HelpTooltip: ({ children, ...props }) =>
      React.createElement('div', { ...props, 'data-testid': 'help-tooltip' }, children),
    InfoTooltip: ({ children, ...props }) =>
      React.createElement('div', { ...props, 'data-testid': 'info-tooltip' }, children),
    Scrollbar: ({ children, autoHideScrollbar: _autoHideScrollbar, showOnHover: _showOnHover, ...props }) =>
      React.createElement('div', { 'data-testid': 'scrollbar', ...props }, children),
    Avatar: ({ children, src, ...props }) =>
      React.createElement('div', { ...props, 'data-testid': 'avatar' }, src ? null : children),
    AvatarImage: ({ src, ...props }) => React.createElement('img', { ...props, src, 'data-testid': 'avatar-image' }),
    AvatarFallback: ({ children, ...props }) =>
      React.createElement('div', { ...props, 'data-testid': 'avatar-fallback' }, children),
    EmojiAvatar: ({ children, ...props }) =>
      React.createElement('div', { ...props, 'data-testid': 'emoji-avatar' }, children),
    EmojiIcon: ({ emoji, className, fluid, fontSize, size }) =>
      React.createElement(
        'div',
        {
          className,
          'data-testid': 'emoji-icon',
          ...(fluid !== undefined ? { 'data-fluid': String(fluid) } : {}),
          ...(fontSize !== undefined ? { 'data-font-size': String(fontSize) } : {}),
          ...(size !== undefined ? { 'data-size': String(size) } : {})
        },
        React.createElement('span', { 'aria-hidden': 'true', 'data-testid': 'emoji-icon-background' }, emoji || '⭐️'),
        emoji
      ),
    DescriptionSwitch: MockCherrystudioUI.DescriptionSwitch,
    Switch: ({ checked, defaultChecked, onCheckedChange, ...props }) =>
      React.createElement('input', {
        ...props,
        type: 'checkbox',
        role: 'switch',
        checked,
        defaultChecked,
        onChange: (e) => onCheckedChange?.(e.target.checked),
        'aria-checked': typeof checked === 'boolean' ? (checked ? 'true' : 'false') : undefined,
        'data-testid': 'switch'
      }),
    // Tabs primitives — flattened: every panel renders, so tests query content without switching
    Tabs: ({ children, ...props }) => React.createElement('div', { ...props, 'data-testid': 'tabs' }, children),
    TabsList: ({ children, ...props }) => React.createElement('div', { ...props, role: 'tablist' }, children),
    TabsTrigger: ({ children, value, ...props }) =>
      React.createElement('button', { ...props, role: 'tab', type: 'button', 'data-value': value }, children),
    TabsContent: ({ children, value, ...props }) =>
      React.createElement('div', { ...props, role: 'tabpanel', 'data-value': value }, children),
    // InputGroup primitives — flattened: the input and its addons render side by side
    InputGroup: ({ children, ...props }) => React.createElement('div', { ...props, role: 'group' }, children),
    InputGroupInput: (props) => React.createElement('input', props),
    InputGroupAddon: ({ children, align, ...props }) =>
      React.createElement('div', { ...props, 'data-align': align }, children),
    InputGroupButton: ({ children, variant, size, ...props }) =>
      React.createElement('button', { ...props, type: 'button' }, children),
    InputGroupText: ({ children, ...props }) => React.createElement('span', props, children),
    // Popover primitives — Radix-style trigger / content split.
    // Content stays mounted so tests can read it without a portal. The trigger
    // still reports open changes, and asChild is not forwarded to the DOM.
    Popover: ({ children, open = false, onOpenChange, ...props }) =>
      React.createElement(
        PopoverContext.Provider,
        { value: { open, onOpenChange } },
        React.createElement('div', { ...props, 'data-testid': 'popover' }, children)
      ),
    PopoverTrigger: ({ children, asChild, ...props }) => {
      const context = React.useContext(PopoverContext)
      const notify = (event) => {
        props.onClick?.(event)
        context.onOpenChange?.(!context.open)
      }
      if (asChild && React.isValidElement(children)) {
        const childProps = children.props ?? {}
        return React.cloneElement(children, {
          ...props,
          ...childProps,
          'data-testid': 'popover-trigger',
          onClick: (event) => {
            childProps.onClick?.(event)
            notify(event)
          }
        })
      }
      return React.createElement('div', { ...props, 'data-testid': 'popover-trigger', onClick: notify }, children)
    },
    PopoverContent: ({
      children,
      align: _align,
      side: _side,
      sideOffset: _sideOffset,
      collisionPadding: _collisionPadding,
      onOpenAutoFocus: _onOpenAutoFocus,
      onCloseAutoFocus: _onCloseAutoFocus,
      forceMount: _forceMount,
      ...props
    }) => {
      const context = React.useContext(PopoverContext)
      if (!context.open) return null
      return createPortal(
        React.createElement('div', { ...props, 'data-testid': 'popover-content' }, children),
        document.body
      )
    },
    HoverCard: ({ children, openDelay: _openDelay, closeDelay: _closeDelay, ...props }) =>
      React.createElement('div', props, children),
    HoverCardTrigger: ({ children, asChild, ...props }) => {
      if (asChild && React.isValidElement(children)) {
        return React.cloneElement(children, { ...props, ...children.props })
      }
      return React.createElement('div', props, children)
    },
    HoverCardContent: ({
      children,
      align: _align,
      side: _side,
      sideOffset: _sideOffset,
      collisionPadding: _collisionPadding,
      ...props
    }) => React.createElement('div', props, children),
    Skeleton: ({ children, ...props }) => React.createElement('div', { ...props, 'data-testid': 'skeleton' }, children),
    // Icon registry stubs
    PROVIDER_ICON_CATALOG: {},
    resolveProviderIcon: () => undefined,
    resolveModelIcon: () => undefined,
    resolveModelToProviderIcon: () => undefined,
    resolveIcon: () => undefined
  }
})

if (typeof globalThis.localStorage === 'undefined' || typeof (globalThis.localStorage as any).getItem !== 'function') {
  let store = new Map<string, string>()

  const localStorageMock = {
    getItem: (key: string) => store.get(key) ?? null,
    setItem: (key: string, value: string) => {
      store.set(key, String(value))
    },
    removeItem: (key: string) => {
      store.delete(key)
    },
    clear: () => {
      store.clear()
    },
    key: (index: number) => Array.from(store.keys())[index] ?? null,
    get length() {
      return store.size
    }
  }

  vi.stubGlobal('localStorage', localStorageMock)
  if (typeof window !== 'undefined') {
    Object.defineProperty(window, 'localStorage', { value: localStorageMock })
  }
}
