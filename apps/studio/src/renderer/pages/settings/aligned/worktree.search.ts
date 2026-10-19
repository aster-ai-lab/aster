import type { SettingsSearchEntry } from '../settingsSearch/types'

export const route = '/settings/worktree'

export const entries: SettingsSearchEntry[] = [
  {
    anchorId: 'refresh',
    titleKey: 'settings.aligned.worktree.refresh',
    descriptionKey: 'settings.aligned.worktree.desc',
    aliases: ['工作树', 'worktree']
  }
]
