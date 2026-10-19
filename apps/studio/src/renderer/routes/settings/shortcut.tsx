import { createFileRoute } from '@tanstack/react-router'

import type { ShortcutSettingsGroup } from '@renderer/hooks/command/useCommandShortcuts'
import ShortcutSettings from '@renderer/pages/settings/ShortcutSettings'
import { type CommandId, findKeybindingRule } from '@shared/utils/command'

const SHORTCUT_SECTIONS: readonly ShortcutSettingsGroup[] = ['general', 'chat', 'topic', 'assistant']

export const Route = createFileRoute('/settings/shortcut')({
  component: ShortcutSettings,
  // `?command=<CommandId>` focuses one row. The id rather than the visible label on purpose:
  // labels are translated, so a link built from one breaks on a locale switch or a copy tweak,
  // silently. An id that no longer maps to a keybinding degrades to the plain list.
  // `?section=` is one sidebar page: 全局与窗口, 消息交互, 会话与对话, AI 助手工具.
  validateSearch: (search: Record<string, unknown>): { command?: CommandId; section?: ShortcutSettingsGroup } => {
    const result: { command?: CommandId; section?: ShortcutSettingsGroup } = {}
    const command = search.command
    if (typeof command === 'string' && findKeybindingRule(command as CommandId)) result.command = command as CommandId
    const section = search.section
    if (typeof section === 'string' && SHORTCUT_SECTIONS.includes(section as ShortcutSettingsGroup)) {
      result.section = section as ShortcutSettingsGroup
    }
    return result
  }
})
