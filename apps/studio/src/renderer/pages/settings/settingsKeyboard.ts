import { useEffect, useRef } from 'react'

import { setSettingsSurfaceActive } from '@renderer/pages/settings/settingsSurface'

interface SettingsPageKeyboardActions {
  leave: () => void
  openSearch: () => void
}

function isEditableTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false
  if (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA') return true
  return target.isContentEditable || target.getAttribute('contenteditable') === 'true'
}

/** Dialogs, menus, and listboxes already consume Escape and the arrow keys. */
function overlayOwnsKey(target: EventTarget | null): boolean {
  if (!(target instanceof Element)) return false
  return Boolean(target.closest('[role="dialog"], [role="menu"], [role="listbox"]'))
}

function moveSettingsNav(direction: 1 | -1): void {
  const nav = document.querySelector('[data-ui="settings.navigation"]')
  if (!nav) return
  const items = Array.from(nav.querySelectorAll<HTMLButtonElement>('[data-slot="menu-item"]'))
  if (items.length === 0) return
  const current = items.findIndex((item) => item.getAttribute('data-active') === 'true')
  const nextIndex = current < 0 ? 0 : (current + direction + items.length) % items.length
  const next = items[nextIndex]
  next.focus()
  next.click()
}

/**
 * One window listener for every settings page: search, leave, and the sidebar.
 * Registered while the settings route is mounted and removed with it.
 */
export function useSettingsPageKeyboard({ leave, openSearch }: SettingsPageKeyboardActions): void {
  const leaveRef = useRef(leave)
  const openSearchRef = useRef(openSearch)
  leaveRef.current = leave
  openSearchRef.current = openSearch

  useEffect(() => {
    setSettingsSurfaceActive(true)
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.isComposing || event.key === 'Process') return

      const mod = event.metaKey || event.ctrlKey
      if (mod && !event.altKey && event.key.toLowerCase() === 'f' && !event.repeat) {
        event.preventDefault()
        event.stopPropagation()
        openSearchRef.current()
        return
      }

      if (event.key === 'Escape' && !event.altKey && !event.ctrlKey && !event.metaKey && !event.shiftKey) {
        if (event.defaultPrevented || isEditableTarget(event.target) || overlayOwnsKey(event.target)) return
        event.preventDefault()
        event.stopPropagation()
        leaveRef.current()
        return
      }

      if (event.key !== 'ArrowDown' && event.key !== 'ArrowUp') return
      if (event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) return
      if (!(event.target instanceof Element)) return
      if (!event.target.closest('[data-ui="settings.navigation"]')) return
      if (isEditableTarget(event.target) || overlayOwnsKey(event.target)) return
      event.preventDefault()
      moveSettingsNav(event.key === 'ArrowDown' ? 1 : -1)
    }

    window.addEventListener('keydown', onKeyDown)
    return () => {
      setSettingsSurfaceActive(false)
      window.removeEventListener('keydown', onKeyDown)
    }
  }, [])
}
