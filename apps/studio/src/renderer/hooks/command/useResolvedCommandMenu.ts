import { useTranslation } from 'react-i18next'

import { resolveCommandDisplayState } from '@renderer/utils/command'
import { isMac, platform } from '@renderer/utils/platform'
import type { MenuLocation, ResolvedMenuModel } from '@shared/types/command'
import {
  type CommandId,
  findCommandDefinition,
  findKeybindingRule,
  resolveCommandKeybinding,
  resolveMenu
} from '@shared/utils/command'

import { useCommandContextReader } from './useCommandContext'
import { useCommandRuntime, useCommandRuntimeRevision, useCommandShortcutPreferences } from './useCommandRuntime'

export function useResolvedCommandMenu(location: MenuLocation): ResolvedMenuModel<CommandId> {
  const { t } = useTranslation()
  const runtime = useCommandRuntime()
  useCommandRuntimeRevision()
  const context = useCommandContextReader()
  const shortcutPreferences = useCommandShortcutPreferences()

  return resolveMenu({
    location,
    context,
    getCommandState: (command) => {
      const definition = findCommandDefinition(command)
      const rule = findKeybindingRule(command)
      const preference = rule ? shortcutPreferences[command] : undefined
      const keybinding = resolveCommandKeybinding({
        command,
        preference,
        context,
        platform: platform
      })
      const state = resolveCommandDisplayState(command, {
        definition,
        preference,
        context,
        hasHandler: runtime.hasHandler,
        translate: t,
        isMac,
        platform: platform
      })

      return {
        ...state,
        accelerator: keybinding?.accelerator
      }
    }
  })
}
