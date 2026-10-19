import { Button, Input, Popover, PopoverAnchor, PopoverContent, Tooltip } from '@a3s-lab/ui'
import { cn } from '@a3s-lab/ui/lib/utils'
import { ArrowLeft, ArrowRight, ExternalLink, History, Lock, LockOpen, RotateCw, Search, X } from 'lucide-react'
import type { ReactNode, RefObject } from 'react'
import { useCallback, useEffect, useId, useLayoutEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { useQuery } from '@data/hooks/useDataApi'
import { loggerService } from '@logger'
import { useWebviewNavigation } from '@renderer/hooks/useWebviewNavigation'
import type { WebviewTag } from '@renderer/host/webviewTypes'
import { ipcApi } from '@renderer/ipc'
import { toast } from '@renderer/services/toast'
import {
  normalizeWebviewAddress,
  quarkSearchQuery,
  workspaceBrowserAddress
} from '@renderer/utils/normalizeWebviewAddress'
import type { WebviewAnnotationTarget } from '@shared/types/webviewAnnotation'
import { isHttpUrl } from '@shared/utils/url'

import { WebviewAnnotationControls, type WebviewAnnotationSavedPayload } from './WebviewAnnotationControls'
import { quarkSearchSuggestions } from './workspaceBrowser'

const logger = loggerService.withContext('WebviewNavigation')
const HISTORY_SEARCH_DEBOUNCE_MS = 300

interface Props {
  webviewRef: RefObject<WebviewTag | null>
  webviewRevision: number
  initialUrl: string
  currentUrl?: string | null
  pageTitle?: string
  historyEnabled?: boolean
  isWebviewReady: boolean
  isHostActive: boolean
  target: WebviewAnnotationTarget
  onReload?: () => void
  onStop?: () => void
  loading?: boolean
  onNavigate?: (url: string) => void
  onAnnotationSaved?: (payload: WebviewAnnotationSavedPayload) => void
  toolbarActions?: ReactNode
  /** The embedded page covers a tip or menu that opens downward. */
  tooltipPlacement?: 'top' | 'bottom'
  menuPlacement?: 'top' | 'bottom'
  /** Tells the embedded page to move down while the address menu is open. */
  onMenuHeight?: (height: number) => void
  navigationControls?: {
    canGoBack: boolean
    canGoForward: boolean
    onBack: () => void
    onForward: () => void
  }
}

function commandKeyLabel(key: string): string {
  const mac = typeof navigator !== 'undefined' && /Mac|iPhone|iPad/.test(navigator.userAgent)
  return mac ? `⌘${key}` : `Ctrl+${key}`
}

function addressSecurity(value: string): 'secure' | 'insecure' | 'none' {
  try {
    const url = new URL(value)
    if (url.protocol === 'https:') return 'secure'
    if (url.protocol !== 'http:') return 'none'
    const host = url.hostname
    if (host === 'localhost' || host === '127.0.0.1' || host === '::1') return 'secure'
    return 'insecure'
  } catch {
    return 'none'
  }
}

function isForeignField(target: EventTarget | null, address: HTMLInputElement | null): boolean {
  if (!(target instanceof HTMLElement) || target === address) return false
  return target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.isContentEditable
}

/** A resting http(s) address shows the domain and the path. Focus shows the full URL. */
function restingAddress(value: string): { text: string; host: string; detail: string; detailKind: 'path' | 'none' } {
  try {
    const url = new URL(value)
    if (url.protocol !== 'http:' && url.protocol !== 'https:') {
      return { text: value, host: value, detail: '', detailKind: 'none' }
    }
    const path = `${url.pathname}${url.search}${url.hash}`
    const suffix = path === '/' ? '' : path
    return { text: `${url.host}${suffix}`, host: url.host, detail: suffix, detailKind: suffix ? 'path' : 'none' }
  } catch {
    return { text: value, host: value, detail: '', detailKind: 'none' }
  }
}

export function WebviewNavigation({
  webviewRef,
  webviewRevision,
  initialUrl,
  currentUrl,
  pageTitle,
  historyEnabled = false,
  isWebviewReady,
  isHostActive,
  target,
  onReload,
  onStop,
  loading = false,
  onNavigate,
  onAnnotationSaved,
  toolbarActions,
  tooltipPlacement = 'bottom',
  menuPlacement = 'bottom',
  onMenuHeight,
  navigationControls
}: Props) {
  const { t } = useTranslation()
  const {
    canGoBack,
    canGoForward,
    currentPageUrl,
    addressValue,
    setAddressValue,
    isAddressEditingRef,
    restoreCurrentPageUrl,
    goBack: handleGoBack,
    goForward: handleGoForward
  } = useWebviewNavigation({
    webview: webviewRef.current,
    revision: webviewRevision,
    targetId: target.id,
    url: currentUrl || initialUrl
  })
  const canStepBack = navigationControls ? navigationControls.canGoBack : canGoBack
  const canStepForward = navigationControls ? navigationControls.canGoForward : canGoForward
  const stepBack = navigationControls ? navigationControls.onBack : handleGoBack
  const stepForward = navigationControls ? navigationControls.onForward : handleGoForward
  const addressInputRef = useRef<HTMLInputElement | null>(null)
  const [isAddressFocused, setIsAddressFocused] = useState(false)
  const historyListId = useId()
  const [historyOpen, setHistoryOpen] = useState(false)
  const [historySearch, setHistorySearch] = useState('')
  const [debouncedSearch, setDebouncedSearch] = useState('')
  const [activeSuggestion, setActiveSuggestion] = useState(-1)
  const showHistory = historyEnabled && historyOpen && isHostActive && isWebviewReady
  const search = historySearch.trim().slice(0, 500)
  useEffect(() => {
    if (!showHistory) return
    const timer = setTimeout(() => setDebouncedSearch(search), HISTORY_SEARCH_DEBOUNCE_MS)
    return () => clearTimeout(timer)
  }, [search, showHistory])
  const {
    data: history,
    isLoading: historyLoading,
    error: historyError
  } = useQuery('/browser-visits', {
    query: { search: debouncedSearch, offset: 0, limit: 20 },
    enabled: showHistory && search === debouncedSearch,
    swrOptions: { keepPreviousData: false }
  })
  const historySuggestions =
    search === debouncedSearch
      ? (history?.items ?? [])
          .filter((visit, index, visits) => visits.findIndex((item) => item.url === visit.url) === index)
          .slice(0, 8)
      : []
  const searchQuery = showHistory && search === debouncedSearch ? quarkSearchQuery(debouncedSearch) : null
  const [searchSuggestions, setSearchSuggestions] = useState<string[]>([])
  const [searchPending, setSearchPending] = useState(false)
  useEffect(() => {
    if (!searchQuery) {
      setSearchSuggestions([])
      setSearchPending(false)
      return
    }
    let stale = false
    setSearchSuggestions([])
    setSearchPending(true)
    void quarkSearchSuggestions(searchQuery).then((words) => {
      if (stale) return
      setSearchSuggestions(words)
      setSearchPending(false)
    })
    return () => {
      stale = true
    }
  }, [searchQuery])
  const suggestions = [
    ...searchSuggestions.map((word) => ({ kind: 'search' as const, url: word, title: word })),
    ...historySuggestions.map((visit) => ({
      kind: 'history' as const,
      url: visit.url,
      title: visit.title || visit.url
    }))
  ]
  const selectedSuggestion = showHistory ? suggestions[activeSuggestion] : undefined

  useEffect(() => {
    if (showHistory && activeSuggestion >= 0)
      document.getElementById(`${historyListId}-${activeSuggestion}`)?.scrollIntoView?.({ block: 'nearest' })
  }, [activeSuggestion, historyListId, showHistory])

  const handleReload = useCallback(() => {
    if (onReload) {
      onReload()
      return
    }
    try {
      webviewRef.current?.reload()
    } catch {
      logger.debug('WebView is not ready to reload', { targetId: target.id })
    }
  }, [onReload, target.id, webviewRef])

  const handleOpenExternal = useCallback(() => {
    void ipcApi.request('system.shell.open_external_website', currentPageUrl)
  }, [currentPageUrl])

  const navigateToAddress = useCallback(
    (address: string) => {
      if (!address.trim()) {
        isAddressEditingRef.current = false
        setHistoryOpen(false)
        restoreCurrentPageUrl()
        addressInputRef.current?.blur()
        return
      }
      const normalizedAddress = (historyEnabled ? workspaceBrowserAddress : normalizeWebviewAddress)(address)
      if (!normalizedAddress) {
        toast.error(t('webview.navigation.invalid_address'))
        restoreCurrentPageUrl()
        return
      }

      isAddressEditingRef.current = false
      setHistoryOpen(false)
      setAddressValue(normalizedAddress)
      addressInputRef.current?.blur()

      if (onNavigate) {
        onNavigate(normalizedAddress)
        return
      }

      const webview = webviewRef.current
      if (!webview) {
        toast.error(t('webview.navigation.load_failed'))
        restoreCurrentPageUrl()
        return
      }

      try {
        void webview.loadURL(normalizedAddress).catch((error) => {
          logger.error('Failed to navigate WebView from address bar', error as Error, { targetId: target.id })
          restoreCurrentPageUrl()
          toast.error(t('webview.navigation.load_failed'))
        })
      } catch (error) {
        logger.error('Failed to navigate WebView from address bar', error as Error, { targetId: target.id })
        restoreCurrentPageUrl()
        toast.error(t('webview.navigation.load_failed'))
      }
    },
    [historyEnabled, isAddressEditingRef, onNavigate, restoreCurrentPageUrl, setAddressValue, t, target.id, webviewRef]
  )

  useLayoutEffect(() => {
    if (isAddressFocused) addressInputRef.current?.select()
  }, [isAddressFocused])

  const handleAddressFocus = useCallback(() => {
    isAddressEditingRef.current = true
    setIsAddressFocused(true)
    setHistoryOpen(true)
    setHistorySearch('')
    setActiveSuggestion(-1)
  }, [isAddressEditingRef])

  const handleAddressBlur = useCallback(() => {
    setIsAddressFocused(false)
    setHistoryOpen(false)
    if (!isAddressEditingRef.current) return
    isAddressEditingRef.current = false
    restoreCurrentPageUrl()
  }, [isAddressEditingRef, restoreCurrentPageUrl])

  const handleAddressKeyDown = useCallback(
    (event: React.KeyboardEvent<HTMLInputElement>) => {
      if (event.nativeEvent.isComposing) {
        if (event.key === 'Enter') event.preventDefault()
        return
      }
      if (showHistory && suggestions.length && (event.key === 'ArrowDown' || event.key === 'ArrowUp')) {
        event.preventDefault()
        setActiveSuggestion((current) =>
          event.key === 'ArrowDown'
            ? (current + 1) % suggestions.length
            : current < 0
              ? suggestions.length - 1
              : (current - 1 + suggestions.length) % suggestions.length
        )
        return
      }
      if (event.key === 'Enter' && selectedSuggestion) {
        event.preventDefault()
        navigateToAddress(selectedSuggestion.url)
        return
      }
      if (event.key !== 'Escape') return
      setHistoryOpen(false)
      event.preventDefault()
      isAddressEditingRef.current = false
      restoreCurrentPageUrl()
      event.currentTarget.blur()
    },
    [isAddressEditingRef, navigateToAddress, restoreCurrentPageUrl, selectedSuggestion, showHistory, suggestions.length]
  )

  const menuRef = useRef<HTMLDivElement | null>(null)
  useLayoutEffect(() => {
    if (!onMenuHeight) return
    if (!showHistory) {
      onMenuHeight(0)
      return
    }
    const menu = menuRef.current
    const field = addressInputRef.current
    if (!menu || !field) return
    const report = () => {
      const pageTop =
        field.closest('[data-browser-toolbar]')?.getBoundingClientRect().bottom ?? menu.getBoundingClientRect().top
      onMenuHeight(Math.max(0, Math.ceil(menu.getBoundingClientRect().bottom - pageTop) + 8))
    }
    report()
    const observer = typeof ResizeObserver === 'function' ? new ResizeObserver(report) : undefined
    observer?.observe(menu)
    return () => {
      observer?.disconnect()
      onMenuHeight(0)
    }
  }, [onMenuHeight, showHistory, suggestions.length])

  useEffect(() => {
    if (!isHostActive) return
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.isComposing) return
      const key = event.key.toLowerCase()
      const mod = event.metaKey || event.ctrlKey
      if (isForeignField(event.target, addressInputRef.current)) return
      if (mod && !event.altKey && !event.shiftKey && key === 'l') {
        event.preventDefault()
        addressInputRef.current?.focus()
        addressInputRef.current?.select()
        return
      }
      if (mod && !event.altKey && !event.shiftKey && key === 'r') {
        event.preventDefault()
        handleReload()
        return
      }
      if (mod && !event.altKey && !event.shiftKey && (key === '[' || key === ']')) {
        event.preventDefault()
        if (key === '[') {
          if (canStepBack) stepBack()
        } else if (canStepForward) stepForward()
        return
      }
      if (event.key === 'Escape' && !mod && !event.altKey && !event.shiftKey && loading && onStop) {
        const toolbar = addressInputRef.current?.closest('[data-browser-toolbar]')
        if (
          event.target instanceof Node &&
          toolbar?.contains(event.target) &&
          event.target !== addressInputRef.current
        ) {
          event.preventDefault()
          onStop()
          return
        }
      }
      if (!mod && event.altKey && !event.shiftKey && document.activeElement !== addressInputRef.current) {
        if (event.key === 'ArrowLeft' && canStepBack) {
          event.preventDefault()
          stepBack()
        } else if (event.key === 'ArrowRight' && canStepForward) {
          event.preventDefault()
          stepForward()
        }
      }
    }
    const onShortcut = (event: Event) => {
      if ((event as CustomEvent<string>).detail !== 'focus-address') return
      addressInputRef.current?.focus()
      addressInputRef.current?.select()
    }
    window.addEventListener('keydown', onKeyDown, true)
    window.addEventListener('studio-browser-shortcut', onShortcut)
    return () => {
      window.removeEventListener('keydown', onKeyDown, true)
      window.removeEventListener('studio-browser-shortcut', onShortcut)
    }
  }, [canStepBack, canStepForward, handleReload, isHostActive, loading, onStop, stepBack, stepForward])

  const canOpenExternal = isHttpUrl(currentPageUrl)
  const security = addressSecurity(currentPageUrl)
  const resting = restingAddress(addressValue)
  const addressDisplay = resting.text

  const backLabel = t('webview.navigation.back')
  const forwardLabel = t('webview.navigation.forward')
  const reloadLabel = loading ? t('webview.navigation.stop') : t('webview.navigation.reload')
  const securityLabel =
    security === 'secure'
      ? t('webview.navigation.secure')
      : security === 'insecure'
        ? t('webview.navigation.not_secure')
        : undefined
  const visibleTitle = pageTitle && pageTitle !== currentPageUrl && pageTitle !== resting.host ? pageTitle : ''
  const addressTip = [securityLabel, visibleTitle, currentPageUrl].filter(Boolean).join(' — ')

  return (
    <div
      data-browser-toolbar
      className="relative flex h-8.75 shrink-0 items-center gap-2 border-b border-border-subtle bg-background px-2">
      {loading && (
        <span
          className="pointer-events-none absolute inset-x-0 bottom-0 h-0.5 overflow-hidden"
          role="progressbar"
          aria-label={t('common.loading')}>
          <style>
            {'@keyframes a3s-browser-load{from{transform:translateX(-120%)}to{transform:translateX(420%)}}'}
          </style>
          <span
            className="block h-full w-1/3 bg-foreground/80"
            style={{ animation: 'a3s-browser-load 1.1s ease-in-out infinite' }}
          />
        </span>
      )}
      <div className="flex shrink-0 items-center gap-0.5">
        <Tooltip content={`${backLabel} (${commandKeyLabel('[')})`} placement={tooltipPlacement}>
          <Button
            type="button"
            variant="ghost"
            size="icon-sm"
            disabled={!isWebviewReady || !canStepBack}
            onClick={stepBack}
            className={navigationButtonClassName}
            aria-label={backLabel}>
            <ArrowLeft size={14} />
          </Button>
        </Tooltip>
        <Tooltip content={`${forwardLabel} (${commandKeyLabel(']')})`} placement={tooltipPlacement}>
          <Button
            type="button"
            variant="ghost"
            size="icon-sm"
            disabled={!isWebviewReady || !canStepForward}
            onClick={stepForward}
            className={navigationButtonClassName}
            aria-label={forwardLabel}>
            <ArrowRight size={14} />
          </Button>
        </Tooltip>
        <Tooltip
          content={loading ? reloadLabel : `${reloadLabel} (${commandKeyLabel('R')})`}
          placement={tooltipPlacement}>
          <Button
            type="button"
            variant="ghost"
            size="icon-sm"
            disabled={!isWebviewReady}
            onClick={loading && onStop ? onStop : handleReload}
            className={navigationButtonClassName}
            aria-label={reloadLabel}>
            {loading && onStop ? <X size={14} /> : <RotateCw size={14} />}
          </Button>
        </Tooltip>
      </div>

      <Popover open={showHistory} onOpenChange={setHistoryOpen}>
        <PopoverAnchor asChild>
          <form
            className="@container/address relative mx-1 min-w-0 flex-1"
            onSubmit={(event) => {
              event.preventDefault()
              navigateToAddress(addressValue)
            }}>
            <Input
              ref={addressInputRef}
              type="text"
              inputMode="url"
              value={isAddressFocused ? addressValue : addressDisplay}
              onChange={(event) => {
                setAddressValue(event.target.value)
                setHistorySearch(event.target.value)
                setActiveSuggestion(-1)
                setHistoryOpen(true)
              }}
              onFocus={handleAddressFocus}
              onBlur={handleAddressBlur}
              onKeyDown={handleAddressKeyDown}
              disabled={!isWebviewReady}
              aria-label={t('webview.navigation.address')}
              role={historyEnabled ? 'combobox' : undefined}
              aria-autocomplete={historyEnabled ? 'list' : undefined}
              aria-expanded={historyEnabled ? showHistory : undefined}
              aria-controls={showHistory ? historyListId : undefined}
              aria-activedescendant={selectedSuggestion ? `${historyListId}-${activeSuggestion}` : undefined}
              title={addressTip}
              placeholder={t(
                historyEnabled ? 'webview.navigation.history_placeholder' : 'webview.navigation.address_placeholder'
              )}
              autoCapitalize="none"
              autoComplete="off"
              autoCorrect="off"
              spellCheck={false}
              className={cn(
                'h-7 truncate rounded-md border-input bg-background px-2.5 text-xs shadow-none',
                isAddressFocused ? 'pr-7 text-foreground' : 'text-transparent'
              )}
            />
            {isAddressFocused && addressValue && (
              <Button
                type="button"
                variant="ghost"
                size="icon-sm"
                aria-label={t('webview.navigation.clear')}
                className="absolute top-1/2 right-0.5 -translate-y-1/2 text-muted-foreground"
                onMouseDown={(event) => event.preventDefault()}
                onClick={() => {
                  setAddressValue('')
                  setHistorySearch('')
                  setActiveSuggestion(-1)
                  setHistoryOpen(true)
                }}>
                <X size={12} />
              </Button>
            )}
            {!isAddressFocused && (
              <span
                aria-hidden
                className="pointer-events-none absolute inset-0 flex items-center gap-1.5 overflow-hidden px-2.5 text-xs md:text-sm">
                {security === 'secure' && (
                  <Lock
                    data-address-security="secure"
                    aria-hidden
                    className="size-3.5 shrink-0 text-muted-foreground"
                  />
                )}
                {security === 'insecure' && (
                  <LockOpen
                    data-address-security="insecure"
                    aria-hidden
                    className="size-3.5 shrink-0 text-muted-foreground"
                  />
                )}
                <span className="truncate text-foreground @sm/address:shrink-0">{resting.host}</span>
                {resting.detailKind === 'path' && (
                  <span className="min-w-0 truncate text-muted-foreground">{resting.detail}</span>
                )}
              </span>
            )}
          </form>
        </PopoverAnchor>
        <PopoverContent
          align="start"
          side={menuPlacement}
          avoidCollisions={menuPlacement !== 'top'}
          className="w-[var(--radix-popover-trigger-width)] p-1"
          onOpenAutoFocus={(event) => event.preventDefault()}
          onCloseAutoFocus={(event) => event.preventDefault()}
          onInteractOutside={(event) => {
            if (event.target === addressInputRef.current) event.preventDefault()
          }}>
          <div ref={menuRef}>
            <div
              id={historyListId}
              role="listbox"
              aria-label={t(searchSuggestions.length ? 'webview.navigation.suggestions' : 'settings.browser.history')}
              className="max-h-80 overflow-y-auto">
              {suggestions.map((item, index) => (
                <Button
                  key={`${item.kind}:${item.url}`}
                  id={`${historyListId}-${index}`}
                  role="option"
                  aria-selected={index === activeSuggestion}
                  tabIndex={-1}
                  type="button"
                  variant="ghost"
                  className={cn(
                    'h-auto w-full justify-start gap-2 px-2 py-2 text-start',
                    index === activeSuggestion && 'bg-accent'
                  )}
                  onMouseDown={(event) => event.preventDefault()}
                  onMouseEnter={() => setActiveSuggestion(index)}
                  onClick={() => navigateToAddress(item.url)}>
                  {item.kind === 'search' ? (
                    <Search aria-hidden="true" className="size-4 shrink-0 text-muted-foreground" />
                  ) : (
                    <History aria-hidden="true" className="size-4 shrink-0 text-muted-foreground" />
                  )}
                  <span className="min-w-0 flex-1">
                    <span className="block truncate text-sm">{item.title}</span>
                    {item.kind === 'history' && (
                      <span className="block truncate text-xs text-muted-foreground">{item.url}</span>
                    )}
                  </span>
                </Button>
              ))}
            </div>
            {!suggestions.length && (
              <p role="status" className="px-3 py-4 text-sm text-muted-foreground">
                {t(
                  historyError
                    ? 'settings.browser.error'
                    : historyLoading || searchPending || search !== debouncedSearch
                      ? 'common.loading'
                      : 'settings.browser.noResults'
                )}
              </p>
            )}
          </div>
        </PopoverContent>
      </Popover>

      <div className="flex shrink-0 items-center gap-0.5">
        {onAnnotationSaved && (
          <WebviewAnnotationControls
            webviewRef={webviewRef}
            webviewRevision={webviewRevision}
            isWebviewReady={isWebviewReady}
            isHostActive={isHostActive}
            target={target}
            onAnnotationSaved={onAnnotationSaved}
          />
        )}
        {canOpenExternal ? (
          <Tooltip content={t('webview.navigation.open_external')} placement={tooltipPlacement}>
            <Button
              type="button"
              variant="ghost"
              size="icon-sm"
              onClick={handleOpenExternal}
              className={navigationButtonClassName}
              aria-label={t('webview.navigation.open_external')}>
              <ExternalLink size={14} />
            </Button>
          </Tooltip>
        ) : null}
        {toolbarActions}
      </div>
    </div>
  )
}

const navigationButtonClassName = cn(
  'rounded text-muted-foreground shadow-none active:scale-95',
  'hover:text-foreground disabled:cursor-default disabled:hover:bg-transparent disabled:active:scale-100'
)
