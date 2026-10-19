import { X } from 'lucide-react'
import { useLocation, useNavigate, useRouter, useSearch } from '@tanstack/react-router'
import { useCallback, useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { SearchInput } from '@a3s-lab/ui'

import { useSettingsSearchDomIds } from './SettingsSearchDomIds'
import { moveActiveIndex, requestJump, setLiveQuery, useSettingsSearchKeyboard } from './store'

const SEARCH_DEBOUNCE_MS = 150

/**
 * Sidebar search field. Local state echoes keystrokes instantly; the URL is a
 * debounced mirror (per-keystroke navigation would re-persist the whole tab
 * list through the TabRouter sync loop). First debounced entry pushes onto
 * history, subsequent ones replace, so back returns to the origin section.
 * The settings nav mounts this field while search is open. `onCollapse`
 * reports an empty Escape, blur, or leaving the search page so the caller
 * can return to the search icon.
 */
const SettingsSearchBox = ({
  onCollapse,
  autoFocus = true
}: {
  onCollapse: () => void
  autoFocus?: boolean
}) => {
  const navigate = useNavigate()
  const location = useLocation()
  const router = useRouter()
  const search = useSearch({ strict: false })
  const { t } = useTranslation()

  const isSearchPage = location.pathname === '/settings/search'
  const urlQuery = isSearchPage && typeof search.q === 'string' ? search.q : ''
  const { activeIndex, resultCount } = useSettingsSearchKeyboard()
  const { listboxId, optionDomId } = useSettingsSearchDomIds()
  const hasResults = isSearchPage && resultCount > 0
  const [value, setValue] = useState(urlQuery)
  // Tracks the previous input value across effect passes — distinguishes a
  // user-initiated clear from a deep-link seed still in flight
  const prevValueRef = useRef(value)
  // A deep link lands on the search page with the URL already on the history
  // stack, so the first debounced navigate must replace, not push
  const hasPushedRef = useRef(isSearchPage)
  // Distinguishes a new external URL value from an <Activity> re-show re-run
  // (unchanged urlQuery) that must not clobber input typed during the hide
  const appliedUrlRef = useRef(urlQuery)
  // One leave per clear: Escape and the debounced empty-pass both want to
  // leave; a second navigation would overwrite the panel back() returns to
  const leaveInFlightRef = useRef(false)
  // Debounce handle kept reachable so Enter can flush the navigation early
  const debounceRef = useRef<ReturnType<typeof setTimeout> | undefined>(undefined)

  // Navigation that does not end on the search page (menu click, result jump)
  // drops pending keystrokes — else the debounce timer hijacks the trip later
  const prevPathnameRef = useRef(location.pathname)
  useEffect(() => {
    const pathnameChanged = prevPathnameRef.current !== location.pathname
    prevPathnameRef.current = location.pathname
    if (!pathnameChanged) return
    if (isSearchPage) {
      // Any arrival here (own push, back/forward) has the URL on the stack
      // already: the mirrored debounced navigate must replace, not duplicate
      hasPushedRef.current = true
      leaveInFlightRef.current = false
      return
    }
    // Reset even on empty-value exits (the back path skips the value check):
    // a stale true downgrades the next session's first entry to replace
    hasPushedRef.current = false
    leaveInFlightRef.current = false
    // Leaving the search page ends the search flow — collapse back to icon
    onCollapse()
    if (value) {
      setValue('')
      setLiveQuery('')
    }
  }, [location.pathname, isSearchPage, value, onCollapse])

  // External URL updates (history back/forward, deep links in the same tab)
  // sync back into the box. Our own debounced mirrors land as urlQuery ===
  // value.trim() and are ignored; an unchanged urlQuery (Activity re-show)
  // must not clobber input typed during the hide window.
  useEffect(() => {
    if (urlQuery === appliedUrlRef.current) return
    appliedUrlRef.current = urlQuery
    if (urlQuery === value.trim()) return
    setValue(urlQuery)
    setLiveQuery(urlQuery)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [urlQuery])

  // Window-global store: hide/unmount clears it; Activity show re-runs this
  // with surviving state, re-publishing the input the hide phase cleared
  useEffect(() => {
    setLiveQuery(value || undefined)
    return () => setLiveQuery(undefined)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  // Single leave funnel: only one navigation may leave per clear — the
  // fallback push after back() would overwrite the panel back() returns to
  const performLeave = useCallback(() => {
    if (leaveInFlightRef.current) return
    leaveInFlightRef.current = true
    if (router.history.canGoBack()) router.history.back()
    else void navigate({ to: '/settings/general' })
  }, [router, navigate])

  const exitSearch = () => {
    // Only leave when the user actually searched: an empty-box Esc or one
    // before the debounce pushed must not walk the history back.
    const shouldLeave = hasPushedRef.current || isSearchPage
    setValue('')
    setLiveQuery('')
    hasPushedRef.current = false
    if (!shouldLeave) return
    performLeave()
  }

  useEffect(() => {
    const trimmed = value.trim()
    const wasNonEmpty = prevValueRef.current.trim().length > 0
    prevValueRef.current = value
    if (!trimmed) {
      // Guard: a deep-link dispatch lands with the seed setValue still in
      // flight (value reads '' for one pass); only a real user clear may go back.
      if (isSearchPage && wasNonEmpty) {
        performLeave()
      }
      return
    }
    // Typing again abandons any leave still in flight
    leaveInFlightRef.current = false

    debounceRef.current = setTimeout(() => {
      void navigate({
        to: '/settings/search',
        search: { q: trimmed },
        replace: hasPushedRef.current
      })
      hasPushedRef.current = true
    }, SEARCH_DEBOUNCE_MS)
    return () => {
      if (debounceRef.current) clearTimeout(debounceRef.current)
    }
  }, [value, navigate, isSearchPage, router, performLeave])

  const cancelSearch = () => {
    exitSearch()
    onCollapse()
  }

  return (
    <div className="flex w-full items-center gap-1">
      <SearchInput
        autoFocus={autoFocus}
        containerClassName="h-9 min-w-0 flex-1 rounded-[8px] border-0 bg-[#f5f5f5] shadow-none has-[[data-slot=input-group-control]:focus-visible]:border-0"
        value={value}
        placeholder={t('settings.search.placeholder')}
        aria-label={t('settings.search.placeholder')}
        role="combobox"
        aria-autocomplete="list"
        aria-expanded={hasResults}
        aria-controls={hasResults ? listboxId : undefined}
        aria-activedescendant={hasResults ? optionDomId(activeIndex) : undefined}
        className="placeholder:text-[var(--settings-nav-muted)]"
        onChange={(e) => {
          setValue(e.target.value)
          // Immediate pre-debounce mirror: the results page ranks against the
          // live query so Enter never jumps on stale (pre-keystroke) results
          setLiveQuery(e.target.value)
        }}
        onClear={() => {
          exitSearch()
          if (!isSearchPage) onCollapse()
        }}
        clearLabel={t('common.clear')}
        onBlur={() => {
          if (!value && !isSearchPage) onCollapse()
        }}
        onKeyDown={(e) => {
          if (e.key === 'ArrowDown') {
            e.preventDefault()
            moveActiveIndex(1)
          } else if (e.key === 'ArrowUp') {
            e.preventDefault()
            moveActiveIndex(-1)
          } else if (e.key === 'Enter') {
            e.preventDefault()
            if (isSearchPage) {
              requestJump()
            } else if (value.trim()) {
              // Enter before the debounce lands must still respond: off the
              // search page the results list is not mounted, so flush the
              // navigation instead of jumping blind (second Enter jumps)
              if (debounceRef.current) clearTimeout(debounceRef.current)
              void navigate({
                to: '/settings/search',
                search: { q: value.trim() },
                replace: hasPushedRef.current
              })
              hasPushedRef.current = true
            }
          } else if (e.key === 'Escape') {
            e.preventDefault()
            exitSearch()
            if (!isSearchPage) onCollapse()
          }
        }}
      />
      <button
        type="button"
        data-ui="settings.search.cancel"
        aria-label={t('settings.search.cancel')}
        onMouseDown={(event) => event.preventDefault()}
        onClick={cancelSearch}
        className="flex size-9 shrink-0 items-center justify-center rounded-[8px] text-[#3f3f3f] outline-none transition-colors hover:bg-[#f7f7f7] hover:text-[#171717] focus-visible:ring-2 focus-visible:ring-black/10">
        <X className="size-4" strokeWidth={1.7} aria-hidden />
      </button>
    </div>
  )
}

export default SettingsSearchBox
