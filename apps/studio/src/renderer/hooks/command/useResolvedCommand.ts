import { useTranslation } from 'react-i18next'

import { usePreference } from '@data/hooks/usePreference'
import type { ExtensionCommandId, RendererCommandId } from '@renderer/types/command'
import { resolveCommandDisplayState } from '@renderer/utils/command'
import { isMac, platform } from '@renderer/utils/platform'
import type { ResolvedCommandState } from '@shared/types/command'
import { type CommandId, findCommandDefinition, findKeybindingRule } from '@shared/utils/command'

import { useCommandContextReader } from './useCommandContext'
import { useCommandRuntime, useCommandRuntimeRevision } from './useCommandRuntime'

export function useResolvedCommand(command: RendererCommandId): ResolvedCommandState<RendererCommandId> & {
  execute: () => void
} {
  const { t } = useTranslation()
  const runtime = useCommandRuntime()
  useCommandRuntimeRevision()
  const context = useCommandContextReader()
  const extension = command.startsWith('extension:')
  const builtin = extension ? undefined : (command as CommandId)
  const definition = builtin ? findCommandDefinition(builtin) : undefined
  const rule = builtin ? findKeybindingRule(builtin) : undefined
  const [preference] = usePreference(rule?.preferenceKey ?? 'shortcut.topic.create')

  if (extension) {
    const resolved = runtime.resolveCommand(command as ExtensionCommandId)
    return {
      id: command,
      label: resolved?.title ?? command,
      enabled: resolved?.enabled ?? false,
      shortcutLabel: '',
      iconKey: resolved?.iconKey,
      execute: () => {
        void resolved?.execute()
      }
    }
  }
  const state = resolveCommandDisplayState(command as CommandId, {
    definition,
    preference,
    context,
    hasHandler: runtime.hasHandler,
    translate: t,
    isMac,
    platform: platform
  })

  return {
    id: command,
    ...state,
    execute: () => runtime.execute(command)
  }
}
