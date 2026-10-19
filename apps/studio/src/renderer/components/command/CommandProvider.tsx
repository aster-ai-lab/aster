import React, { useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react'

import { useMultiplePreferences, usePreference } from '@data/hooks/usePreference'
import {
  CommandRuntimeContext,
  type CommandSharedPreferences,
  CommandSharedPreferencesContext,
  useCommandContextSnapshot
} from '@renderer/hooks/command'
import { RendererCommandService } from '@renderer/services/rendererCommandService'
import { isSettingsSurfaceActive, isSettingsSurfaceCommand } from '@renderer/services/settingsSurfaceService'
import { platform } from '@renderer/utils/platform'
import type { PreferenceShortcutType } from '@shared/data/preference/preferenceTypes'
import type { ContextReader } from '@shared/types/command'
import { type CommandId, REGISTERED_KEYBINDINGS, resolveCommandByKeybinding } from '@shared/utils/command'
import { getShortcutBindingFromKeyboardEvent } from '@shared/utils/shortcut'

interface CommandDispatcherState {
  context: ContextReader
  shortcutPreferences: Partial<Record<CommandId, PreferenceShortcutType>>
  hasHandler: (command: CommandId) => boolean
  execute: (command: CommandId) => void
}

/**
 * True when the event target is a text-entry surface — an `<input>`,
 * `<textarea>`, or any element with `contenteditable`. Used to suppress
 * no-modifier shortcuts so plain keys (Escape, single letters) don't hijack
 * typing.
 */
function isEditableTarget(target: globalThis.EventTarget | null): boolean {
  if (!(target instanceof globalThis.HTMLElement)) {
    return false
  }
  const tagName = target.tagName
  if (tagName === 'INPUT' || tagName === 'TEXTAREA') {
    return true
  }
  // `isContentEditable` reflects the inherited editing state in real browsers;
  // the attribute fallback covers environments (jsdom) that don't compute it.
  return target.isContentEditable || target.getAttribute('contenteditable') === 'true'
}

const shortcutPreferenceKeys = Object.fromEntries(
  REGISTERED_KEYBINDINGS.map((rule) => [rule.command, rule.preferenceKey])
) as Record<CommandId, (typeof REGISTERED_KEYBINDINGS)[number]['preferenceKey']>

export function CommandProvider({ children }: { children: React.ReactNode }) {
  const contextSnapshot = useCommandContextSnapshot()
  const [shortcutPreferences] = useMultiplePreferences(shortcutPreferenceKeys)
  const [menuPresentationMode] = usePreference('menu.presentation_mode')

  const [runtime] = useState(() => new RendererCommandService())
  const { hasHandler, execute, registerHandler } = runtime
  const dispatcherStateRef = useRef<CommandDispatcherState>({
    context: contextSnapshot,
    shortcutPreferences,
    hasHandler,
    execute
  })

  useLayoutEffect(() => {
    runtime.open()
    return () => runtime.close()
  }, [runtime])

  useLayoutEffect(() => runtime.setContext(contextSnapshot), [runtime, contextSnapshot])

  useEffect(() => {
    dispatcherStateRef.current = {
      context: contextSnapshot,
      shortcutPreferences: shortcutPreferences,
      hasHandler,
      execute
    }
  }, [contextSnapshot, execute, hasHandler, shortcutPreferences])

  useEffect(() => {
    const handleKeyDown = (event: globalThis.KeyboardEvent) => {
      if (event.isComposing) {
        return
      }

      // Skip no-modifier shortcuts while an editable target is focused so plain
      // keys (Escape, single letters) don't hijack typing. Modifier combos
      // (Ctrl/Meta/Alt) still fire everywhere; Shift alone does not count.
      const hasModifier = event.ctrlKey || event.metaKey || event.altKey
      if (!hasModifier && isEditableTarget(event.target)) {
        return
      }

      const state = dispatcherStateRef.current
      const binding = getShortcutBindingFromKeyboardEvent(event, platform)
      const command = resolveCommandByKeybinding({
        binding,
        preferences: state.shortcutPreferences,
        context: state.context,
        platform: platform,
        scope: 'renderer',
        canExecuteCommand: state.hasHandler
      })

      if (!command) {
        return
      }

      // The page under the settings overlay stays mounted. Ignore its shortcuts
      // so they do not run, or steal keys, while Settings is in front.
      if (isSettingsSurfaceActive() && !isSettingsSurfaceCommand(command)) {
        return
      }

      event.preventDefault()
      state.execute(command)
    }

    globalThis.window.addEventListener('keydown', handleKeyDown)
    return () => {
      globalThis.window.removeEventListener('keydown', handleKeyDown)
    }
  }, [])

  const value = useMemo(
    () => ({
      execute,
      hasHandler,
      registerHandler,
      registerCommand: runtime.registerCommand,
      resolveCommand: runtime.resolveCommand,
      subscribe: runtime.subscribe,
      getSnapshot: runtime.getSnapshot
    }),
    [execute, hasHandler, registerHandler, runtime]
  )

  const sharedPreferences = useMemo<CommandSharedPreferences>(
    () => ({
      shortcutPreferences: shortcutPreferences,
      menuPresentationMode: menuPresentationMode
    }),
    [shortcutPreferences, menuPresentationMode]
  )

  return (
    <CommandRuntimeContext value={value}>
      <CommandSharedPreferencesContext value={sharedPreferences}>{children}</CommandSharedPreferencesContext>
    </CommandRuntimeContext>
  )
}
