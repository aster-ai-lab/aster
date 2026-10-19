import type { SettingsSearchEntry } from '../settingsSearch/types'

// Indexed rows = window theme and language. Layout rows stay off this page.
export const route = '/settings/appearance'

export const entries: SettingsSearchEntry[] = [
  {
    anchorId: 'theme-mode',
    titleKey: 'settings.appearance.window',
    groupKey: 'settings.appearance.title',
    aliases: ['主题', '跟随系统', 'theme', 'dark mode', '深色模式', '浅色模式', 'light mode', '窗口外观']
  },
  {
    anchorId: 'display-language',
    titleKey: 'common.language',
    groupKey: 'settings.general.common.sections.display_language',
    aliases: ['language', '语言', '中文', '英文']
  }
]
