import type { SettingsSearchEntry } from '../settingsSearch/types'

export const route = '/settings/prompts'

export const entries: SettingsSearchEntry[] = [
  {
    anchorId: 'custom-instructions',
    titleKey: 'settings.aligned.custom.title',
    descriptionKey: 'settings.aligned.custom.desc',
    aliases: ['custom instructions', '自定义指令', '个性化']
  }
]
