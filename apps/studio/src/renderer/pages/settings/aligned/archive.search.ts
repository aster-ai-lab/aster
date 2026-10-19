import type { SettingsSearchEntry } from '../settingsSearch/types'

export const route = '/settings/archive'

export const entries: SettingsSearchEntry[] = [
  {
    anchorId: 'search',
    titleKey: 'settings.aligned.archive.search',
    descriptionKey: 'settings.aligned.archive.desc',
    aliases: ['已归档任务', 'archived tasks']
  },
  {
    anchorId: 'project-filter',
    titleKey: 'settings.aligned.archive.all_projects',
    aliases: ['项目', 'project']
  },
  {
    anchorId: 'delete-all',
    titleKey: 'settings.aligned.archive.delete_all',
    aliases: ['删除全部', 'delete all']
  }
]
