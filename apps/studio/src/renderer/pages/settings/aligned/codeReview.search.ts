import type { SettingsSearchEntry } from '../settingsSearch/types'

export const route = '/settings/code-review'

export const entries: SettingsSearchEntry[] = [
  {
    anchorId: 'mode',
    titleKey: 'settings.aligned.review.mode',
    aliases: ['代码审查', 'code review', '当前会话', '子会话']
  }
]
